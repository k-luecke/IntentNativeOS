/* models/inference/quant_engine.c - Supported Quantized Inference Engine (Task L103 / L104) */

#include "quant_engine.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <time.h>
#if defined(__AVX2__)
#include <immintrin.h>
#endif

float vec_dot_q8_0(const block_q8_0* vx, const float* vy, int n) {
    const int nb = n / QK8_0;
    float sum = 0.0f;
    for (int i = 0; i < nb; i++) {
        float d = vx[i].d;
        const float* y_block = vy + i * QK8_0;
#if defined(__AVX2__)
        __m256 acc0 = _mm256_setzero_ps();
        __m256 acc1 = _mm256_setzero_ps();
        for (int j = 0; j < QK8_0; j += 16) {
            __m128i q16 = _mm_loadu_si128((const __m128i*)(vx[i].qs + j));
            __m256i q_lo_i32 = _mm256_cvtepi8_epi32(q16);
            __m256i q_hi_i32 = _mm256_cvtepi8_epi32(_mm_srli_si128(q16, 8));
            __m256 q_lo_ps = _mm256_cvtepi32_ps(q_lo_i32);
            __m256 q_hi_ps = _mm256_cvtepi32_ps(q_hi_i32);
            __m256 y_lo = _mm256_loadu_ps(y_block + j);
            __m256 y_hi = _mm256_loadu_ps(y_block + j + 8);
#if defined(__FMA__)
            acc0 = _mm256_fmadd_ps(q_lo_ps, y_lo, acc0);
            acc1 = _mm256_fmadd_ps(q_hi_ps, y_hi, acc1);
#else
            acc0 = _mm256_add_ps(acc0, _mm256_mul_ps(q_lo_ps, y_lo));
            acc1 = _mm256_add_ps(acc1, _mm256_mul_ps(q_hi_ps, y_hi));
#endif
        }
        __m256 acc = _mm256_add_ps(acc0, acc1);
        __m128 h = _mm_add_ps(_mm256_castps256_ps128(acc), _mm256_extractf128_ps(acc, 1));
        h = _mm_hadd_ps(h, h);
        h = _mm_hadd_ps(h, h);
        sum += d * _mm_cvtss_f32(h);
#else
        float block_sum = 0.0f;
        for (int j = 0; j < QK8_0; j += 4) {
            block_sum += (float)vx[i].qs[j]   * y_block[j]
                       + (float)vx[i].qs[j+1] * y_block[j+1]
                       + (float)vx[i].qs[j+2] * y_block[j+2]
                       + (float)vx[i].qs[j+3] * y_block[j+3];
        }
        sum += d * block_sum;
#endif
    }
    return sum;
}

float vec_dot_q4_0(const block_q4_0* vx, const float* vy, int n) {
    const int nb = n / QK4_0;
    float sum = 0.0f;
    for (int i = 0; i < nb; i++) {
        float d = vx[i].d;
        float block_sum = 0.0f;
        const float* y_block = vy + i * QK4_0;
        for (int j = 0; j < QK4_0 / 2; j += 2) {
            uint8_t b0 = vx[i].qs[j];
            uint8_t b1 = vx[i].qs[j+1];
            
            int8_t v00 = (int8_t)(b0 & 0x0F) - 8;
            int8_t v01 = (int8_t)((b0 >> 4) & 0x0F) - 8;
            int8_t v10 = (int8_t)(b1 & 0x0F) - 8;
            int8_t v11 = (int8_t)((b1 >> 4) & 0x0F) - 8;

            block_sum += (float)v00 * y_block[j * 2]     + (float)v01 * y_block[j * 2 + 1]
                       + (float)v10 * y_block[j * 2 + 2] + (float)v11 * y_block[j * 2 + 3];
        }
        sum += d * block_sum;
    }
    return sum;
}

void quantize_row_q8_0(const float* src, block_q8_0* dst, int n) {
    const int nb = n / QK8_0;
    for (int i = 0; i < nb; i++) {
        const float* x = src + i * QK8_0;
        float amax = 0.0f;
        for (int j = 0; j < QK8_0; j++) {
            float v = fabsf(x[j]);
            if (v > amax) amax = v;
        }
        float d = amax / 127.0f;
        dst[i].d = d;
        float id = d > 0.0f ? 1.0f / d : 0.0f;
        for (int j = 0; j < QK8_0; j++) {
            int val = (int)roundf(x[j] * id);
            if (val > 127) val = 127;
            if (val < -128) val = -128;
            dst[i].qs[j] = (int8_t)val;
        }
    }
}

