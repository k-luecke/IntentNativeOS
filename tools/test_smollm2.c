/* tools/test_smollm2.c - Host unit test for SmolLM2-135M-Instruct Q8_0 inference */

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/mman.h>
#include <math.h>
#include <time.h>

#define QK8_0 32

#pragma pack(push, 1)
typedef struct {
    float d;
    int8_t qs[32];
} flat_block_q8_0;
#pragma pack(pop)

typedef struct {
    int dim;
    int hidden_dim;
    int n_layers;
    int n_heads;
    int n_kv_heads;
    int vocab_size;
    int seq_len;
} ModelConfig;

typedef struct {
    const flat_block_q8_0* token_embedding_table; // [vocab_size, dim / 32]
    const float* attn_norm[30];
    const flat_block_q8_0* wq[30];  // [dim, dim / 32]
    const flat_block_q8_0* wk[30];  // [kv_dim, dim / 32]
    const flat_block_q8_0* wv[30];  // [kv_dim, dim / 32]
    const flat_block_q8_0* wo[30];  // [dim, dim / 32]
    const float* ffn_norm[30];
    const flat_block_q8_0* w1[30];  // [hidden_dim, dim / 32]
    const flat_block_q8_0* w2[30];  // [dim, hidden_dim / 32]
    const flat_block_q8_0* w3[30];  // [hidden_dim, dim / 32]
    const float* output_norm;       // [dim]
} SmolLM2Weights;

typedef struct {
    float x[576];
    float xb[576];
    float xb2[576];
    float hb[1536];
    float hb2[1536];
    float q[576];
    float att[9 * 512];
    float logits[49152];
    float key_cache[30 * 512 * 192];
    float value_cache[30 * 512 * 192];
} RunState;

static inline float vec_dot_q8_0(const flat_block_q8_0* vx, const float* vy, int n) {
    int nb = n / 32;
    float sum = 0.0f;
    for (int b = 0; b < nb; b++) {
        const flat_block_q8_0* blk = &vx[b];
        const float* y = &vy[b * 32];
        float d = blk->d;
        float block_sum = 0.0f;
        for (int j = 0; j < 32; j++) {
            block_sum += (float)blk->qs[j] * y[j];
        }
        sum += block_sum * d;
    }
    return sum;
}

static void matmul_q8(float* xout, const float* x, const flat_block_q8_0* w, int n, int d) {
    int blocks_per_row = n / 32;
    for (int i = 0; i < d; i++) {
        xout[i] = vec_dot_q8_0(&w[i * blocks_per_row], x, n);
    }
}

static void rmsnorm(float* o, const float* x, const float* weight, int size) {
    float ss = 0.0f;
    for (int j = 0; j < size; j++) {
        ss += x[j] * x[j];
    }
    ss /= size;
    ss += 1e-5f;
    float inv_ss = 1.0f / sqrtf(ss);
    for (int j = 0; j < size; j++) {
        o[j] = x[j] * inv_ss * weight[j];
    }
}

static void softmax(float* x, int size) {
    float max_val = x[0];
    for (int i = 1; i < size; i++) {
        if (x[i] > max_val) max_val = x[i];
    }
    float sum = 0.0f;
    for (int i = 0; i < size; i++) {
        x[i] = expf(x[i] - max_val);
        sum += x[i];
    }
    float inv_sum = 1.0f / sum;
    for (int i = 0; i < size; i++) {
        x[i] *= inv_sum;
    }
}

