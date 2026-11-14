/* tools/inspect_gguf.c - Native C GGUF File Inspector (Zero Python) */

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
        uint64_t len;
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

int main(int argc, char** argv) {
    if (argc < 2) {
        printf("Usage: %s <model.gguf>\n", argv[0]);
        return 1;
    }

    FILE* f = fopen(argv[1], "rb");
    if (!f) {
        perror("Failed to open file");
        return 1;
    }

    uint32_t magic = 0;
    uint32_t version = 0;
    uint64_t tensor_count = 0;
    uint64_t kv_count = 0;

    fread(&magic, sizeof(uint32_t), 1, f);
    if (magic != GGUF_MAGIC) {
        printf("Error: Not a valid GGUF file! Magic=0x%08x\n", magic);
        fclose(f);
        return 1;
    }

    fread(&version, sizeof(uint32_t), 1, f);
    fread(&tensor_count, sizeof(uint64_t), 1, f);
    fread(&kv_count, sizeof(uint64_t), 1, f);

    printf("=== GGUF Header ===\n");
    printf("Version: %u\n", version);
    printf("Tensor Count: %lu\n", (unsigned long)tensor_count);
    printf("KV Metadata Count: %lu\n", (unsigned long)kv_count);

    char key_buf[256];
    for (uint64_t i = 0; i < kv_count; i++) {
        read_str(f, key_buf, sizeof(key_buf));
        uint32_t vtype = 0;
        fread(&vtype, sizeof(uint32_t), 1, f);

        if (vtype == GGUF_TYPE_STRING) {
            char sval[256];
            read_str(f, sval, sizeof(sval));
            printf("  [META] %s = '%s'\n", key_buf, sval);
        } else if (vtype == GGUF_TYPE_UINT32) {
            uint32_t uval;
            fread(&uval, sizeof(uint32_t), 1, f);
            printf("  [META] %s = %u\n", key_buf, uval);
        } else if (vtype == GGUF_TYPE_INT32) {
            int32_t ival;
            fread(&ival, sizeof(int32_t), 1, f);
            printf("  [META] %s = %d\n", key_buf, ival);
        } else if (vtype == GGUF_TYPE_UINT64) {
            uint64_t uval;
            fread(&uval, sizeof(uint64_t), 1, f);
            printf("  [META] %s = %lu\n", key_buf, (unsigned long)uval);
        } else if (vtype == GGUF_TYPE_FLOAT32) {
            float fval;
            fread(&fval, sizeof(float), 1, f);
            printf("  [META] %s = %f\n", key_buf, fval);
        } else {
            printf("  [META] %s (type=%u)\n", key_buf, vtype);
            skip_val(f, vtype);
        }
    }

    printf("\n=== Last Tensors ===\n");
    for (uint64_t i = 0; i < tensor_count; i++) {
        char tname[256];
        read_str(f, tname, sizeof(tname));
        uint32_t n_dims = 0;
        fread(&n_dims, sizeof(uint32_t), 1, f);
        uint64_t dims[4] = {0};
        for (uint32_t d = 0; d < n_dims && d < 4; d++) {
            fread(&dims[d], sizeof(uint64_t), 1, f);
        }
        uint32_t ttype = 0;
        fread(&ttype, sizeof(uint32_t), 1, f);
        uint64_t offset = 0;
        fread(&offset, sizeof(uint64_t), 1, f);

        printf("  Tensor %lu: '%s' dims=[%lu, %lu] type=%u offset=%lu\n",
               (unsigned long)i, tname, (unsigned long)dims[0], (unsigned long)dims[1], ttype, (unsigned long)offset);
    }
    long meta_end = ftell(f);
    long data_start = (meta_end + 31) & ~31;
    printf("\n=== Data Offset ===\n");
    printf("Metadata End: %ld, Data Start: %ld (alignment 32)\n", meta_end, data_start);
    fclose(f);
    return 0;
}