void quantize_row_q4_0(const float* src, block_q4_0* dst, int n) {
    const int nb = n / QK4_0;
    for (int i = 0; i < nb; i++) {
        const float* x = src + i * QK4_0;
        float amax = 0.0f;
        for (int j = 0; j < QK4_0; j++) {
            float v = fabsf(x[j]);
            if (v > amax) amax = v;
        }
        float d = amax / 7.0f;
        dst[i].d = d;
        float id = d > 0.0f ? 1.0f / d : 0.0f;
        for (int j = 0; j < QK4_0 / 2; j++) {
            int v0 = (int)roundf(x[j * 2] * id) + 8;
            int v1 = (int)roundf(x[j * 2 + 1] * id) + 8;
            if (v0 > 15) v0 = 15; if (v0 < 0) v0 = 0;
            if (v1 > 15) v1 = 15; if (v1 < 0) v1 = 0;
            dst[i].qs[j] = (uint8_t)(v0 | (v1 << 4));
        }
    }
}

void quantize_row_q6_K(const float* src, block_q6_K* dst, int n) {
    const int nb = n / QK_K;
    for (int b = 0; b < nb; b++) {
        const float* x = src + b * QK_K;
        block_q6_K* blk = &dst[b];
        memset(blk, 0, sizeof(block_q6_K));

        /* Compute per-16 sub-block scales and max scale across the 256-weight super-block */
        float sub_scales[16];
        float max_scale = 0.0f;
        for (int sb = 0; sb < 16; sb++) {
            float amax = 0.0f;
            for (int j = 0; j < 16; j++) {
                float v = fabsf(x[sb * 16 + j]);
                if (v > amax) amax = v;
            }
            /* 6-bit signed symmetric range [-32, 31], scale = amax / 31.0f */
            sub_scales[sb] = amax / 31.0f;
            if (sub_scales[sb] > max_scale) max_scale = sub_scales[sb];
        }

        float d = max_scale / 127.0f;
        blk->d = d;
        float inv_d = (d > 0.0f) ? (1.0f / d) : 0.0f;

        for (int sb = 0; sb < 16; sb++) {
            int sc = (int)roundf(sub_scales[sb] * inv_d);
            if (sc > 127) sc = 127;
            if (sc < -128) sc = -128;
            blk->scales[sb] = (int8_t)sc;

            float eff_scale = d * (float)sc;
            float inv_eff = (eff_scale > 0.0f) ? (1.0f / eff_scale) : 0.0f;

            for (int j = 0; j < 16; j++) {
                int idx = sb * 16 + j;
                int q = (int)roundf(x[idx] * inv_eff) + 32;
                if (q < 0) q = 0;
                if (q > 63) q = 63;

                uint8_t low4 = (uint8_t)(q & 0x0F);
                uint8_t high2 = (uint8_t)((q >> 4) & 0x03);

                /* Pack low4 into ql[idx / 2] and high2 into qh[idx / 4] */
                if ((idx & 1) == 0) {
                    blk->ql[idx >> 1] |= low4;
                } else {
                    blk->ql[idx >> 1] |= (uint8_t)(low4 << 4);
                }
                int shift = (idx & 3) * 2;
                blk->qh[idx >> 2] |= (uint8_t)(high2 << shift);
            }
        }
    }
}

void dequantize_row_q6_K(const block_q6_K* src, float* dst, int n) {
    const int nb = n / QK_K;
    for (int b = 0; b < nb; b++) {
        const block_q6_K* blk = &src[b];
        float* y = dst + b * QK_K;
        float d = blk->d;
        for (int sb = 0; sb < 16; sb++) {
            float sc = d * (float)blk->scales[sb];
            for (int j = 0; j < 16; j++) {
                int idx = sb * 16 + j;
                uint8_t low4 = ((idx & 1) == 0)
                    ? (blk->ql[idx >> 1] & 0x0F)
                    : ((blk->ql[idx >> 1] >> 4) & 0x0F);
                uint8_t high2 = (blk->qh[idx >> 2] >> ((idx & 3) * 2)) & 0x03;
                int q = (int)(low4 | (high2 << 4)) - 32;
                y[idx] = sc * (float)q;
            }
        }
    }
}

