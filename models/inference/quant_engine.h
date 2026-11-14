/* models/inference/quant_engine.h - Quantized Inference Engine Interface (Task L103 / L104) */

#ifndef QUANT_ENGINE_H
#define QUANT_ENGINE_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#define QK8_0 32
#define QK4_0 32
#define QK_K 256

/* Block-quantized types matching standard GGUF layouts */
typedef struct {
    float d;               /* delta scale factor */
    int8_t qs[QK8_0];      /* 32 signed 8-bit quantized weights */
} block_q8_0;

typedef struct {
    float d;               /* delta scale factor */
    uint8_t qs[QK4_0 / 2]; /* 16 bytes storing 32 4-bit nibbles */
} block_q4_0;

/* Q6_K super-block: 256 weights in 212 bytes (~6.625 bits/weight).
 * Pareto-optimal rung from Phase 2.3 Fidelity Ladder (0 safety drift, 93.8% op match). */
typedef struct {
    uint8_t ql[QK_K / 2];      /* 128 bytes: lower 4 bits of 256 6-bit quants */
    uint8_t qh[QK_K / 4];      /* 64 bytes: upper 2 bits of 256 6-bit quants */
    int8_t  scales[QK_K / 16]; /* 16 bytes: 8-bit sub-block scales (16 sub-blocks of 16 weights) */
    float   d;                 /* super-block scale factor */
} block_q6_K;

/* Dot product of quantized block vector with float activation vector */
float vec_dot_q8_0(const block_q8_0* vx, const float* vy, int n);
float vec_dot_q6_K(const block_q6_K* vx, const float* vy, int n);
float vec_dot_q4_0(const block_q4_0* vx, const float* vy, int n);

/* Quantize float vector into Q8_0, Q6_K, or Q4_0 blocks */
void quantize_row_q8_0(const float* src, block_q8_0* dst, int n);
void quantize_row_q6_K(const float* src, block_q6_K* dst, int n);
void quantize_row_q4_0(const float* src, block_q4_0* dst, int n);
void dequantize_row_q6_K(const block_q6_K* src, float* dst, int n);

/* L1-Cache-Resident Quantized LoRA Hot-Swap Engine (Rank r <= 16)
 * Down-projection A is stored in Q8_0 blocks; Up-projection B_T is stored in
 * contiguous rank-major layout [rank][dim_out] for vector FMA AXPY sweeps. */
#define LORA_MAX_RANK 16

typedef enum {
    LORA_EXEC_NONE = 0,
    LORA_EXEC_SIDECAR_Q8 = 1, /* Fast hot-swap mode (< 200 ns switch, Q8_0 A + AXPY B_T) */
    LORA_EXEC_PREMERGED  = 2  /* Fused weight mode (0.0% per-token overhead on held regimes) */
} lora_exec_mode_t;

typedef struct {
    char name[32];
    uint32_t domain_id;
    uint32_t generation;
    int rank;
    int dim_in;
    int dim_out;
    float alpha;
    float scale;          /* alpha / rank */
    block_q8_0* lora_a_q8;/* [rank * (dim_in / QK8_0)] quantized down-projection */
    float* lora_b_t;      /* [rank * dim_out] contiguous rank-major up-projection */
} lora_adapter_q8_t;

int lora_adapter_q8_init(
    lora_adapter_q8_t* adapter,
    const char* name,
    uint32_t domain_id,
    int rank,
    int dim_in,
    int dim_out,
    float alpha,
    const float* a_fp32,  /* [rank * dim_in] */
    const float* b_fp32   /* [dim_out * rank] */
);
void lora_adapter_q8_free(lora_adapter_q8_t* adapter);

/* Execute low-rank sidecar: out += scale * B * (A_q8 * x) using 4-way FMA + AXPY */
void apply_lora_q8_sidecar(
    float* out,
    const float* x,
    const lora_adapter_q8_t* adapter
);

/* Fuse active LoRA adapter into a Q8_0 base weight matrix when Tonic Gate holds regime:
 * W_fused_q8[i] = quantize_q8_0(dequant(W_base_q8[i]) + scale * sum_r B_T[r][i] * dequant(A_q8[r])) */
void fuse_lora_into_q8_weights(
    const block_q8_0* w_base,
    block_q8_0* w_fused_out,
    const lora_adapter_q8_t* adapter
);

/* Top-K Mixture-of-LoRA-Experts (MoLE) Subspace Blending for Cross-App Workflows:
 * out += sum_{k=0..k_count-1} weights[k] * scale_k * B_k * (A_k_q8 * x) */
void apply_mole_topk_q8(
    float* out,
    const float* x,
    const lora_adapter_q8_t* const* adapters,
    const float* weights,
    int k_count
);

/* FNWD (Fast Native Weight Dispatch) Single-Pass Activation-Shared MoLE Kernel:
 * Shares x across all K adapters in L1D during Q8_0 down-projection and fuses
 * the rank-major B_T up-projection in SIMD register accumulators so out[0..dim_out-1]
 * is written in a single pass. */
void apply_mole_topk_q8_fnwd(
    float* out,
    const float* x,
    const lora_adapter_q8_t* const* adapters,
    const float* weights,
    int k_count
);

/* Rotary Positional Embedding (RoPE) */
void apply_rope(float* vec, int pos, int dim, int head_dim);

/* KV Cache structure */
typedef struct {
    float* key_cache;   /* [layers, max_seq, dim] */
    float* val_cache;   /* [layers, max_seq, dim] */
    int max_seq;
    int dim;
    int layers;
} kv_cache_t;

kv_cache_t* kv_cache_init(int layers, int max_seq, int dim);
void kv_cache_free(kv_cache_t* cache);

/* Argmax / greedy decoding */
int sample_greedy(const float* logits, int vocab_size);

/* Numerical parity check: returns maximum absolute error and cosine similarity */
float compute_max_abs_error(const float* ref, const float* test, int n);
float compute_cosine_similarity(const float* ref, const float* test, int n);

#endif /* QUANT_ENGINE_H */
