/* tools/incremental_hasher.c - Incremental Content-Addressed Hasher with Working Cache and Full Audit Mode
 *
 * Designed for INOS test harnesses and routine development iterations:
 * - Working cache indexed by (path, size, mtime_sec, mtime_nsec) avoids re-reading unchanged files.
 * - Working cache is kept strictly distinct from sealed/immutable acceptance manifests.
 * - Atomic cache updates via write-to-temp and rename() protect against interrupted runs.
 * - Full audit mode (--audit) bypasses cache and cryptographically revalidates 100% of bytes against recorded digests.
 * - Self-contained standard FIPS 180-4 SHA-256 implementation with zero external dependencies.
 */

#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <unistd.h>
#include <fcntl.h>
#include <errno.h>

#define MAX_ENTRIES 4096
#define MAX_PATH 1024
#define BUFFER_SIZE (64 * 1024)

/* --- Pure C FIPS 180-4 SHA-256 Implementation --- */

typedef struct {
    uint32_t state[8];
    uint64_t count;
    uint8_t buffer[64];
} SHA256_CTX;

static const uint32_t K256[64] = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
};

#define ROTR(x, n) (((x) >> (n)) | ((x) << (32 - (n))))
#define CH(x, y, z) (((x) & (y)) ^ (~(x) & (z)))
#define MAJ(x, y, z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define SIG0(x) (ROTR(x, 2) ^ ROTR(x, 13) ^ ROTR(x, 22))
#define SIG1(x) (ROTR(x, 6) ^ ROTR(x, 11) ^ ROTR(x, 25))
#define THETA0(x) (ROTR(x, 7) ^ ROTR(x, 18) ^ ((x) >> 3))
#define THETA1(x) (ROTR(x, 17) ^ ROTR(x, 19) ^ ((x) >> 10))

static void sha256_init(SHA256_CTX *ctx) {
    ctx->state[0] = 0x6a09e667;
    ctx->state[1] = 0xbb67ae85;
    ctx->state[2] = 0x3c6ef372;
    ctx->state[3] = 0xa54ff53a;
    ctx->state[4] = 0x510e527f;
    ctx->state[5] = 0x9b05688c;
    ctx->state[6] = 0x1f83d9ab;
    ctx->state[7] = 0x5be0cd19;
    ctx->count = 0;
}

static void sha256_transform(SHA256_CTX *ctx, const uint8_t data[64]) {
    uint32_t W[64];
    for (int i = 0; i < 16; i++) {
        W[i] = ((uint32_t)data[i * 4] << 24) |
               ((uint32_t)data[i * 4 + 1] << 16) |
               ((uint32_t)data[i * 4 + 2] << 8) |
               ((uint32_t)data[i * 4 + 3]);
    }
    for (int i = 16; i < 64; i++) {
        W[i] = THETA1(W[i - 2]) + W[i - 7] + THETA0(W[i - 15]) + W[i - 16];
    }

    uint32_t a = ctx->state[0], b = ctx->state[1], c = ctx->state[2], d = ctx->state[3];
    uint32_t e = ctx->state[4], f = ctx->state[5], g = ctx->state[6], h = ctx->state[7];

    for (int i = 0; i < 64; i++) {
        uint32_t t1 = h + SIG1(e) + CH(e, f, g) + K256[i] + W[i];
        uint32_t t2 = SIG0(a) + MAJ(a, b, c);
        h = g;
        g = f;
        f = e;
        e = d + t1;
        d = c;
        c = b;
        b = a;
        a = t1 + t2;
    }

    ctx->state[0] += a;
    ctx->state[1] += b;
    ctx->state[2] += c;
    ctx->state[3] += d;
    ctx->state[4] += e;
    ctx->state[5] += f;
    ctx->state[6] += g;
    ctx->state[7] += h;
}

static void sha256_update(SHA256_CTX *ctx, const uint8_t *data, size_t len) {
    for (size_t i = 0; i < len; i++) {
        size_t idx = (ctx->count % 64);
        ctx->buffer[idx] = data[i];
        ctx->count++;
        if (ctx->count % 64 == 0) {
            sha256_transform(ctx, ctx->buffer);
        }
    }
}