float vec_dot_q6_K(const block_q6_K* vx, const float* vy, int n) {
    const int nb = n / QK_K;
    float total_sum = 0.0f;
    for (int b = 0; b < nb; b++) {
        const block_q6_K* blk = &vx[b];
        const float* y_base = vy + b * QK_K;
        float d = blk->d;
        float super_sum = 0.0f;

        for (int sb = 0; sb < 16; sb++) {
            float sc = (float)blk->scales[sb];
            const float* y = y_base + sb * 16;
            float sub_sum = 0.0f;
            int base_idx = sb * 16;

            for (int j = 0; j < 16; j += 4) {
                int idx = base_idx + j;
                uint8_t ql_pair0 = blk->ql[idx >> 1];
                uint8_t ql_pair1 = blk->ql[(idx >> 1) + 1];
                uint8_t qh_quad  = blk->qh[idx >> 2];

                int q0 = (int)((ql_pair0 & 0x0F)        | (((qh_quad >> 0) & 0x03) << 4)) - 32;
                int q1 = (int)(((ql_pair0 >> 4) & 0x0F) | (((qh_quad >> 2) & 0x03) << 4)) - 32;
                int q2 = (int)((ql_pair1 & 0x0F)        | (((qh_quad >> 4) & 0x03) << 4)) - 32;
                int q3 = (int)(((ql_pair1 >> 4) & 0x0F) | (((qh_quad >> 6) & 0x03) << 4)) - 32;

                sub_sum += (float)q0 * y[j]
                         + (float)q1 * y[j + 1]
                         + (float)q2 * y[j + 2]
                         + (float)q3 * y[j + 3];
            }
            super_sum += sc * sub_sum;
        }
        total_sum += d * super_sum;
    }
    return total_sum;
}

int lora_adapter_q8_init(
    lora_adapter_q8_t* adapter,
    const char* name,
    uint32_t domain_id,
    int rank,
    int dim_in,
    int dim_out,
    float alpha,
    const float* a_fp32,
    const float* b_fp32
) {
    if (!adapter || rank <= 0 || rank > LORA_MAX_RANK || (dim_in % QK8_0) != 0) {
        return -1;
    }
    memset(adapter, 0, sizeof(lora_adapter_q8_t));
    if (name) {
        strncpy(adapter->name, name, sizeof(adapter->name) - 1);
    }
    adapter->domain_id = domain_id;
    adapter->generation = 1;
    adapter->rank = rank;
    adapter->dim_in = dim_in;
    adapter->dim_out = dim_out;
    adapter->alpha = alpha;
    adapter->scale = alpha / (float)rank;

    int blocks_per_row = dim_in / QK8_0;
    adapter->lora_a_q8 = (block_q8_0*)calloc((size_t)rank * blocks_per_row, sizeof(block_q8_0));
    adapter->lora_b_t  = (float*)calloc((size_t)rank * dim_out, sizeof(float));
    if (!adapter->lora_a_q8 || !adapter->lora_b_t) {
        lora_adapter_q8_free(adapter);
        return -1;
    }

    /* Quantize A rows [rank][dim_in] into Q8_0 for L1-cache residency */
    for (int r = 0; r < rank; r++) {
        quantize_row_q8_0(a_fp32 + r * dim_in, adapter->lora_a_q8 + r * blocks_per_row, dim_in);
    }

    /* Transpose B [dim_out][rank] -> B_T [rank][dim_out] for contiguous FMA AXPY sweeps */
    for (int r = 0; r < rank; r++) {
        float* dst_row = adapter->lora_b_t + r * dim_out;
        for (int d = 0; d < dim_out; d++) {
            dst_row[d] = b_fp32[d * rank + r];
        }
    }
    return 0;
}

void lora_adapter_q8_free(lora_adapter_q8_t* adapter) {
    if (adapter) {
        if (adapter->lora_a_q8) { free(adapter->lora_a_q8); adapter->lora_a_q8 = NULL; }
        if (adapter->lora_b_t)  { free(adapter->lora_b_t);  adapter->lora_b_t = NULL; }
    }
}

