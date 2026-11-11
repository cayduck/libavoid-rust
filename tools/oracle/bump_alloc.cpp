// Deterministic allocator for the fixture oracle (tools/oracle/build.sh with
// ORACLE_ALLOC=bump).
//
// Upstream libavoid orders some containers and breaks some ties by pointer
// value (std::set<T*>, CmpVertInf, CmpNodePos, and `(int)(ea->v - eb->v)` in
// scanline.cpp compare_events, a difference of unrelated Node pointers that
// GCC compiles as an exact division by sizeof(Node)).  With glibc malloc the
// outcome depends on heap layout.  The Rust port models pointer order as
// creation order, so this allocator makes the C++ compute exactly that model:
// memory is never reused and every allocation is rounded up to a multiple of
// a stride that is a multiple of sizeof(Avoid::Node) and of 16.
#include <cstdlib>
#include <new>
#include <sys/mman.h>
#include "libavoid/libavoid.h"
#include "libavoid/scanline.h"

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
    n = ((n + kStride - 1) / kStride) * kStride;
    if (n == 0) n = kStride;
    void *p = g_arena + g_arena_off;
    g_arena_off += n;
    if (g_arena_off > g_arena_size) abort();
    return p;
}

void *operator new(size_t n) { return oracleAlloc(n); }
void *operator new[](size_t n) { return oracleAlloc(n); }
void operator delete(void *) noexcept { }
void operator delete[](void *) noexcept { }
void operator delete(void *, size_t) noexcept { }
void operator delete[](void *, size_t) noexcept { }