static void sha256_final(SHA256_CTX *ctx, uint8_t hash[32]) {
    uint64_t total_bits = ctx->count * 8;
    sha256_update(ctx, (const uint8_t *)"\x80", 1);
    while ((ctx->count % 64) != 56) {
        sha256_update(ctx, (const uint8_t *)"\x00", 1);
    }
    for (int i = 7; i >= 0; i--) {
        uint8_t byte = (total_bits >> (i * 8)) & 0xff;
        sha256_update(ctx, &byte, 1);
    }
    for (int i = 0; i < 8; i++) {
        hash[i * 4] = (ctx->state[i] >> 24) & 0xff;
        hash[i * 4 + 1] = (ctx->state[i] >> 16) & 0xff;
        hash[i * 4 + 2] = (ctx->state[i] >> 8) & 0xff;
        hash[i * 4 + 3] = (ctx->state[i]) & 0xff;
    }
}

static void hash_to_hex(const uint8_t hash[32], char hex[65]) {
    static const char hex_chars[] = "0123456789abcdef";
    for (int i = 0; i < 32; i++) {
        hex[i * 2] = hex_chars[(hash[i] >> 4) & 0x0f];
        hex[i * 2 + 1] = hex_chars[hash[i] & 0x0f];
    }
    hex[64] = '\0';
}

static int hash_file(const char *path, char hex_out[65], uint64_t *bytes_read_out) {
    int fd = open(path, O_RDONLY);
    if (fd < 0) return -1;

    SHA256_CTX ctx;
    sha256_init(&ctx);

    uint8_t *buf = malloc(BUFFER_SIZE);
    if (!buf) { close(fd); return -1; }

    uint64_t total = 0;
    ssize_t n;
    while ((n = read(fd, buf, BUFFER_SIZE)) > 0) {
        sha256_update(&ctx, buf, (size_t)n);
        total += (uint64_t)n;
    }
    free(buf);
    close(fd);

    if (n < 0) return -1;

    uint8_t digest[32];
    sha256_final(&ctx, digest);
    hash_to_hex(digest, hex_out);
    if (bytes_read_out) *bytes_read_out = total;
    return 0;
}

/* --- Metadata Cache Table --- */

typedef struct {
    char path[MAX_PATH];
    uint64_t size;
    uint64_t mtime_sec;
    uint64_t mtime_nsec;
    char sha256_hex[65];
    int touched;
} CacheEntry;

static CacheEntry g_cache[MAX_ENTRIES];
static size_t g_cache_count = 0;

static void load_cache(const char *cache_file) {
    g_cache_count = 0;
    FILE *f = fopen(cache_file, "r");
    if (!f) return;

    char line[MAX_PATH + 128];
    while (fgets(line, sizeof(line), f)) {
        if (line[0] == '#' || line[0] == '\n') continue;
        char p[MAX_PATH];
        uint64_t sz, m_sec, m_nsec;
        char h[65];
        if (sscanf(line, "%1023[^\t]\t%lu\t%lu\t%lu\t%64s", p, &sz, &m_sec, &m_nsec, h) == 5) {
            if (g_cache_count < MAX_ENTRIES) {
                size_t plen = strlen(p);
                if (plen >= MAX_PATH) plen = MAX_PATH - 1;
                memcpy(g_cache[g_cache_count].path, p, plen);
                g_cache[g_cache_count].path[plen] = '\0';
                g_cache[g_cache_count].size = sz;
                g_cache[g_cache_count].mtime_sec = m_sec;
                g_cache[g_cache_count].mtime_nsec = m_nsec;
                memcpy(g_cache[g_cache_count].sha256_hex, h, 64);
                g_cache[g_cache_count].sha256_hex[64] = '\0';
                g_cache[g_cache_count].touched = 0;
                g_cache_count++;
            }
        }
    }
    fclose(f);
}

static int save_cache_atomic(const char *cache_file) {
    char tmp_file[MAX_PATH + 32];
    snprintf(tmp_file, sizeof(tmp_file), "%s.tmp.%d", cache_file, (int)getpid());

    FILE *f = fopen(tmp_file, "w");
    if (!f) return -1;

    fprintf(f, "# INOS Incremental Hasher Working Cache - Generated at runtime\n");
    fprintf(f, "# PATH\tSIZE\tMTIME_SEC\tMTIME_NSEC\tSHA256\n");
    for (size_t i = 0; i < g_cache_count; i++) {
        fprintf(f, "%s\t%lu\t%lu\t%lu\t%s\n",
                g_cache[i].path,
                g_cache[i].size,
                g_cache[i].mtime_sec,
                g_cache[i].mtime_nsec,
                g_cache[i].sha256_hex);
    }
    fflush(f);
    int fd = fileno(f);
    if (fd >= 0) fsync(fd);
    fclose(f);

    if (rename(tmp_file, cache_file) != 0) {
        unlink(tmp_file);
        return -1;
    }
    return 0;
}