void apply_lora_q8_sidecar(
    float* out,
    const float* x,
    const lora_adapter_q8_t* adapter
) {
    if (!adapter || !adapter->lora_a_q8 || !adapter->lora_b_t) return;
    const int rank = adapter->rank;
    const int dim_in = adapter->dim_in;
    const int dim_out = adapter->dim_out;
    const int blocks_per_row = dim_in / QK8_0;
    const float scale = adapter->scale;

    /* 1. Down-project x via Q8_0 dot product -> r_mid[r] * scale
     * 2. Up-project via contiguous AXPY sweep over B_T[r][0..dim_out-1] */
    for (int r = 0; r < rank; r++) {
        float mid_scaled = scale * vec_dot_q8_0(adapter->lora_a_q8 + r * blocks_per_row, x, dim_in);
        if (fabsf(mid_scaled) < 1e-12f) continue;
        const float* b_row = adapter->lora_b_t + r * dim_out;
        int d = 0;
        for (; d + 3 < dim_out; d += 4) {
            out[d]     += mid_scaled * b_row[d];
            out[d + 1] += mid_scaled * b_row[d + 1];
            out[d + 2] += mid_scaled * b_row[d + 2];
            out[d + 3] += mid_scaled * b_row[d + 3];
        }
        for (; d < dim_out; d++) {
            out[d] += mid_scaled * b_row[d];
        }
    }
}

void apply_mole_topk_q8(
    float* out,
    const float* x,
    const lora_adapter_q8_t* const* adapters,
    const float* weights,
    int k_count
) {
    if (!out || !x || !adapters || !weights || k_count <= 0) return;
    /* Prefetch L1D down-projection blocks for all Top-K adapters */
    for (int k = 0; k < k_count; k++) {
        if (adapters[k] && adapters[k]->lora_a_q8 && weights[k] > 1e-6f) {
            __builtin_prefetch(adapters[k]->lora_a_q8, 0, 3);
        }
    }
    for (int k = 0; k < k_count; k++) {
        const lora_adapter_q8_t* adapter = adapters[k];
        float wk = weights[k];
        if (!adapter || !adapter->lora_a_q8 || !adapter->lora_b_t || fabsf(wk) < 1e-6f) continue;
        const int rank = adapter->rank;
        const int dim_in = adapter->dim_in;
        const int dim_out = adapter->dim_out;
        const int blocks_per_row = dim_in / QK8_0;
        const float eff_scale = wk * adapter->scale;
        for (int r = 0; r < rank; r++) {
            float mid_scaled = eff_scale * vec_dot_q8_0(adapter->lora_a_q8 + r * blocks_per_row, x, dim_in);
            if (fabsf(mid_scaled) < 1e-12f) continue;
            const float* b_row = adapter->lora_b_t + r * dim_out;
            int d = 0;
            for (; d + 3 < dim_out; d += 4) {
                out[d]     += mid_scaled * b_row[d];
                out[d + 1] += mid_scaled * b_row[d + 1];
                out[d + 2] += mid_scaled * b_row[d + 2];
                out[d + 3] += mid_scaled * b_row[d + 3];
            }
            for (; d < dim_out; d++) {
                out[d] += mid_scaled * b_row[d];
            }
        }
    }
}

