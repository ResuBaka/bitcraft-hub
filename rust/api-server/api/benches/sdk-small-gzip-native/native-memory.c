// Link against the same bundled libdeflate archive as the replay. Measure its
// requested heap bytes (not malloc usable size), including lifetime and reuse.
#include <libdeflate.h>
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

static size_t allocations, bytes, frees;
static void *allocate(size_t n) { ++allocations; bytes += n; return malloc(n); }
static void release(void *p) { if (p) ++frees; free(p); }

int main(void) {
    const struct libdeflate_options opts = {
        .sizeof_options = sizeof(opts), .malloc_func = allocate, .free_func = release
    };
    struct libdeflate_decompressor *d = libdeflate_alloc_decompressor_ex(&opts);
    assert(d);
    // Empty member: first block final, stored.
    const unsigned char gz[] = {
        31,139,8,0,0,0,0,0,0,255, 1,0,0,255,255, 0,0,0,0,0,0,0,0
    };
    unsigned char out[8191];
    size_t n, setup = allocations;
    for (int i = 0; i < 1000; ++i)
        assert(libdeflate_gzip_decompress(d, gz, sizeof(gz), out, sizeof(out), &n) == LIBDEFLATE_SUCCESS && n == 0);
    libdeflate_free_decompressor(d);
    assert(frees == allocations);
    printf("{\"setup_allocations\":%zu,\"requested_heap_bytes\":%zu,\"decode_allocations\":%zu,\"frees\":%zu}\n",
           setup, bytes, allocations - setup, frees);
}