static float* forward(const ModelConfig* cfg, const SmolLM2Weights* w, RunState* s, int token, int pos) {
    float* x = s->x;
    int dim = cfg->dim;
    int kv_dim = (cfg->dim * cfg->n_kv_heads) / cfg->n_heads; // 192
    int kv_mul = cfg->n_heads / cfg->n_kv_heads;             // 3
    int hidden_dim = cfg->hidden_dim;                         // 1536
    int head_size = dim / cfg->n_heads;                       // 64
    int max_seq = 512;

    // Embedding lookup & dequantize
    const flat_block_q8_0* emb_row = &w->token_embedding_table[token * (dim / 32)];
    for (int b = 0; b < dim / 32; b++) {
        float d = emb_row[b].d;
        for (int j = 0; j < 32; j++) {
            x[b * 32 + j] = (float)emb_row[b].qs[j] * d;
        }
    }

    for (int l = 0; l < cfg->n_layers; l++) {
        rmsnorm(s->xb, x, w->attn_norm[l], dim);

        int loff = l * max_seq * kv_dim;
        float* k_cache = s->key_cache + loff + pos * kv_dim;
        float* v_cache = s->value_cache + loff + pos * kv_dim;

        matmul_q8(s->q, s->xb, w->wq[l], dim, dim);
        matmul_q8(k_cache, s->xb, w->wk[l], dim, kv_dim);
        matmul_q8(v_cache, s->xb, w->wv[l], dim, kv_dim);

        // RoPE with base 100,000
        for (int i = 0; i < dim; i += 2) {
            int head_dim = i % head_size;
            float freq = 1.0f / powf(100000.0f, (float)head_dim / head_size);
            float val = pos * freq;
            float fcr = cosf(val);
            float fci = sinf(val);
            int rotn = i < kv_dim ? 2 : 1;
            for (int v = 0; v < rotn; v++) {
                float* vec = v == 0 ? s->q : k_cache;
                float v0 = vec[i];
                float v1 = vec[i + 1];
                vec[i] = v0 * fcr - v1 * fci;
                vec[i + 1] = v0 * fci + v1 * fcr;
            }
        }

        // Multi-head attention with GQA (9:3)
        for (int h = 0; h < cfg->n_heads; h++) {
            float* q = s->q + h * head_size;
            float* att = s->att + h * max_seq;
            for (int t = 0; t <= pos; t++) {
                float* k = s->key_cache + loff + t * kv_dim + (h / kv_mul) * head_size;
                float score = 0.0f;
                for (int i = 0; i < head_size; i++) {
                    score += q[i] * k[i];
                }
                score /= sqrtf((float)head_size);
                att[t] = score;
            }
            softmax(att, pos + 1);

            float* xb = s->xb + h * head_size;
            for (int i = 0; i < head_size; i++) {
                xb[i] = 0.0f;
            }
            for (int t = 0; t <= pos; t++) {
                float* v = s->value_cache + loff + t * kv_dim + (h / kv_mul) * head_size;
                float a = att[t];
                for (int i = 0; i < head_size; i++) {
                    xb[i] += a * v[i];
                }
            }
        }

        matmul_q8(s->xb2, s->xb, w->wo[l], dim, dim);
        for (int i = 0; i < dim; i++) {
            x[i] += s->xb2[i];
        }

        // FFN
        rmsnorm(s->xb, x, w->ffn_norm[l], dim);
        matmul_q8(s->hb, s->xb, w->w1[l], dim, hidden_dim);
        matmul_q8(s->hb2, s->xb, w->w3[l], dim, hidden_dim);

        // SwiGLU: silu(hb) * hb2
        for (int i = 0; i < hidden_dim; i++) {
            float val = s->hb[i];
            val *= (1.0f / (1.0f + expf(-val)));
            val *= s->hb2[i];
            s->hb[i] = val;
        }

        matmul_q8(s->xb, s->hb, w->w2[l], hidden_dim, dim);
        for (int i = 0; i < dim; i++) {
            x[i] += s->xb[i];
        }
    }

    // Final RMSNorm & Output projection (tied to token_embedding_table)
    rmsnorm(x, x, w->output_norm, dim);
    matmul_q8(s->logits, x, w->token_embedding_table, dim, cfg->vocab_size);
    return s->logits;
}

// Tokenizer vocabulary lookup
typedef struct {
    char* str;
    float score;
} TokenEntry;

static TokenEntry g_vocab[49152];
static int g_max_token_len = 0;

static void load_tokenizer(const char* path) {
    FILE* f = fopen(path, "rb");
    if (!f) { perror("fopen tokenizer"); exit(1); }
    fread(&g_max_token_len, sizeof(int), 1, f);
    for (int i = 0; i < 49152; i++) {
        fread(&g_vocab[i].score, sizeof(float), 1, f);
        int len = 0;
        fread(&len, sizeof(int), 1, f);
        g_vocab[i].str = (char*)malloc(len + 1);
        if (len > 0) {
            fread(g_vocab[i].str, 1, len, f);
        }
        g_vocab[i].str[len] = '\0';
    }
    fclose(f);
}

static int find_tok(const char* str) {
    for (int i = 0; i < 49152; i++) {
        if (g_vocab[i].str && strcmp(g_vocab[i].str, str) == 0) return i;
    }
    return -1;
}