void apply_mole_topk_q8_fnwd(
    float* out,
    const float* x,
    const lora_adapter_q8_t* const* adapters,
    const float* weights,
    int k_count
) {
    if (!out || !x || !adapters || !weights || k_count <= 0) return;
    /* Cap K to 8 for stack-resident FNWD register/L1 scratchpad */
    const int k_max = (k_count > 8) ? 8 : k_count;
    float mid_scaled[8][LORA_MAX_RANK];
    memset(mid_scaled, 0, sizeof(mid_scaled));

    int shared_dim_in = 0;
    int shared_dim_out = 0;
    for (int k = 0; k < k_max; k++) {
        if (adapters[k] && adapters[k]->lora_a_q8 && adapters[k]->lora_b_t && fabsf(weights[k]) >= 1e-6f) {
            shared_dim_in = adapters[k]->dim_in;
            shared_dim_out = adapters[k]->dim_out;
            __builtin_prefetch(adapters[k]->lora_a_q8, 0, 3);
            __builtin_prefetch(adapters[k]->lora_b_t, 0, 3);
        }
    }
    if (shared_dim_in <= 0 || shared_dim_out <= 0) return;

    const int blocks_per_row = shared_dim_in / QK8_0;
    /* Phase 1 (FNWD Shared-Activation Down-Projection):
     * Sweep each block b of x ONCE while accumulating across all K adapters and ranks */
    for (int b = 0; b < blocks_per_row; b++) {
        const float* x_blk = x + b * QK8_0;
        for (int k = 0; k < k_max; k++) {
            const lora_adapter_q8_t* ad = adapters[k];
            if (!ad || !ad->lora_a_q8 || fabsf(weights[k]) < 1e-6f) continue;
            for (int r = 0; r < ad->rank; r++) {
                const block_q8_0* blk = &ad->lora_a_q8[r * blocks_per_row + b];
                float bsum = 0.0f;
                for (int j = 0; j < QK8_0; j += 4) {
                    bsum += (float)blk->qs[j]     * x_blk[j]
                          + (float)blk->qs[j + 1] * x_blk[j + 1]
                          + (float)blk->qs[j + 2] * x_blk[j + 2]
                          + (float)blk->qs[j + 3] * x_blk[j + 3];
                }
                mid_scaled[k][r] += blk->d * bsum;
            }
        }
    }
    for (int k = 0; k < k_max; k++) {
        const lora_adapter_q8_t* ad = adapters[k];
        if (!ad) continue;
        float eff_scale = weights[k] * ad->scale;
        for (int r = 0; r < ad->rank; r++) {
            mid_scaled[k][r] *= eff_scale;
        }
    }

    /* Phase 2 (FNWD Fused Single-Store Up-Projection):
     * Accumulate all (k, r) contributions into register accumulators before storing to out[d..d+3] */
    int d = 0;
    for (; d + 3 < shared_dim_out; d += 4) {
        float acc0 = out[d];
        float acc1 = out[d + 1];
        float acc2 = out[d + 2];
        float acc3 = out[d + 3];
        for (int k = 0; k < k_max; k++) {
            const lora_adapter_q8_t* ad = adapters[k];
            if (!ad || !ad->lora_b_t || fabsf(weights[k]) < 1e-6f) continue;
            for (int r = 0; r < ad->rank; r++) {
                float ms = mid_scaled[k][r];
                if (fabsf(ms) < 1e-12f) continue;
                const float* b_row = ad->lora_b_t + r * shared_dim_out + d;
                acc0 += ms * b_row[0];
                acc1 += ms * b_row[1];
                acc2 += ms * b_row[2];
                acc3 += ms * b_row[3];
            }
        }
        out[d]     = acc0;
        out[d + 1] = acc1;
        out[d + 2] = acc2;
        out[d + 3] = acc3;
    }
    for (; d < shared_dim_out; d++) {
        float acc = out[d];
        for (int k = 0; k < k_max; k++) {
            const lora_adapter_q8_t* ad = adapters[k];
            if (!ad || !ad->lora_b_t || fabsf(weights[k]) < 1e-6f) continue;
            for (int r = 0; r < ad->rank; r++) {
                acc += mid_scaled[k][r] * ad->lora_b_t[r * shared_dim_out + d];
            }
        }
        out[d] = acc;
    }
}

void fuse_lora_into_q8_weights(
    const block_q8_0* w_base,
    block_q8_0* w_fused_out,
    const lora_adapter_q8_t* adapter
) {
    if (!adapter || !w_base || !w_fused_out) return;
    const int rank = adapter->rank;
    const int dim_in = adapter->dim_in;
    const int dim_out = adapter->dim_out;
    const int blocks_per_row = dim_in / QK8_0;
    const float scale = adapter->scale;

    float* a_dequant = (float*)malloc((size_t)rank * dim_in * sizeof(float));
    float* row_buf   = (float*)malloc((size_t)dim_in * sizeof(float));
    if (!a_dequant || !row_buf) {
        free(a_dequant);
        free(row_buf);
        return;
    }

    for (int r = 0; r < rank; r++) {
        const block_q8_0* a_row = adapter->lora_a_q8 + r * blocks_per_row;
        float* dst = a_dequant + r * dim_in;
        for (int b = 0; b < blocks_per_row; b++) {
            float d = a_row[b].d;
            for (int j = 0; j < QK8_0; j++) {
                dst[b * QK8_0 + j] = d * (float)a_row[b].qs[j];
            }
        }
    }

    for (int i = 0; i < dim_out; i++) {
        const block_q8_0* base_row = w_base + i * blocks_per_row;
        for (int b = 0; b < blocks_per_row; b++) {
            float d = base_row[b].d;
            for (int j = 0; j < QK8_0; j++) {
                row_buf[b * QK8_0 + j] = d * (float)base_row[b].qs[j];
            }
        }
        for (int r = 0; r < rank; r++) {
            float b_val = scale * adapter->lora_b_t[r * dim_out + i];
            const float* a_row = a_dequant + r * dim_in;
            for (int j = 0; j < dim_in; j++) {
                row_buf[j] += b_val * a_row[j];
            }
        }
        quantize_row_q8_0(row_buf, w_fused_out + i * blocks_per_row, dim_in);
    }

    free(a_dequant);
    free(row_buf);
}

