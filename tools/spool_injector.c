/* tools/spool_injector.c - Pure C Intent Spool Injector for Dedicated Transport Region (Gate G12) */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

#define SECTOR_SIZE 512
#define SPOOL_LBA 24576
#define SPOOL_OFFSET ((int64_t)SPOOL_LBA * SECTOR_SIZE)
#define SPOOL_MAGIC 0x494E545F

struct __attribute__((packed)) IntentSpoolHeader {
    uint32_t magic;
    uint16_t version;
    uint16_t status;
    uint16_t filename_len;
    char filename[32];
    uint32_t total_content_len;
    uint16_t total_sectors;
    uint8_t provenance_hash[32];
    uint8_t prompt_hash[32];
    uint8_t is_ambiguous;
    uint8_t reserved[399];
};

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "Usage: %s <disk_image> <mode> [filename] [payload_file] [prov_hash]\n", argv[0]);
        fprintf(stderr, "Modes: --concrete, --ambiguous, --malformed\n");
        return 1;
    }

    const char *disk_path = argv[1];
    const char *mode = argv[2];

    FILE *f = fopen(disk_path, "r+b");
    if (!f) {
        perror("fopen disk_image");
        return 1;
    }

    if (fseek(f, SPOOL_OFFSET, SEEK_SET) != 0) {
        perror("fseek to spool LBA");
        fclose(f);
        return 1;
    }

    struct IntentSpoolHeader hdr;
    memset(&hdr, 0, sizeof(hdr));

    if (strcmp(mode, "--malformed") == 0) {
        hdr.magic = 0xDEADBEEF; // Invalid magic
        hdr.version = 1;
        hdr.status = 1;
        fwrite(&hdr, 1, sizeof(hdr), f);
        fclose(f);
        printf("[SPOOL_INJECTOR] Injected malformed packet with bad magic (0xDEADBEEF)\n");
        return 0;
    }

    if (strcmp(mode, "--ambiguous") == 0) {
        hdr.magic = SPOOL_MAGIC;
        hdr.version = 1;
        hdr.status = 1;
        hdr.is_ambiguous = 1;
        const char *msg = "ambiguous_request";
        hdr.filename_len = strlen(msg);
        strncpy(hdr.filename, msg, 32);
        fwrite(&hdr, 1, sizeof(hdr), f);
        fclose(f);
        printf("[SPOOL_INJECTOR] Injected ambiguous instruction packet (is_ambiguous = 1)\n");
        return 0;
    }

    if (strcmp(mode, "--concrete") == 0) {
        if (argc < 5) {
            fprintf(stderr, "Missing arguments for --concrete: filename, payload_file\n");
            fclose(f);
            return 1;
        }
        const char *filename = argv[3];
        const char *payload_file = argv[4];

        FILE *pf = fopen(payload_file, "rb");
        if (!pf) {
            perror("fopen payload_file");
            fclose(f);
            return 1;
        }
        fseek(pf, 0, SEEK_END);
        long p_len = ftell(pf);
        fseek(pf, 0, SEEK_SET);

        if (p_len > 4096) {
            fprintf(stderr, "Payload too large (> 4096 bytes): %ld\n", p_len);
            fclose(pf);
            fclose(f);
            return 1;
        }

        uint8_t payload[4096];
        memset(payload, 0, sizeof(payload));
        fread(payload, 1, p_len, pf);
        fclose(pf);

        hdr.magic = SPOOL_MAGIC;
        hdr.version = 1;
        hdr.status = 1; // Ready
        hdr.filename_len = strlen(filename);
        strncpy(hdr.filename, filename, 32);
        hdr.total_content_len = (uint32_t)p_len;
        hdr.total_sectors = (uint16_t)((p_len + SECTOR_SIZE - 1) / SECTOR_SIZE);
        hdr.is_ambiguous = 0;

        if (argc >= 6) {
            strncpy((char*)hdr.provenance_hash, argv[5], 32);
        }

        fwrite(&hdr, 1, sizeof(hdr), f);

        // Write payload sectors
        for (int s = 0; s < hdr.total_sectors; s++) {
            uint8_t sector[SECTOR_SIZE];
            memset(sector, 0, SECTOR_SIZE);
            int start = s * SECTOR_SIZE;
            int chunk = p_len - start;
            if (chunk > SECTOR_SIZE) chunk = SECTOR_SIZE;
            memcpy(sector, payload + start, chunk);
            fwrite(sector, 1, SECTOR_SIZE, f);
        }

        fclose(f);
        printf("[SPOOL_INJECTOR] Injected concrete packet: file='%s', len=%ld bytes (%d sectors) at LBA %d\n",
               filename, p_len, hdr.total_sectors, SPOOL_LBA);
        return 0;
    }

    fprintf(stderr, "Unknown mode: %s\n", mode);
    fclose(f);
    return 1;
}
