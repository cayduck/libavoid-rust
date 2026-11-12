// Bump allocator for the end-to-end fuzzer (tools/oracle/e2e_fuzz.cpp).
//
// As tools/oracle/bump_alloc.cpp: memory is never reused and every block
// is a multiple of a stride that is a multiple of sizeof(Avoid::Node) and of
// 16, so pointer order == allocation order (the creation-serial model of the
// Rust port) and scanline.cpp's `(int)(ea->v - eb->v)` stays an exact
// division.
//
// Memory cap: the arena is E2E_ARENA_MB megabytes (default 1024).  Every
// scene runs in its own forked child, so the arena dies with the scene;
// since the allocator never frees, a scene that allocates more than the cap
// exits with code 9 ("ERROR: e2e memory cap"), which the driver classifies
// as "#exclude memory".  The mapping is MAP_NORESERVE, so only touched
// pages count towards RSS.
//
// The ASan/UBSan variant (-DE2E_SANITIZE) does not use this allocator: it
// runs on ASan's own malloc (with a small quarantine), so its pointer order
// -- and possibly its execution -- can differ from the bump builds; it is
// only used to detect undefined behaviour.
#ifndef E2E_SANITIZE
#include <cstdlib>
#include <cstring>
#include <new>
#include <sys/mman.h>
#include <unistd.h>
#include "libavoid/libavoid.h"
#include "libavoid/scanline.h"

static char *g_arena = nullptr;
static size_t g_arena_off = 0;
static size_t g_arena_size = 0;
static const size_t kStride = (sizeof(Avoid::Node) % 16 == 0) ?
        sizeof(Avoid::Node) : 2 * sizeof(Avoid::Node);

static void *oracleAlloc(size_t n)
{
    if (!g_arena)
    {
        const char *mb = getenv("E2E_ARENA_MB");
        g_arena_size = (size_t) (mb ? atol(mb) : 1024) << 20;
        g_arena = (char *) mmap(nullptr, g_arena_size, PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
        if (g_arena == MAP_FAILED) abort();
    }
    size_t rounded = ((n + kStride - 1) / kStride) * kStride;
    if (rounded == 0) rounded = kStride;
    if (g_arena_off + rounded > g_arena_size)
    {
        static const char msg[] = "ERROR: e2e memory cap (bump arena exhausted)\n";
        (void) !write(2, msg, sizeof(msg) - 1);
        _exit(9);
    }
    void *p = g_arena + g_arena_off;
    g_arena_off += rounded;
    return p;
}

void *operator new(size_t n) { return oracleAlloc(n); }
void *operator new[](size_t n) { return oracleAlloc(n); }
void operator delete(void *) noexcept { }
void operator delete[](void *) noexcept { }
void operator delete(void *, size_t) noexcept { }
void operator delete[](void *, size_t) noexcept { }
#endif