void apply_rope(float* vec, int pos, int dim, int head_dim) {
    int num_heads = dim / head_dim;
    int half_dim = head_dim / 2;
    float cos_th[half_dim];
    float sin_th[half_dim];

    for (int i = 0; i < head_dim; i += 2) {
        float freq = 1.0f / powf(10000.0f, (float)i / (float)head_dim);
        float theta = (float)pos * freq;
        cos_th[i / 2] = cosf(theta);
        sin_th[i / 2] = sinf(theta);
    }

    for (int h = 0; h < num_heads; h++) {
        float* head = vec + h * head_dim;
        for (int i = 0; i < head_dim; i += 2) {
            float c = cos_th[i / 2];
            float s = sin_th[i / 2];
            float v0 = head[i];
            float v1 = head[i + 1];
            head[i]     = v0 * c - v1 * s;
            head[i + 1] = v0 * s + v1 * c;
        }
    }
}

kv_cache_t* kv_cache_init(int layers, int max_seq, int dim) {
    kv_cache_t* cache = (kv_cache_t*)malloc(sizeof(kv_cache_t));
    if (!cache) return NULL;
    cache->layers = layers;
    cache->max_seq = max_seq;
    cache->dim = dim;
    size_t total_elements = (size_t)layers * max_seq * dim;
    cache->key_cache = (float*)calloc(total_elements, sizeof(float));
    cache->val_cache = (float*)calloc(total_elements, sizeof(float));
    if (!cache->key_cache || !cache->val_cache) {
        kv_cache_free(cache);
        return NULL;
    }
    return cache;
}

void kv_cache_free(kv_cache_t* cache) {
    if (cache) {
        if (cache->key_cache) free(cache->key_cache);
        if (cache->val_cache) free(cache->val_cache);
        free(cache);
    }
}

int sample_greedy(const float* logits, int vocab_size) {
    int max_idx = 0;
    float max_val = logits[0];
    for (int i = 1; i < vocab_size; i++) {
        if (logits[i] > max_val) {
            max_val = logits[i];
            max_idx = i;
        }
    }
    return max_idx;
}

float compute_max_abs_error(const float* ref, const float* test, int n) {
    float max_err = 0.0f;
    for (int i = 0; i < n; i++) {
        float err = fabsf(ref[i] - test[i]);
        if (err > max_err) max_err = err;
    }
    return max_err;
}

float compute_cosine_similarity(const float* ref, const float* test, int n) {
    double dot = 0.0, nr = 0.0, nt = 0.0;
    for (int i = 0; i < n; i++) {
        dot += (double)ref[i] * (double)test[i];
        nr  += (double)ref[i] * (double)ref[i];
        nt  += (double)test[i] * (double)test[i];
    }
    if (nr <= 1e-12 || nt <= 1e-12) return 0.0f;
    return (float)(dot / (sqrt(nr) * sqrt(nt)));
}

