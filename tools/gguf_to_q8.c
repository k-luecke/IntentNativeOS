/* tools/gguf_to_q8.c - Converts GGUF Q8_0 weights to contiguous flat binary format
   for Intent-Native OS Ring 3 Freestanding Neural Inference.
   Zero Python, zero external libraries.
*/

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

#define GGUF_MAGIC 0x46554747 // 'GGUF'

enum gguf_type {
    GGUF_TYPE_UINT8   = 0,
    GGUF_TYPE_INT8    = 1,
    GGUF_TYPE_UINT16  = 2,
    GGUF_TYPE_INT16   = 3,
    GGUF_TYPE_UINT32  = 4,
    GGUF_TYPE_INT32   = 5,
    GGUF_TYPE_FLOAT32 = 6,
    GGUF_TYPE_BOOL    = 7,
    GGUF_TYPE_STRING  = 8,
    GGUF_TYPE_ARRAY   = 9,
    GGUF_TYPE_UINT64  = 10,
    GGUF_TYPE_INT64   = 11,
    GGUF_TYPE_FLOAT64 = 12,
};

typedef struct {
    char name[128];
    uint32_t n_dims;
    uint64_t dims[4];
    uint32_t type;
    uint64_t offset;
    size_t size_bytes;
} GgufTensor;

typedef struct {
    int dim;
    int hidden_dim;
    int n_layers;
    int n_heads;
    int n_kv_heads;
    int vocab_size;
    int seq_len;
} ModelConfig;

static void read_str(FILE* f, char* buf, size_t max_len) {
    uint64_t len = 0;
    if (fread(&len, sizeof(uint64_t), 1, f) != 1) return;
    if (len >= max_len) {
        fread(buf, 1, max_len - 1, f);
        buf[max_len - 1] = '\0';
        fseek(f, (long)(len - (max_len - 1)), SEEK_CUR);
    } else {
        fread(buf, 1, len, f);
        buf[len] = '\0';
    }
}

static void skip_val(FILE* f, uint32_t type) {
    if (type <= GGUF_TYPE_INT8 || type == GGUF_TYPE_BOOL) {
        fseek(f, 1, SEEK_CUR);
    } else if (type == GGUF_TYPE_UINT16 || type == GGUF_TYPE_INT16) {
        fseek(f, 2, SEEK_CUR);
    } else if (type == GGUF_TYPE_UINT32 || type == GGUF_TYPE_INT32 || type == GGUF_TYPE_FLOAT32) {
        fseek(f, 4, SEEK_CUR);
    } else if (type == GGUF_TYPE_UINT64 || type == GGUF_TYPE_INT64 || type == GGUF_TYPE_FLOAT64) {
        fseek(f, 8, SEEK_CUR);
    } else if (type == GGUF_TYPE_STRING) {
        uint64_t len = 0;
        fread(&len, sizeof(uint64_t), 1, f);
        fseek(f, (long)len, SEEK_CUR);
    } else if (type == GGUF_TYPE_ARRAY) {
        uint32_t elem_type;
        uint64_t len;
        fread(&elem_type, sizeof(uint32_t), 1, f);
        fread(&len, sizeof(uint64_t), 1, f);
        for (uint64_t i = 0; i < len; i++) {
            skip_val(f, elem_type);
        }
    }
}

static inline float fp16_to_fp32(uint16_t h) {
    uint32_t sign = (h >> 15) & 0x0001;
    uint32_t exp  = (h >> 10) & 0x001f;
    uint32_t mant = h & 0x03ff;
    uint32_t f;
    if (exp == 0) {
        if (mant == 0) {
            f = sign << 31;
        } else {
            while (!(mant & 0x0400)) {
                mant <<= 1;
                exp--;
            }
            exp++;
            mant &= ~0x0400;
            f = (sign << 31) | ((exp + (127 - 15)) << 23) | (mant << 13);
        }
    } else if (exp == 31) {
        f = (sign << 31) | 0x7f800000 | (mant << 13);
    } else {
        f = (sign << 31) | ((exp + (127 - 15)) << 23) | (mant << 13);
    }
    float out;
    memcpy(&out, &f, sizeof(float));
    return out;
}

static size_t calculate_tensor_size(uint32_t type, uint64_t* dims, uint32_t n_dims) {
    uint64_t ne = 1;
    for (uint32_t d = 0; d < n_dims; d++) {
        ne *= dims[d];
    }
    if (type == 0) { // FP32
        return (size_t)ne * sizeof(float);
    } else if (type == 8) { // Q8_0 in GGUF: 34 bytes (uint16_t d + 32 int8 qs)
        return (size_t)(ne / 32) * 34;
    }
    return 0;
}

