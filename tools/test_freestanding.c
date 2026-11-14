#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/mman.h>

extern int run_ring3_pretrained_inference(void* model_weights_base);

int main(int argc, char** argv) {
    const char* path = argc > 1 ? argv[1] : "models/pretrained/stories15M.bin";
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        perror("open");
        return 1;
    }
    off_t size = lseek(fd, 0, SEEK_END);
    lseek(fd, 0, SEEK_SET);
    void* weights = mmap(NULL, size, PROT_READ, MAP_PRIVATE, fd, 0);
    if (weights == MAP_FAILED) {
        perror("mmap");
        return 1;
    }
    printf("[TEST] Running freestanding transformer on %s (%ld bytes)...\n", path, size);
    int res = run_ring3_pretrained_inference(weights);
    printf("[TEST] Finished with result code: %d\n", res);
    return res;
}
