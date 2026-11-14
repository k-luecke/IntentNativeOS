/* models/adapter/quant_c_stubs.c
 * OCaml C FFI Bridge to Hardware AVX2/FMA `models/inference/quant_engine.c`
 * Executes Q8_0 Block Quantization, AVX2+FMA `vec_dot_q8_0`, Sidecar Q8_0 LoRA,
 * and Single-Pass Shared-Activation FNWD MoLE directly in C registers.
 */

#define CAML_NAME_SPACE
#include <caml/mlvalues.h>
#include <caml/alloc.h>
#include <caml/memory.h>

#include <string.h>
#include <stdlib.h>
#include "quant_engine.h"

CAMLprim value caml_quant_matvec_q8_0(value v_weights, value v_biases, value v_x) {
    CAMLparam3(v_weights, v_biases, v_x);
    CAMLlocal1(v_out);

    int dim_out = Wosize_val(v_weights);
    int dim_in = Wosize_val(v_x) / Double_wosize;
    v_out = caml_alloc(dim_out * Double_wosize, Double_array_tag);

    /* Pad dim_in to multiple of QK8_0 (32) on stack (max 256) */
    int padded_in = ((dim_in + QK8_0 - 1) / QK8_0) * QK8_0;
    if (padded_in <= 0 || padded_in > 512) {
        for (int i = 0; i < dim_out; i++) {
            value v_row = Field(v_weights, i);
            double sum = Double_field(v_biases, i);
            for (int j = 0; j < dim_in; j++) {
                sum += Double_field(v_row, j) * Double_field(v_x, j);
            }
            Store_double_field(v_out, i, sum);
        }
        CAMLreturn(v_out);
    }

    float x_buf[512];
    float row_buf[512];
    block_q8_0 q8_row[512 / QK8_0];
    memset(x_buf, 0, (size_t)padded_in * sizeof(float));
    for (int j = 0; j < dim_in; j++) {
        x_buf[j] = (float)Double_field(v_x, j);
    }

    for (int i = 0; i < dim_out; i++) {
        value v_row = Field(v_weights, i);
        memset(row_buf, 0, (size_t)padded_in * sizeof(float));
        for (int j = 0; j < dim_in; j++) {
            row_buf[j] = (float)Double_field(v_row, j);
        }
        quantize_row_q8_0(row_buf, q8_row, padded_in);
        float dot = vec_dot_q8_0(q8_row, x_buf, padded_in);
        double res = Double_field(v_biases, i) + (double)dot;
        Store_double_field(v_out, i, res);
    }

    CAMLreturn(v_out);
}

CAMLprim value caml_quant_q8_int_row_dot(value v_qrow, value v_act) {
    CAMLparam2(v_qrow, v_act);
    int len_q = Wosize_val(v_qrow);
    int len_a = Wosize_val(v_act) / Double_wosize;
    int n = (len_q < len_a) ? len_q : len_a;
    int padded = ((n + QK8_0 - 1) / QK8_0) * QK8_0;
    if (padded <= 0 || padded > 512) {
        double acc = 0.0;
        for (int j = 0; j < n; j++) {
            acc += (double)Long_val(Field(v_qrow, j)) * Double_field(v_act, j);
        }
        CAMLreturn(caml_copy_double(acc));
    }

    block_q8_0 blk[512 / QK8_0];
    float y_buf[512];
    memset(blk, 0, sizeof(blk));
    memset(y_buf, 0, (size_t)padded * sizeof(float));
    int nb = padded / QK8_0;
    for (int b = 0; b < nb; b++) {
        blk[b].d = 1.0f;
        for (int j = 0; j < QK8_0; j++) {
            int idx = b * QK8_0 + j;
            if (idx < n) {
                int qv = (int)Long_val(Field(v_qrow, idx));
                if (qv > 127) qv = 127;
                if (qv < -128) qv = -128;
                blk[b].qs[j] = (int8_t)qv;
                y_buf[idx] = (float)Double_field(v_act, idx);
            }
        }
    }
    float dot = vec_dot_q8_0(blk, y_buf, padded);
    CAMLreturn(caml_copy_double((double)dot));
}