static GgufTensor* find_tensor(GgufTensor* tensors, uint64_t count, const char* name) {
    for (uint64_t i = 0; i < count; i++) {
        if (strcmp(tensors[i].name, name) == 0) {
            return &tensors[i];
        }
    }
    return NULL;
}

#pragma pack(push, 1)
typedef struct {
    uint16_t d;
    int8_t qs[32];
} gguf_raw_q8_0;

typedef struct {
    float d;
    int8_t qs[32];
} flat_block_q8_0;
#pragma pack(pop)

static void copy_tensor_data(FILE* in, long data_start, GgufTensor* t, FILE* out) {
    fseek(in, data_start + t->offset, SEEK_SET);
    if (t->type == 0) {
        // Direct copy for FP32 tensors
        char buf[65536];
        size_t remaining = t->size_bytes;
        while (remaining > 0) {
            size_t chunk = remaining < sizeof(buf) ? remaining : sizeof(buf);
            if (fread(buf, 1, chunk, in) != chunk) {
                fprintf(stderr, "Error reading FP32 tensor %s\n", t->name);
                exit(1);
            }
            if (fwrite(buf, 1, chunk, out) != chunk) {
                fprintf(stderr, "Error writing FP32 tensor %s\n", t->name);
                exit(1);
            }
            remaining -= chunk;
        }
    } else if (t->type == 8) {
        // Convert Q8_0 from GGUF (uint16_t d) to Flat Binary (float d)
        size_t n_blocks = t->size_bytes / sizeof(gguf_raw_q8_0);
        gguf_raw_q8_0 in_blocks[1024];
        flat_block_q8_0 out_blocks[1024];

        size_t remaining = n_blocks;
        while (remaining > 0) {
            size_t batch = remaining < 1024 ? remaining : 1024;
            if (fread(in_blocks, sizeof(gguf_raw_q8_0), batch, in) != batch) {
                fprintf(stderr, "Error reading Q8_0 blocks for %s\n", t->name);
                exit(1);
            }
            for (size_t b = 0; b < batch; b++) {
                out_blocks[b].d = fp16_to_fp32(in_blocks[b].d);
                memcpy(out_blocks[b].qs, in_blocks[b].qs, 32);
            }
            if (fwrite(out_blocks, sizeof(flat_block_q8_0), batch, out) != batch) {
                fprintf(stderr, "Error writing Q8_0 blocks for %s\n", t->name);
                exit(1);
            }
            remaining -= batch;
        }
    }
}

