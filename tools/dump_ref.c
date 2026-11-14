#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <unistd.h>
#include <math.h>
#include <sys/mman.h>
#include <string.h>

typedef struct {
    int dim;
    int hidden_dim;
    int n_layers;
    int n_heads;
    int n_kv_heads;
    int vocab_size;
    int seq_len;
} Config;

typedef struct {
    float* token_embedding_table;
    float* rms_att_weight;
    float* rms_ffn_weight;
    float* wq;
    float* wk;
    float* wv;
    float* wo;
    float* w1;
    float* w2;
    float* w3;
    float* rms_final_weight;
    float* wcls;
} TransformerWeights;

typedef struct {
    float *x;
    float *xb;
    float *xb2;
    float *hb;
    float *hb2;
    float *q;
    float *k;
    float *v;
    float *att;
    float *logits;
    float *key_cache;
    float *value_cache;
} RunState;

void rmsnorm(float* o, float* x, float* weight, int size) {
    float ss = 0.0f;
    for (int j = 0; j < size; j++) {
        ss += x[j] * x[j];
    }
    ss /= size;
    ss += 1e-5f;
    ss = 1.0f / sqrtf(ss);
    for (int j = 0; j < size; j++) {
        o[j] = x[j] * ss * weight[j];
    }
}

void softmax(float* x, int size) {
    float max_val = x[0];
    for (int i = 1; i < size; i++) {
        if (x[i] > max_val) {
            max_val = x[i];
        }
    }
    float sum = 0.0f;
    for (int i = 0; i < size; i++) {
        x[i] = expf(x[i] - max_val);
        sum += x[i];
    }
    for (int i = 0; i < size; i++) {
        x[i] /= sum;
    }
}

void matmul(float* xout, float* x, float* w, int n, int d) {
    for (int i = 0; i < d; i++) {
        float val = 0.0f;
        for (int j = 0; j < n; j++) {
            val += w[i * n + j] * x[j];
        }
        xout[i] = val;
    }
}

float* forward(Config* p, TransformerWeights* w, RunState* s, int token, int pos) {
    float *x = s->x;
    int dim = p->dim;
    int kv_dim = (p->dim * p->n_kv_heads) / p->n_heads;
    int kv_mul = p->n_heads / p->n_kv_heads;
    int hidden_dim = p->hidden_dim;
    int head_size = dim / p->n_heads;

    float* content_row = w->token_embedding_table + token * dim;
    memcpy(x, content_row, dim * sizeof(*x));

    for (unsigned long long l = 0; l < (unsigned long long)p->n_layers; l++) {
        rmsnorm(s->xb, x, w->rms_att_weight + l * dim, dim);
        int loff = l * p->seq_len * kv_dim;
        s->k = s->key_cache + loff + pos * kv_dim;
        s->v = s->value_cache + loff + pos * kv_dim;

        matmul(s->q, s->xb, w->wq + l * dim * dim, dim, dim);
        matmul(s->k, s->xb, w->wk + l * dim * kv_dim, dim, kv_dim);
        matmul(s->v, s->xb, w->wv + l * dim * kv_dim, dim, kv_dim);

        for (int i = 0; i < dim; i += 2) {
            int head_dim = i % head_size;
            float freq = 1.0f / powf(10000.0f, head_dim / (float)head_size);
            float val = pos * freq;
            float fcr = cosf(val);
            float fci = sinf(val);
            int rotn = i < kv_dim ? 2 : 1;
            for (int v = 0; v < rotn; v++) {
                float* vec = v == 0 ? s->q : s->k;
                float v0 = vec[i];
                float v1 = vec[i + 1];
                vec[i] = v0 * fcr - v1 * fci;
                vec[i + 1] = v0 * fci + v1 * fcr;
            }
        }

        for (int h = 0; h < p->n_heads; h++) {
            float* q = s->q + h * head_size;
            float* att = s->att + h * p->seq_len;
            for (int t = 0; t <= pos; t++) {
                float* k = s->key_cache + loff + t * kv_dim + (h / kv_mul) * head_size;
                float score = 0.0f;
                for (int i = 0; i < head_size; i++) {
                    score += q[i] * k[i];
                }
                score /= sqrtf(head_size);
                att[t] = score;
            }
            softmax(att, pos + 1);

            float* xb = s->xb + h * head_size;
            memset(xb, 0, head_size * sizeof(float));
            for (int t = 0; t <= pos; t++) {
                float* v = s->value_cache + loff + t * kv_dim + (h / kv_mul) * head_size;
                float a = att[t];
                for (int i = 0; i < head_size; i++) {
                    xb[i] += a * v[i];
                }
            }
        }

        matmul(s->xb2, s->xb, w->wo + l * dim * dim, dim, dim);
        for (int i = 0; i < dim; i++) {
            x[i] += s->xb2[i];
        }

        rmsnorm(s->xb, x, w->rms_ffn_weight + l * dim, dim);
        matmul(s->hb, s->xb, w->w1 + l * dim * hidden_dim, dim, hidden_dim);
        matmul(s->hb2, s->xb, w->w3 + l * dim * hidden_dim, dim, hidden_dim);

        for (int i = 0; i < hidden_dim; i++) {
            float val = s->hb[i];
            val *= (1.0f / (1.0f + expf(-val)));
            val *= s->hb2[i];
            s->hb[i] = val;
        }

        matmul(s->xb, s->hb, w->w2 + l * dim * hidden_dim, hidden_dim, dim);
        for (int i = 0; i < dim; i++) {
            x[i] += s->xb[i];
        }
    }

    rmsnorm(x, x, w->rms_final_weight, dim);
    matmul(s->logits, x, w->wcls, dim, p->vocab_size);
    return s->logits;
}