static CacheEntry *find_cache_entry(const char *path) {
    for (size_t i = 0; i < g_cache_count; i++) {
        if (strcmp(g_cache[i].path, path) == 0) {
            return &g_cache[i];
        }
    }
    return NULL;
}

static void update_or_add_cache(const char *path, uint64_t sz, uint64_t m_sec, uint64_t m_nsec, const char *hex) {
    CacheEntry *e = find_cache_entry(path);
    if (e) {
        e->size = sz;
        e->mtime_sec = m_sec;
        e->mtime_nsec = m_nsec;
        memcpy(e->sha256_hex, hex, 64);
        e->sha256_hex[64] = '\0';
        e->touched = 1;
    } else if (g_cache_count < MAX_ENTRIES) {
        size_t plen = strlen(path);
        if (plen >= MAX_PATH) plen = MAX_PATH - 1;
        memcpy(g_cache[g_cache_count].path, path, plen);
        g_cache[g_cache_count].path[plen] = '\0';
        g_cache[g_cache_count].size = sz;
        g_cache[g_cache_count].mtime_sec = m_sec;
        g_cache[g_cache_count].mtime_nsec = m_nsec;
        memcpy(g_cache[g_cache_count].sha256_hex, hex, 64);
        g_cache[g_cache_count].sha256_hex[64] = '\0';
        g_cache[g_cache_count].touched = 1;
        g_cache_count++;
    }
}

static uint64_t current_time_us(void) {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return (uint64_t)tv.tv_sec * 1000000ULL + (uint64_t)tv.tv_usec;
}

static void print_usage(const char *prog) {
    fprintf(stderr, "Usage: %s [options] <file1> [file2 ...]\n", prog);
    fprintf(stderr, "Options:\n");
    fprintf(stderr, "  --cache <path>     Working cache file path (default: .evidence_cache.tsv)\n");
    fprintf(stderr, "  --manifest <path>  Write output in sha256sum format to manifest file\n");
    fprintf(stderr, "  --audit <manifest> Full cryptographic audit mode: re-read 100%% of bytes and check against manifest\n");
    fprintf(stderr, "  --timing           Output microsecond execution timing and cache hit metrics\n");
    fprintf(stderr, "  --help             Show this help\n");
}

