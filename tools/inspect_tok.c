#include <stdio.h>
#include <stdlib.h>

int main(int argc, char** argv) {
    FILE* f = fopen("models/pretrained/smollm2_tokenizer.bin", "rb");
    if (!f) return 1;
    int max_len;
    if (fread(&max_len, sizeof(int), 1, f) != 1) return 1;
    printf("max_len = %d\n", max_len);
    for (int i = 0; i < 49152; i++) {
        float score;
        int len;
        if (fread(&score, sizeof(float), 1, f) != 1) break;
        if (fread(&len, sizeof(int), 1, f) != 1) break;
        char buf[256];
        if (len > 0 && len < 255) {
            fread(buf, 1, len, f);
            buf[len] = '\0';
        } else {
            buf[0] = '\0';
        }
        if (len > 1 && strstr("assistant", buf) != NULL) {
            printf("Token %5d: '%s'\n", i, buf);
        }
    }
    fclose(f);
    return 0;
}
