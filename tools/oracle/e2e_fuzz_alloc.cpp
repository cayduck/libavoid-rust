// Bump allocator for the end-to-end fuzzer (tools/oracle/e2e_fuzz.cpp).
//
// As tools/oracle/bump_alloc.cpp: memory is never reused and every block
// is a multiple of a stride that is a multiple of sizeof(Avoid::Node) and of
// 16, so pointer order == allocation order (the creation-serial model of the
// Rust port) and scanline.cpp's `(int)(ea->v - eb->v)` stays an exact
// division.
//
// Built with -DE2E_SANITIZE (the ASan/UBSan variant), every block gets a
// poisoned header and trailing redzone of one stride each, and `delete`
// poisons the block: AddressSanitizer then reports heap overflows and
// use-after-free while pointer order is still allocation order (so the
// sanitized run executes exactly the same code paths as the NDEBUG run).
#include <cstdlib>
#include <cstring>
#include <new>
#include <sys/mman.h>
#include <unistd.h>
#include "libavoid/libavoid.h"
#include "libavoid/scanline.h"
#ifdef E2E_SANITIZE
#include <sanitizer/asan_interface.h>
#endif

static char *g_arena = nullptr;
static size_t g_arena_off = 0;
static const size_t g_arena_size = (size_t) 1 << 40;
static const size_t kStride = (sizeof(Avoid::Node) % 16 == 0) ?
        sizeof(Avoid::Node) : 2 * sizeof(Avoid::Node);

static void *oracleAlloc(size_t n)
{
    if (!g_arena)
    {
        g_arena = (char *) mmap(nullptr, g_arena_size, PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
        if (g_arena == MAP_FAILED) abort();
    }
    size_t rounded = ((n + kStride - 1) / kStride) * kStride;
    if (rounded == 0) rounded = kStride;
#ifdef E2E_SANITIZE
    // [header stride][payload rounded][redzone stride]
    char *base = g_arena + g_arena_off;
    g_arena_off += rounded + 2 * kStride;
    if (g_arena_off > g_arena_size) abort();
    *(size_t *) base = rounded;
    __asan_poison_memory_region(base, kStride);
    __asan_unpoison_memory_region(base + kStride, n ? n : 1);
    __asan_poison_memory_region(base + kStride + (n ? n : 1),
            rounded - (n ? n : 1) + kStride);
    return base + kStride;
#else
    void *p = g_arena + g_arena_off;
    g_arena_off += rounded;
    if (g_arena_off > g_arena_size) abort();
    return p;
#endif
}

static void oracleFree(void *p)
{
#ifdef E2E_SANITIZE
    if (!p) return;
    char *base = (char *) p - kStride;
    __asan_unpoison_memory_region(base, sizeof(size_t));
    size_t rounded = *(size_t *) base;
    const size_t freed = (size_t) 1 << 63;
    if (rounded & freed)
    {
        static const char msg[] = "ERROR: e2e_fuzz_alloc: double delete\n";
        (void) !write(2, msg, sizeof(msg) - 1);
        abort();
    }
    *(size_t *) base = rounded | freed;
    __asan_poison_memory_region(base, sizeof(size_t));
    __asan_poison_memory_region(p, rounded);
#else
    (void) p;
#endif
}

void *operator new(size_t n) { return oracleAlloc(n); }
void *operator new[](size_t n) { return oracleAlloc(n); }
void operator delete(void *p) noexcept { oracleFree(p); }
void operator delete[](void *p) noexcept { oracleFree(p); }
void operator delete(void *p, size_t) noexcept { oracleFree(p); }
void operator delete[](void *p, size_t) noexcept { oracleFree(p); }