int main(int argc, char *argv[]) {
    const char *cache_path = ".evidence_cache.tsv";
    const char *manifest_out = NULL;
    const char *audit_manifest = NULL;
    int show_timing = 0;

    int file_start = 1;
    while (file_start < argc && argv[file_start][0] == '-') {
        if (strcmp(argv[file_start], "--cache") == 0 && file_start + 1 < argc) {
            cache_path = argv[++file_start];
            file_start++;
        } else if (strcmp(argv[file_start], "--manifest") == 0 && file_start + 1 < argc) {
            manifest_out = argv[++file_start];
            file_start++;
        } else if (strcmp(argv[file_start], "--audit") == 0 && file_start + 1 < argc) {
            audit_manifest = argv[++file_start];
            file_start++;
        } else if (strcmp(argv[file_start], "--timing") == 0) {
            show_timing = 1;
            file_start++;
        } else if (strcmp(argv[file_start], "--help") == 0) {
            print_usage(argv[0]);
            return 0;
        } else {
            fprintf(stderr, "Unknown option: %s\n", argv[file_start]);
            print_usage(argv[0]);
            return 1;
        }
    }

    uint64_t t_start = current_time_us();

    /* --- MODE 1: FULL CRYPTOGRAPHIC AUDIT --- */
    if (audit_manifest) {
        FILE *mf = fopen(audit_manifest, "r");
        if (!mf) {
            fprintf(stderr, "ERROR: Cannot open audit manifest: %s\n", audit_manifest);
            return 1;
        }

        printf("[AUDIT] Starting full cryptographic verification against: %s\n", audit_manifest);
        char line[MAX_PATH + 128];
        int total = 0, passed = 0, failed = 0;
        uint64_t total_bytes = 0;

        while (fgets(line, sizeof(line), mf)) {
            char exp_hash[65];
            char path[MAX_PATH];
            if (sscanf(line, "%64s  %1023[^\n]", exp_hash, path) == 2 ||
                sscanf(line, "%64s *%1023[^\n]", exp_hash, path) == 2) {
                total++;
                char actual_hash[65];
                uint64_t b = 0;
                char target_path[MAX_PATH * 2];
                size_t plen = strlen(path);
                if (plen >= sizeof(target_path)) plen = sizeof(target_path) - 1;
                memcpy(target_path, path, plen);
                target_path[plen] = '\0';

                if (access(target_path, R_OK) != 0) {
                    /* Try resolving relative to manifest directory */
                    const char *slash = strrchr(audit_manifest, '/');
                    if (!slash) slash = strrchr(audit_manifest, '\\');
                    if (slash) {
                        size_t dirlen = slash - audit_manifest;
                        snprintf(target_path, sizeof(target_path), "%.*s/%s", (int)dirlen, audit_manifest, path);
                    }
                }

                if (hash_file(target_path, actual_hash, &b) != 0) {
                    printf("  [MISSING/UNREADABLE] %s\n", path);
                    failed++;
                } else {
                    total_bytes += b;
                    if (strcmp(exp_hash, actual_hash) == 0) {
                        printf("  [OK] %s\n", path);
                        passed++;
                    } else {
                        printf("  [FAIL - MISMATCH] %s (expected %s, found %s)\n", path, exp_hash, actual_hash);
                        failed++;
                    }
                }
            }
        }
        fclose(mf);

        uint64_t t_end = current_time_us();
        printf("[AUDIT COMPLETE] Verified: %d/%d (Passed: %d, Failed: %d, Read: %lu bytes in %lu us)\n",
               passed, total, passed, failed, total_bytes, t_end - t_start);
        return (failed == 0) ? 0 : 1;
    }

    /* --- MODE 2: INCREMENTAL WORKING CACHE HASHING --- */
    if (file_start >= argc) {
        fprintf(stderr, "ERROR: No target files specified for hashing.\n");
        print_usage(argv[0]);
        return 1;
    }

    load_cache(cache_path);

    FILE *out_fp = stdout;
    if (manifest_out) {
        out_fp = fopen(manifest_out, "w");
        if (!out_fp) {
            fprintf(stderr, "ERROR: Cannot open manifest output: %s\n", manifest_out);
            return 1;
        }
    }

    int cache_hits = 0;
    int cache_misses = 0;
    uint64_t bytes_hashed = 0;
    int cache_modified = 0;

    for (int i = file_start; i < argc; i++) {
        const char *path = argv[i];
        struct stat st;
        if (stat(path, &st) != 0) {
            fprintf(stderr, "WARNING: Cannot stat file: %s (%s)\n", path, strerror(errno));
            continue;
        }

        uint64_t sz = (uint64_t)st.st_size;
        uint64_t m_sec = (uint64_t)st.st_mtime;
#if defined(__APPLE__) || defined(__FreeBSD__)
        uint64_t m_nsec = (uint64_t)st.st_mtimespec.tv_nsec;
#elif defined(_POSIX_C_SOURCE) && (_POSIX_C_SOURCE >= 200809L)
        uint64_t m_nsec = (uint64_t)st.st_mtim.tv_nsec;
#else
        uint64_t m_nsec = 0;
#endif

        CacheEntry *entry = find_cache_entry(path);
        char hex[65];
        if (entry && entry->size == sz && entry->mtime_sec == m_sec && entry->mtime_nsec == m_nsec) {
            /* CACHE HIT: O(1) metadata match */
            strncpy(hex, entry->sha256_hex, 64);
            hex[64] = '\0';
            entry->touched = 1;
            cache_hits++;
        } else {
            /* CACHE MISS: Compute SHA-256 */
            uint64_t b = 0;
            if (hash_file(path, hex, &b) != 0) {
                fprintf(stderr, "ERROR: Failed to hash file: %s\n", path);
                continue;
            }
            bytes_hashed += b;
            update_or_add_cache(path, sz, m_sec, m_nsec, hex);
            cache_modified = 1;
            cache_misses++;
        }

        fprintf(out_fp, "%s  %s\n", hex, path);
    }

    if (manifest_out && out_fp != stdout) {
        fclose(out_fp);
    }

    if (cache_modified) {
        save_cache_atomic(cache_path);
    }

    uint64_t t_end = current_time_us();

    if (show_timing) {
        fprintf(stderr, "[INCREMENTAL HASHER TELEMETRY]\n");
        fprintf(stderr, "  Total Files Checked: %d\n", cache_hits + cache_misses);
        fprintf(stderr, "  Cache Hits (O(1)):    %d\n", cache_hits);
        fprintf(stderr, "  Cache Misses:        %d\n", cache_misses);
        fprintf(stderr, "  Bytes Read & Hashed: %lu\n", bytes_hashed);
        fprintf(stderr, "  Total Duration:      %lu us (~%.2f ms)\n",
                t_end - t_start, (double)(t_end - t_start) / 1000.0);
    }

    return 0;
}