int main() {
    int fd = open("models/pretrained/stories15M.bin", O_RDONLY);
    Config c;
    read(fd, &c, sizeof(Config));
    int shared = c.vocab_size > 0 ? 1 : 0;
    c.vocab_size = abs(c.vocab_size);

    off_t fsize = lseek(fd, 0, SEEK_END);
    float* data = (float*)mmap(NULL, fsize, PROT_READ, MAP_SHARED, fd, 0);
    float* ptr = data + (sizeof(Config) / sizeof(float));

    TransformerWeights w;
    unsigned long long n_layers = c.n_layers;
    int head_size = c.dim / c.n_heads;
    w.token_embedding_table = ptr; ptr += c.vocab_size * c.dim;
    w.rms_att_weight = ptr; ptr += n_layers * c.dim;
    w.wq = ptr; ptr += n_layers * c.dim * c.dim;
    w.wk = ptr; ptr += n_layers * c.dim * ((c.dim * c.n_kv_heads) / c.n_heads);
    w.wv = ptr; ptr += n_layers * c.dim * ((c.dim * c.n_kv_heads) / c.n_heads);
    w.wo = ptr; ptr += n_layers * c.dim * c.dim;
    w.rms_ffn_weight = ptr; ptr += n_layers * c.dim;
    w.w1 = ptr; ptr += n_layers * c.dim * c.hidden_dim;
    w.w2 = ptr; ptr += n_layers * c.hidden_dim * c.dim;
    w.w3 = ptr; ptr += n_layers * c.dim * c.hidden_dim;
    w.rms_final_weight = ptr; ptr += c.dim;
    ptr += c.seq_len * head_size / 2;
    ptr += c.seq_len * head_size / 2;
    w.wcls = shared ? w.token_embedding_table : ptr;

    RunState s;
    int kv_dim = (c.dim * c.n_kv_heads) / c.n_heads;
    s.x = calloc(c.dim, sizeof(float));
    s.xb = calloc(c.dim, sizeof(float));
    s.xb2 = calloc(c.dim, sizeof(float));
    s.hb = calloc(c.hidden_dim, sizeof(float));
    s.hb2 = calloc(c.hidden_dim, sizeof(float));
    s.q = calloc(c.dim, sizeof(float));
    s.key_cache = calloc(c.n_layers * c.seq_len * kv_dim, sizeof(float));
    s.value_cache = calloc(c.n_layers * c.seq_len * kv_dim, sizeof(float));
    s.att = calloc(c.n_heads * c.seq_len, sizeof(float));
    s.logits = calloc(c.vocab_size, sizeof(float));

    // Load tokenizer
    FILE* tfile = fopen("models/pretrained/tokenizer.bin", "rb");
    int max_tok_len;
    fread(&max_tok_len, sizeof(int), 1, tfile);
    char* vocab[32000];
    for (int i = 0; i < c.vocab_size; i++) {
        float score;
        fread(&score, sizeof(float), 1, tfile);
        int len;
        fread(&len, sizeof(int), 1, tfile);
        vocab[i] = malloc(len + 1);
        fread(vocab[i], len, 1, tfile);
        vocab[i][len] = '\0';
    }
    fclose(tfile);

    printf("\n=== GREEDY AUTOREGRESSIVE GENERATION (6 TOKENS) ===\n");
    int token = 1; // <s>
    for (int pos = 0; pos < 6; pos++) {
        float* logits = forward(&c, &w, &s, token, pos);
        int next_tok = 0;
        float max_l = logits[0];
        for (int i = 1; i < c.vocab_size; i++) {
            if (logits[i] > max_l) {
                max_l = logits[i];
                next_tok = i;
            }
        }
        char* piece = vocab[next_tok];
        if (token == 1 && piece[0] == ' ') piece++;
        printf("Step %d: token_in=%d, next_tok=%d, logit=%.4f, piece='%s'\n",
               pos, token, next_tok, max_l, piece);
        token = next_tok;
    }

    close(fd);
    return 0;
}