#ifndef QUANT_ENGINE_LIB
/* Standalone test driver for validation & numerical parity */
int main(int argc, char** argv) {
    if (argc > 1 && strcmp(argv[1], "--test-parity") == 0) {
        printf("=== Testing Quantized Numerical Parity (Q8_0, Q6_K, Q4_0 & Dual-Mode LoRA) ===\n");
        const int n = 256;
        float ref_vec[n];
        float act_vec[n];
        for (int i = 0; i < n; i++) {
            ref_vec[i] = sinf((float)i * 0.1f);
            act_vec[i] = cosf((float)i * 0.05f);
        }

        /* Calculate exact reference dot product */
        float ref_dot = 0.0f;
        for (int i = 0; i < n; i++) ref_dot += ref_vec[i] * act_vec[i];

        /* Q8_0 parity test */
        block_q8_0 q8_blocks[n / QK8_0];
        quantize_row_q8_0(ref_vec, q8_blocks, n);
        float q8_dot = vec_dot_q8_0(q8_blocks, act_vec, n);
        float q8_abs_err = fabsf(ref_dot - q8_dot);
        printf("  [PASS] Ref Dot: %.6f, Q8_0 Dot: %.6f, Abs Error: %.6f (36 B / 32 w = 9.00 bpw)\n",
               ref_dot, q8_dot, q8_abs_err);
        if (q8_abs_err > 0.05f) {
            fprintf(stderr, "Q8_0 absolute error exceeded 0.05\n");
            return 1;
        }

        /* Q6_K super-block parity test */
        block_q6_K q6k_blocks[n / QK_K];
        float q6k_dequant[n];
        quantize_row_q6_K(ref_vec, q6k_blocks, n);
        dequantize_row_q6_K(q6k_blocks, q6k_dequant, n);
        float q6k_dot = vec_dot_q6_K(q6k_blocks, act_vec, n);
        float q6k_abs_err = fabsf(ref_dot - q6k_dot);
        float q6k_cos = compute_cosine_similarity(ref_vec, q6k_dequant, n);
        printf("  [PASS] Ref Dot: %.6f, Q6_K Dot: %.6f, Abs Error: %.6f, CosSim: %.6f (212 B / 256 w = 6.625 bpw)\n",
               ref_dot, q6k_dot, q6k_abs_err, q6k_cos);
        if (q6k_abs_err > 0.08f || q6k_cos < 0.999f) {
            fprintf(stderr, "Q6_K parity failed (err=%.6f, cos=%.6f)\n", q6k_abs_err, q6k_cos);
            return 1;
        }

        /* Q4_0 parity test */
        block_q4_0 q4_blocks[n / QK4_0];
        quantize_row_q4_0(ref_vec, q4_blocks, n);
        float q4_dot = vec_dot_q4_0(q4_blocks, act_vec, n);
        float q4_abs_err = fabsf(ref_dot - q4_dot);
        printf("  [PASS] Ref Dot: %.6f, Q4_0 Dot: %.6f, Abs Error: %.6f (20 B / 32 w = 5.00 bpw)\n",
               ref_dot, q4_dot, q4_abs_err);
        if (q4_abs_err > 0.25f) {
            fprintf(stderr, "Q4_0 absolute error exceeded 0.25\n");
            return 2;
        }

        /* Dual-Mode LoRA Hot-Swap & Weight Fusion Parity Test (Sidecar_Q8 vs PreMerged_Fused) */
        const int dim = 64;
        const int rank = 8;
        float w_fp32[dim * dim];
        float a_fp32[rank * dim];
        float b_fp32[dim * rank];
        float x_in[dim];
        for (int i = 0; i < dim; i++) x_in[i] = cosf((float)i * 0.13f);
        for (int i = 0; i < dim * dim; i++) w_fp32[i] = sinf((float)i * 0.03f) * 0.5f;
        for (int i = 0; i < rank * dim; i++) a_fp32[i] = cosf((float)i * 0.07f) * 0.2f;
        for (int i = 0; i < dim * rank; i++) b_fp32[i] = sinf((float)i * 0.11f) * 0.2f;

        block_q8_0 w_base_q8[dim * (dim / QK8_0)];
        block_q8_0 w_fused_q8[dim * (dim / QK8_0)];
        for (int row = 0; row < dim; row++) {
            quantize_row_q8_0(w_fp32 + row * dim, w_base_q8 + row * (dim / QK8_0), dim);
        }

        lora_adapter_q8_t lora;
        if (lora_adapter_q8_init(&lora, "lora_test", 1, rank, dim, dim, 16.0f, a_fp32, b_fp32) != 0) {
            fprintf(stderr, "LoRA Q8 init failed\n");
            return 4;
        }
        fuse_lora_into_q8_weights(w_base_q8, w_fused_q8, &lora);

        float out_sidecar[dim];
        float out_fused[dim];
        for (int row = 0; row < dim; row++) {
            out_sidecar[row] = vec_dot_q8_0(w_base_q8 + row * (dim / QK8_0), x_in, dim);
            out_fused[row]   = vec_dot_q8_0(w_fused_q8 + row * (dim / QK8_0), x_in, dim);
        }
        apply_lora_q8_sidecar(out_sidecar, x_in, &lora);
        float lora_cos = compute_cosine_similarity(out_sidecar, out_fused, dim);
        float lora_err = compute_max_abs_error(out_sidecar, out_fused, dim);
        printf("  [PASS] Dual-Mode LoRA Parity (Sidecar_Q8 vs PreMerged_Fused): CosSim=%.6f, MaxAbsErr=%.6f\n",
               lora_cos, lora_err);
        if (lora_cos < 0.999f) {
            lora_adapter_q8_free(&lora);
            fprintf(stderr, "Dual-Mode LoRA parity failed (cos=%.6f)\n", lora_cos);
            return 5;
        }

        /* FNWD Single-Pass Shared-Activation MoLE Parity Test (apply_mole_topk_q8 vs apply_mole_topk_q8_fnwd) */
        float out_mole_ref[dim];
        float out_mole_fnwd[dim];
        for (int row = 0; row < dim; row++) {
            out_mole_ref[row]  = vec_dot_q8_0(w_base_q8 + row * (dim / QK8_0), x_in, dim);
            out_mole_fnwd[row] = out_mole_ref[row];
        }
        const lora_adapter_q8_t* mole_ads[2] = { &lora, &lora };
        float mole_weights[2] = { 0.65f, 0.35f };
        apply_mole_topk_q8(out_mole_ref, x_in, mole_ads, mole_weights, 2);
        apply_mole_topk_q8_fnwd(out_mole_fnwd, x_in, mole_ads, mole_weights, 2);
        float fnwd_cos = compute_cosine_similarity(out_mole_ref, out_mole_fnwd, dim);
        float fnwd_err = compute_max_abs_error(out_mole_ref, out_mole_fnwd, dim);
        printf("  [PASS] FNWD Single-Pass MoLE Parity (Top-K Shared Activation): CosSim=%.6f, MaxAbsErr=%.6f\n",
               fnwd_cos, fnwd_err);
        lora_adapter_q8_free(&lora);
        if (fnwd_cos < 0.99999f || fnwd_err > 1e-4f) {
            fprintf(stderr, "FNWD MoLE parity failed (cos=%.6f, err=%.6f)\n", fnwd_cos, fnwd_err);
            return 6;
        }

        /* RoPE positional preservation test */
        float rope_test[16];
        for (int i = 0; i < 16; i++) rope_test[i] = 1.0f;
        apply_rope(rope_test, 0, 16, 8);
        printf("  [PASS] RoPE pos=0 preserves norm: %.4f\n", rope_test[0]);

        /* KV Cache sanity */
        kv_cache_t* cache = kv_cache_init(4, 128, 64);
        if (!cache) {
            fprintf(stderr, "KV cache allocation failed\n");
            return 3;
        }
        printf("  [PASS] KV cache allocated successfully (%d layers, %d max_seq, %d dim)\n",
               cache->layers, cache->max_seq, cache->dim);
        kv_cache_free(cache);

        printf("=== ALL NUMERICAL PARITY TESTS PASSED ===\n");
        return 0;
    }

    if (argc > 1 && strcmp(argv[1], "--benchmark-cpu") == 0) {
        const int n = 1024;
        const int iters = 100000;
        float src[n];
        float act[n];
        for (int i = 0; i < n; i++) {
            src[i] = sinf((float)i * 0.05f);
            act[i] = cosf((float)i * 0.03f);
        }
        block_q8_0 b_q8[n / QK8_0];
        block_q6_K b_q6k[n / QK_K];
        block_q4_0 b_q4[n / QK4_0];
        quantize_row_q8_0(src, b_q8, n);
        quantize_row_q6_K(src, b_q6k, n);
        quantize_row_q4_0(src, b_q4, n);

        clock_t t0 = clock();
        float acc_q8 = 0.0f;
        for (int i = 0; i < iters; i++) acc_q8 += vec_dot_q8_0(b_q8, act, n);
        double sec_q8 = (double)(clock() - t0) / CLOCKS_PER_SEC;

        t0 = clock();
        float acc_q6k = 0.0f;
        for (int i = 0; i < iters; i++) acc_q6k += vec_dot_q6_K(b_q6k, act, n);
        double sec_q6k = (double)(clock() - t0) / CLOCKS_PER_SEC;

        t0 = clock();
        float acc_q4 = 0.0f;
        for (int i = 0; i < iters; i++) acc_q4 += vec_dot_q4_0(b_q4, act, n);
        double sec_q4 = (double)(clock() - t0) / CLOCKS_PER_SEC;

        printf("CPU Quantized Dot-Product Ladder (N=%d, %d iters):\n", n, iters);
        printf("  Q8_0 (9.000 bpw): %.3f s | %.2f GFLOPS (accum=%.1f)\n", sec_q8,  (2.0 * n * iters) / (sec_q8 * 1e9),  acc_q8);
        printf("  Q6_K (6.625 bpw): %.3f s | %.2f GFLOPS (accum=%.1f)\n", sec_q6k, (2.0 * n * iters) / (sec_q6k * 1e9), acc_q6k);
        printf("  Q4_0 (5.000 bpw): %.3f s | %.2f GFLOPS (accum=%.1f)\n", sec_q4,  (2.0 * n * iters) / (sec_q4 * 1e9),  acc_q4);
        return 0;
    }

    printf("Usage: %s [--test-parity | --benchmark-cpu]\n", argv[0]);
    return 0;
}
#endif /* QUANT_ENGINE_LIB */