int main(int argc, char** argv) {
    if (argc < 4) {
        printf("Usage: %s <input.gguf> <output_weights.bin> <output_tokenizer.bin>\n", argv[0]);
        return 1;
    }

    const char* in_path = argv[1];
    const char* out_weights_path = argv[2];
    const char* out_tok_path = argv[3];

    FILE* in = fopen(in_path, "rb");
    if (!in) {
        perror("Failed to open input GGUF");
        return 1;
    }

    uint32_t magic = 0, version = 0;
    uint64_t tensor_count = 0, kv_count = 0;

    fread(&magic, sizeof(uint32_t), 1, in);
    if (magic != GGUF_MAGIC) {
        fprintf(stderr, "Invalid GGUF magic!\n");
        fclose(in);
        return 1;
    }
    fread(&version, sizeof(uint32_t), 1, in);
    fread(&tensor_count, sizeof(uint64_t), 1, in);
    fread(&kv_count, sizeof(uint64_t), 1, in);

    printf("[GGUF_TO_Q8] Input GGUF: Version=%u, Tensors=%lu, Metadata=%lu\n",
           version, (unsigned long)tensor_count, (unsigned long)kv_count);

    ModelConfig cfg = {
        .dim = 576,
        .hidden_dim = 1536,
        .n_layers = 30,
        .n_heads = 9,
        .n_kv_heads = 3,
        .vocab_size = 49152,
        .seq_len = 2048,
    };

    // Extract Tokenizer tokens from metadata
    char** vocab_tokens = (char**)calloc(cfg.vocab_size, sizeof(char*));
    float* vocab_scores = (float*)calloc(cfg.vocab_size, sizeof(float));
    int max_token_len = 0;

    char key_buf[256];
    for (uint64_t i = 0; i < kv_count; i++) {
        read_str(in, key_buf, sizeof(key_buf));
        uint32_t vtype = 0;
        fread(&vtype, sizeof(uint32_t), 1, in);

        if (strcmp(key_buf, "tokenizer.ggml.tokens") == 0 && vtype == GGUF_TYPE_ARRAY) {
            uint32_t elem_type;
            uint64_t len;
            fread(&elem_type, sizeof(uint32_t), 1, in);
            fread(&len, sizeof(uint64_t), 1, in);
            printf("[GGUF_TO_Q8] Extracting %lu tokens from tokenizer.ggml.tokens...\n", (unsigned long)len);
            for (uint64_t t = 0; t < len && t < (uint64_t)cfg.vocab_size; t++) {
                uint64_t slen = 0;
                fread(&slen, sizeof(uint64_t), 1, in);
                char* tok_str = (char*)malloc(slen + 1);
                fread(tok_str, 1, slen, in);
                tok_str[slen] = '\0';
                vocab_tokens[t] = tok_str;
                if ((int)slen > max_token_len) max_token_len = (int)slen;
            }
        } else if (strcmp(key_buf, "tokenizer.ggml.scores") == 0 && vtype == GGUF_TYPE_ARRAY) {
            uint32_t elem_type;
            uint64_t len;
            fread(&elem_type, sizeof(uint32_t), 1, in);
            fread(&len, sizeof(uint64_t), 1, in);
            printf("[GGUF_TO_Q8] Extracting %lu token scores...\n", (unsigned long)len);
            for (uint64_t t = 0; t < len && t < (uint64_t)cfg.vocab_size; t++) {
                fread(&vocab_scores[t], sizeof(float), 1, in);
            }
        } else {
            skip_val(in, vtype);
        }
    }

    // Write tokenizer.bin in sentencepiece format
    if (vocab_tokens[0] != NULL) {
        printf("[GGUF_TO_Q8] Writing tokenizer binary to %s (vocab=%d, max_token_len=%d)...\n",
               out_tok_path, cfg.vocab_size, max_token_len);
        FILE* ftok = fopen(out_tok_path, "wb");
        if (ftok) {
            fwrite(&max_token_len, sizeof(int), 1, ftok);
            for (int i = 0; i < cfg.vocab_size; i++) {
                float score = vocab_scores[i];
                fwrite(&score, sizeof(float), 1, ftok);
                int slen = vocab_tokens[i] ? (int)strlen(vocab_tokens[i]) : 0;
                fwrite(&slen, sizeof(int), 1, ftok);
                if (slen > 0) {
                    fwrite(vocab_tokens[i], 1, slen, ftok);
                }
            }
            fclose(ftok);
            printf("[GGUF_TO_Q8] Tokenizer successfully written.\n");
        }
    }

    // Read Tensor Catalog
    GgufTensor* tensors = (GgufTensor*)calloc(tensor_count, sizeof(GgufTensor));
    for (uint64_t i = 0; i < tensor_count; i++) {
        read_str(in, tensors[i].name, sizeof(tensors[i].name));
        fread(&tensors[i].n_dims, sizeof(uint32_t), 1, in);
        for (uint32_t d = 0; d < tensors[i].n_dims && d < 4; d++) {
            fread(&tensors[i].dims[d], sizeof(uint64_t), 1, in);
        }
        fread(&tensors[i].type, sizeof(uint32_t), 1, in);
        fread(&tensors[i].offset, sizeof(uint64_t), 1, in);
        tensors[i].size_bytes = calculate_tensor_size(tensors[i].type, tensors[i].dims, tensors[i].n_dims);
    }

    long meta_end = ftell(in);
    long data_start = (meta_end + 31) & ~31;
    printf("[GGUF_TO_Q8] Metadata parsed: data_start = %ld\n", data_start);

    // Open output weights file
    FILE* out_w = fopen(out_weights_path, "wb");
    if (!out_w) {
        perror("Failed to open output weights binary");
        return 1;
    }

    // Write 28-byte ModelConfig header
    fwrite(&cfg, sizeof(ModelConfig), 1, out_w);
    printf("[GGUF_TO_Q8] Wrote ModelConfig header: dim=%d, hidden_dim=%d, n_layers=%d, heads=%d, kv_heads=%d, vocab=%d\n",
           cfg.dim, cfg.hidden_dim, cfg.n_layers, cfg.n_heads, cfg.n_kv_heads, cfg.vocab_size);

    // 1. token_embd.weight
    GgufTensor* t_emb = find_tensor(tensors, tensor_count, "token_embd.weight");
    if (!t_emb) { fprintf(stderr, "Missing token_embd.weight!\n"); return 1; }
    printf("  -> Copying token_embd.weight (%zu bytes)...\n", t_emb->size_bytes);
    copy_tensor_data(in, data_start, t_emb, out_w);

    // 2. Layer weights (0 to 29)
    for (int l = 0; l < cfg.n_layers; l++) {
        char name[128];

        // attn_norm
        snprintf(name, sizeof(name), "blk.%d.attn_norm.weight", l);
        GgufTensor* t_attn_norm = find_tensor(tensors, tensor_count, name);
        if (!t_attn_norm) { fprintf(stderr, "Missing %s!\n", name); return 1; }
        copy_tensor_data(in, data_start, t_attn_norm, out_w);

        // attn_q
        snprintf(name, sizeof(name), "blk.%d.attn_q.weight", l);
        GgufTensor* t_wq = find_tensor(tensors, tensor_count, name);
        if (!t_wq) { fprintf(stderr, "Missing %s!\n", name); return 1; }
        copy_tensor_data(in, data_start, t_wq, out_w);

        // attn_k
        snprintf(name, sizeof(name), "blk.%d.attn_k.weight", l);
        GgufTensor* t_wk = find_tensor(tensors, tensor_count, name);
        if (!t_wk) { fprintf(stderr, "Missing %s!\n", name); return 1; }
        copy_tensor_data(in, data_start, t_wk, out_w);

        // attn_v
        snprintf(name, sizeof(name), "blk.%d.attn_v.weight", l);
        GgufTensor* t_wv = find_tensor(tensors, tensor_count, name);
        if (!t_wv) { fprintf(stderr, "Missing %s!\n", name); return 1; }
        copy_tensor_data(in, data_start, t_wv, out_w);

        // attn_output (wo)
        snprintf(name, sizeof(name), "blk.%d.attn_output.weight", l);
        GgufTensor* t_wo = find_tensor(tensors, tensor_count, name);
        if (!t_wo) { fprintf(stderr, "Missing %s!\n", name); return 1; }
        copy_tensor_data(in, data_start, t_wo, out_w);

        // ffn_norm
        snprintf(name, sizeof(name), "blk.%d.ffn_norm.weight", l);
        GgufTensor* t_ffn_norm = find_tensor(tensors, tensor_count, name);
        if (!t_ffn_norm) { fprintf(stderr, "Missing %s!\n", name); return 1; }
        copy_tensor_data(in, data_start, t_ffn_norm, out_w);

        // ffn_gate (w1)
        snprintf(name, sizeof(name), "blk.%d.ffn_gate.weight", l);
        GgufTensor* t_w1 = find_tensor(tensors, tensor_count, name);
        if (!t_w1) { fprintf(stderr, "Missing %s!\n", name); return 1; }
        copy_tensor_data(in, data_start, t_w1, out_w);

        // ffn_down (w2)
        snprintf(name, sizeof(name), "blk.%d.ffn_down.weight", l);
        GgufTensor* t_w2 = find_tensor(tensors, tensor_count, name);
        if (!t_w2) { fprintf(stderr, "Missing %s!\n", name); return 1; }
        copy_tensor_data(in, data_start, t_w2, out_w);

        // ffn_up (w3)
        snprintf(name, sizeof(name), "blk.%d.ffn_up.weight", l);
        GgufTensor* t_w3 = find_tensor(tensors, tensor_count, name);
        if (!t_w3) { fprintf(stderr, "Missing %s!\n", name); return 1; }
        copy_tensor_data(in, data_start, t_w3, out_w);

        if (l % 5 == 0 || l == cfg.n_layers - 1) {
            printf("  -> Processed layer %d/%d\n", l + 1, cfg.n_layers);
        }
    }

    // 3. output_norm.weight (rms_final_weight)
    GgufTensor* t_out_norm = find_tensor(tensors, tensor_count, "output_norm.weight");
    if (!t_out_norm) { fprintf(stderr, "Missing output_norm.weight!\n"); return 1; }
    printf("  -> Copying output_norm.weight (%zu bytes)...\n", t_out_norm->size_bytes);
    copy_tensor_data(in, data_start, t_out_norm, out_w);

    long total_out = ftell(out_w);
    fclose(out_w);
    fclose(in);

    printf("[GGUF_TO_Q8] Success: generated flat weights %s (%ld bytes / %.1f MiB)\n",
           out_weights_path, total_out, total_out / (1024.0 * 1024.0));
    return 0;
}
