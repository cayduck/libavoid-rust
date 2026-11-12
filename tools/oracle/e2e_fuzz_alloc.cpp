// Bump allocator for the end-to-end fuzzer (tools/oracle/e2e_fuzz.cpp).
//
// As tools/oracle/bump_alloc.cpp: addresses are never reused and every
// block is a multiple of a stride that is a multiple of sizeof(Avoid::Node)
// and of 16, so pointer order == allocation order (the creation-serial
// model of the Rust port) and scanline.cpp's `(int)(ea->v - eb->v)` stays
// an exact division.
//
// Memory: addresses are never reused, but physical memory is.  Every block
// carries a one-stride header with its size; a per-page count of live
// blocks is kept, and a page (below the bump pointer) whose blocks have all
// been freed is returned to the kernel with madvise(MADV_DONTNEED).  RSS
// therefore tracks live memory, while the virtual arena (E2E_ARENA_MB,
// default 16384, MAP_NORESERVE) only bounds the total ever allocated by
// one scene; exhausting it exits with code 9 ("ERROR: e2e memory cap"),
// classified as "#exclude memory".  Every scene runs in its own forked
// child, so the arena dies with the scene.
//
// (Freed memory therefore reads as zeros, not as the dead object, once its
// page is released.  Upstream reads of freed objects are use-after-free,
// which the ASan build reports, so such scenes are excluded anyway.)
//
// The ASan/UBSan variant (-DE2E_SANITIZE) does not use this allocator: it
// runs on ASan's own malloc (with a small quarantine), so its pointer order
// -- and possibly its execution -- can differ from the bump builds; it is
// only used to detect undefined behaviour.
#ifndef E2E_SANITIZE
#include <cstdlib>
#include <cstdint>
#include <cstring>
#include <new>
#include <sys/mman.h>
#include <unistd.h>
#include "libavoid/libavoid.h"
#include "libavoid/scanline.h"

static char *g_arena = nullptr;
static size_t g_arena_off = 0;
static size_t g_arena_size = 0;
static uint32_t *g_page_live = nullptr;   // live blocks per page
static const size_t kPage = 4096;
static const size_t kStride = (sizeof(Avoid::Node) % 16 == 0) ?
        sizeof(Avoid::Node) : 2 * sizeof(Avoid::Node);

static void capExceeded(void)
{
    static const char msg[] = "ERROR: e2e memory cap (bump arena exhausted)\n";
    (void) !write(2, msg, sizeof(msg) - 1);
    _exit(9);
}

static void *oracleAlloc(size_t n)
{
    if (!g_arena)
    {
        const char *mb = getenv("E2E_ARENA_MB");
        g_arena_size = (size_t) (mb ? atol(mb) : 16384) << 20;
        g_arena = (char *) mmap(nullptr, g_arena_size, PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
        g_page_live = (uint32_t *) mmap(nullptr, (g_arena_size / kPage) * sizeof(uint32_t),
                PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
        if (g_arena == MAP_FAILED || g_page_live == MAP_FAILED) abort();
    }
    size_t rounded = ((n + kStride - 1) / kStride) * kStride;
    if (rounded == 0) rounded = kStride;
    size_t total = rounded + kStride;   // header + payload
    if (g_arena_off + total > g_arena_size) capExceeded();
    char *base = g_arena + g_arena_off;
    g_arena_off += total;
    *(size_t *) base = total;
    size_t p0 = (size_t) (base - g_arena) / kPage;
    size_t p1 = (size_t) (base + total - 1 - g_arena) / kPage;
    for (size_t p = p0; p <= p1; ++p) ++g_page_live[p];
    return base + kStride;
}

static void oracleFree(void *ptr)
{
    if (!ptr) return;
    char *base = (char *) ptr - kStride;
    size_t total = *(size_t *) base;
    size_t p0 = (size_t) (base - g_arena) / kPage;
    size_t p1 = (size_t) (base + total - 1 - g_arena) / kPage;
    // The page holding the bump pointer may still get new blocks.
    size_t active = g_arena_off / kPage;
    for (size_t p = p0; p <= p1; ++p)
    {
        if (--g_page_live[p] == 0 && p < active)
        {
            madvise(g_arena + p * kPage, kPage, MADV_DONTNEED);
        }
    }
}

void *operator new(size_t n) { return oracleAlloc(n); }
void *operator new[](size_t n) { return oracleAlloc(n); }
void operator delete(void *p) noexcept { oracleFree(p); }
void operator delete[](void *p) noexcept { oracleFree(p); }
void operator delete(void *p, size_t) noexcept { oracleFree(p); }
void operator delete[](void *p, size_t) noexcept { oracleFree(p); }
#endif