int main(int argc, char** argv) {
    const char* weights_path = argc > 1 ? argv[1] : "models/pretrained/smollm2_135m_q8.bin";
    const char* tok_path = argc > 2 ? argv[2] : "models/pretrained/smollm2_tokenizer.bin";

    printf("[HOST_SMOLLM2] Loading tokenizer from %s...\n", tok_path);
    load_tokenizer(tok_path);

    printf("[HOST_SMOLLM2] Loading weights from %s...\n", weights_path);
    int fd = open(weights_path, O_RDONLY);
    if (fd < 0) { perror("open weights"); return 1; }
    off_t fsize = lseek(fd, 0, SEEK_END);
    lseek(fd, 0, SEEK_SET);

    uint8_t* ptr = (uint8_t*)mmap(NULL, fsize, PROT_READ, MAP_PRIVATE, fd, 0);
    if (ptr == MAP_FAILED) { perror("mmap weights"); return 1; }

    ModelConfig* cfg = (ModelConfig*)ptr;
    ptr += sizeof(ModelConfig);

    printf("[HOST_SMOLLM2] ModelConfig: dim=%d, hidden_dim=%d, layers=%d, heads=%d, kv_heads=%d, vocab=%d\n",
           cfg->dim, cfg->hidden_dim, cfg->n_layers, cfg->n_heads, cfg->n_kv_heads, cfg->vocab_size);

    SmolLM2Weights w;
    int dim = cfg->dim;
    int hidden_dim = cfg->hidden_dim;
    int kv_dim = (cfg->dim * cfg->n_kv_heads) / cfg->n_heads;

    // token_embd: [vocab_size, dim] Q8_0
    w.token_embedding_table = (const flat_block_q8_0*)ptr;
    ptr += (size_t)cfg->vocab_size * (dim / 32) * sizeof(flat_block_q8_0);

    for (int l = 0; l < cfg->n_layers; l++) {
        w.attn_norm[l] = (const float*)ptr; ptr += dim * sizeof(float);
        w.wq[l] = (const flat_block_q8_0*)ptr; ptr += (size_t)dim * (dim / 32) * sizeof(flat_block_q8_0);
        w.wk[l] = (const flat_block_q8_0*)ptr; ptr += (size_t)kv_dim * (dim / 32) * sizeof(flat_block_q8_0);
        w.wv[l] = (const flat_block_q8_0*)ptr; ptr += (size_t)kv_dim * (dim / 32) * sizeof(flat_block_q8_0);
        w.wo[l] = (const flat_block_q8_0*)ptr; ptr += (size_t)dim * (dim / 32) * sizeof(flat_block_q8_0);
        w.ffn_norm[l] = (const float*)ptr; ptr += dim * sizeof(float);
        w.w1[l] = (const flat_block_q8_0*)ptr; ptr += (size_t)hidden_dim * (dim / 32) * sizeof(flat_block_q8_0);
        w.w2[l] = (const flat_block_q8_0*)ptr; ptr += (size_t)dim * (hidden_dim / 32) * sizeof(flat_block_q8_0);
        w.w3[l] = (const flat_block_q8_0*)ptr; ptr += (size_t)hidden_dim * (dim / 32) * sizeof(flat_block_q8_0);
    }
    w.output_norm = (const float*)ptr; ptr += dim * sizeof(float);

    size_t consumed = ptr - (uint8_t*)cfg;
    printf("[HOST_SMOLLM2] Ingested weights: %zu / %ld bytes mapped\n", consumed, fsize);

    static RunState s;
    memset(&s, 0, sizeof(s));

    int prompt_tokens[64];
    int n_prompt = 0;

    // Tokenize ChatML with system prompt:
    // <|im_start|>system\nPlan OS intent as JSON: {"op": "duplicate", "src": "draft.txt", "dst": "backup.txt"}<|im_end|>\n
    // <|im_start|>user\nsafely duplicate draft.txt to backup<|im_end|>\n
    // <|im_start|>assistant\n
    // user turn only (no system prompt):
    prompt_tokens[n_prompt++] = 1; // <|im_start|>
    prompt_tokens[n_prompt++] = find_tok("user");
    prompt_tokens[n_prompt++] = find_tok("Ċ"); // \n
    prompt_tokens[n_prompt++] = find_tok("Copy");
    if (prompt_tokens[n_prompt - 1] == -1) prompt_tokens[n_prompt - 1] = find_tok("ĠCopy");
    prompt_tokens[n_prompt++] = find_tok("Ġfile");
    prompt_tokens[n_prompt++] = find_tok("Ġdraft");
    prompt_tokens[n_prompt++] = find_tok(".");
    prompt_tokens[n_prompt++] = find_tok("txt");
    prompt_tokens[n_prompt++] = find_tok("Ġto");
    prompt_tokens[n_prompt++] = find_tok("Ġbackup");
    prompt_tokens[n_prompt++] = 2; // <|im_end|>
    prompt_tokens[n_prompt++] = find_tok("Ċ"); // \n



    // assistant turn:
    prompt_tokens[n_prompt++] = 1; // <|im_start|>
    prompt_tokens[n_prompt++] = find_tok("ass");
    prompt_tokens[n_prompt++] = find_tok("istant");
    prompt_tokens[n_prompt++] = find_tok("Ċ"); // \n
    prompt_tokens[n_prompt++] = find_tok("{");
    prompt_tokens[n_prompt++] = find_tok("\"");
    prompt_tokens[n_prompt++] = find_tok("op");
    prompt_tokens[n_prompt++] = find_tok("\"");
    prompt_tokens[n_prompt++] = find_tok(":");
    prompt_tokens[n_prompt++] = find_tok("Ġ\"");
    prompt_tokens[n_prompt++] = find_tok("copy");
    prompt_tokens[n_prompt++] = find_tok("\"");
    prompt_tokens[n_prompt++] = find_tok(",");
    prompt_tokens[n_prompt++] = find_tok("Ġ\"");
    prompt_tokens[n_prompt++] = find_tok("src");
    prompt_tokens[n_prompt++] = find_tok("\"");
    prompt_tokens[n_prompt++] = find_tok(":");
    prompt_tokens[n_prompt++] = find_tok("Ġ\"");

    printf("[HOST_SMOLLM2] Ingesting prompt (%d tokens):\n  ", n_prompt);

    for (int i = 0; i < n_prompt; i++) {
        int tid = prompt_tokens[i];
        if (tid >= 0 && tid < 49152 && g_vocab[tid].str) {
            printf("[%d: '%s'] ", tid, g_vocab[tid].str);
        } else {
            printf("[%d: NULL] ", tid);
        }
    }
    printf("\n\n");
    fflush(stdout);

    int token = 0;
    for (int pos = 0; pos < n_prompt; pos++) {
        token = prompt_tokens[pos];
        float* logits = forward(cfg, &w, &s, token, pos);
    }

    // Now autoregressively generate up to 20 tokens
    int cur_pos = n_prompt;
    char generated_text[512] = {0};

    for (int gen = 0; gen < 25; gen++) {
        // Forward pass on previous token at (cur_pos - 1) already computed logits if pos was updated,
        // or compute on token
        // In the loop above, logits for prompt_tokens[n_prompt - 1] at pos = n_prompt - 1 are in s.logits!
        // Top 5 candidates
        int top5[5] = {0};
        float top5_l[5] = {-1e9f, -1e9f, -1e9f, -1e9f, -1e9f};
        for (int i = 0; i < cfg->vocab_size; i++) {
            float l = s.logits[i];
            for (int k = 0; k < 5; k++) {
                if (l > top5_l[k]) {
                    for (int j = 4; j > k; j--) {
                        top5_l[j] = top5_l[j - 1];
                        top5[j] = top5[j - 1];
                    }
                    top5_l[k] = l;
                    top5[k] = i;
                    break;
                }
            }
        }
        printf("Token %2d (pos %2d): top1=%5d (%.2f, '%s') | top2=%5d (%.2f, '%s') | top3=%5d (%.2f, '%s')\n",
               gen, cur_pos,
               top5[0], top5_l[0], g_vocab[top5[0]].str ? g_vocab[top5[0]].str : "?",
               top5[1], top5_l[1], g_vocab[top5[1]].str ? g_vocab[top5[1]].str : "?",
               top5[2], top5_l[2], g_vocab[top5[2]].str ? g_vocab[top5[2]].str : "?");

        int top_tok = top5[0];
        if (top_tok == 2 || top_tok == 0) break;
        forward(cfg, &w, &s, top_tok, cur_pos);
        cur_pos++;
    }

    return 0;
}

