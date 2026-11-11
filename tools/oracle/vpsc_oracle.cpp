// Differential oracle for src/avoid/vpsc.rs against upstream libavoid vpsc.cpp.
//
// Build & regenerate the golden file (from the repository root):
//
//   g++ -O2 -std=c++11 -I vendor/adaptagrams/cola \
//       tools/oracle/vpsc_oracle.cpp vendor/adaptagrams/cola/libavoid/vpsc.cpp \
//       -o /tmp/vpsc_oracle && /tmp/vpsc_oracle > tests/golden/vpsc_oracle.txt
//
// (vendor/ is gitignored; point -I and the vpsc.cpp path at your adaptagrams
// checkout.)  Asserts are left enabled (no -DNDEBUG), as in a default build.
//
// The problem generator below must stay in lock-step with the `tests` module
// of src/avoid/vpsc.rs.  Doubles are printed as their IEEE-754 bit pattern
// (compared by the Rust test) followed by " # %.17g" for readability.
//
// Private/protected members are opened up so that code paths which libavoid
// itself never reaches from IncSolver::solve() (Blocks::split, mergeLeft,
// mergeRight, the constraint heaps, the cycle checks, totalOrder) can be
// exercised too.

#include <cassert>
#include <cfloat>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <list>
#include <map>
#include <queue>
#include <set>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#define private public
#define protected public
#include "libavoid/vpsc.h"
#undef private
#undef protected

using namespace Avoid;

struct Lcg {
    uint64_t state;
    explicit Lcg(uint64_t seed) : state(seed) {}
    uint32_t next() {
        state = state * 6364136223846793005ULL + 1442695040888963407ULL;
        return (uint32_t) (state >> 33);
    }
    uint32_t below(uint32_t n) { return next() % n; }
};

static const double freeWeight = 0.00001;
static const double strongWeight = 0.001;
static const double strongerWeight = 1.0;
static const double fixedWeight = 100000;

static const char *hex(double x, char *buf) {
    uint64_t bits;
    memcpy(&bits, &x, sizeof bits);
    sprintf(buf, "%016llx", (unsigned long long) bits);
    return buf;
}

static Variable *newVar(int id, double pos, double weight, double scale) {
    Variable *v = new Variable(id, pos, weight, scale);
    v->finalPosition = 0.0;  // upstream leaves it uninitialised
    return v;
}

static void dumpState(const Variables &vs, const Constraints &cs) {
    char buf[32];
    for (size_t i = 0; i < vs.size(); ++i) {
        printf("v %zu %s # %.17g\n", i, hex(vs[i]->finalPosition, buf),
                vs[i]->finalPosition);
    }
    for (size_t j = 0; j < cs.size(); ++j) {
        printf("c %zu %d %d\n", j, (int) cs[j]->unsatisfiable,
                (int) cs[j]->active);
    }
}

// Returns true if solve completed without throwing.
static bool doSolve(IncSolver &s) {
    try {
        bool r = s.solve();
        printf("solve %d %u\n", (int) r, s.splitCnt);
        return true;
    } catch (const char *) {
        printf("solve throw\n");
        return false;
    }
}

static void extras(IncSolver &s, const Variables &vs, const Constraints &cs) {
    bool cg = s.constraintGraphIsCyclic(vs.size(), vs.data());
    bool bg = s.blockGraphIsCyclic();
    printf("cyc %d %d\n", (int) cg, (int) bg);
    std::list<Variable *> *order = s.bs->totalOrder();
    printf("order");
    for (std::list<Variable *>::iterator it = order->begin();
            it != order->end(); ++it) {
        size_t idx = 0;
        while (vs[idx] != *it) ++idx;
        printf(" %zu", idx);
    }
    printf("\n");
    delete order;

    size_t nb = s.bs->size();
    int nsplits = 0;
    for (size_t i = 0; i < nb; ++i) {
        Block *b = s.bs->at(i);
        if (b->deleted) continue;
        Constraint *found = nullptr;
        for (size_t j = 0; j < cs.size(); ++j) {
            Constraint *c = cs[j];
            if (c->active && !c->equality && c->left->block == b &&
                    c->right->block == b) {
                found = c;
                break;
            }
        }
        if (found) {
            Block *l = nullptr, *r = nullptr;
            s.bs->split(b, l, r, found);
            ++nsplits;
        }
    }
    s.bs->cleanup();
    s.copyResult();
    printf("split %d %zu %ld\n", nsplits, s.bs->size(), s.bs->blockTimeCtr);
    dumpState(vs, cs);
    try {
        bool r = s.satisfy();
        printf("solve %d %u\n", (int) r, s.splitCnt);
    } catch (const char *) {
        printf("solve throw\n");
    }
    dumpState(vs, cs);
    char buf[32];
    double cost = s.bs->cost();
    printf("cost %s # %.17g\n", hex(cost, buf), cost);
}

static double genNudging(Lcg &rng, Variables &vs, Constraints &cs) {
    uint32_t nseg = 2 + rng.below(12);
    double sepDist = (double) (4 + rng.below(8));
    std::vector<std::pair<size_t, bool> > prev;
    for (uint32_t s = 0; s < nseg; ++s) {
        bool fixed = rng.below(5) == 0;
        double desired = (double) rng.below(400) * 0.5;
        if (rng.below(3) == 0) {
            desired += 1.0 / 3.0;
        }
        int id;
        double weight;
        if (fixed) {
            id = 1;
            weight = fixedWeight;
        } else {
            uint32_t w = rng.below(3);
            id = 0;
            weight = (w == 0) ? freeWeight : (w == 1) ? strongWeight
                                                      : strongerWeight;
        }
        Variable *var = newVar(id, desired, weight, 1.0);
        var->fixedDesiredPosition = fixed;
        vs.push_back(var);
        size_t index = vs.size() - 1;
        if (!fixed && rng.below(2) == 0) {
            double lim = desired - (double) rng.below(60);
            vs.push_back(newVar(2, lim, fixedWeight, 1.0));
            cs.push_back(new Constraint(vs[vs.size() - 1], vs[index], 0.0));
        }
        for (size_t p = 0; p < prev.size(); ++p) {
            size_t pv = prev[p].first;
            bool pfixed = prev[p].second;
            if (rng.below(3) == 0 && (!fixed || !pfixed)) {
                uint32_t r = rng.below(6);
                double gap;
                bool eq;
                if (r == 0) {
                    gap = 0.0; eq = true;
                } else if (r == 1) {
                    gap = 0.0; eq = false;
                } else {
                    gap = sepDist; eq = false;
                }
                cs.push_back(new Constraint(vs[pv], vs[index], gap, eq));
            }
        }
        if (!fixed && rng.below(2) == 0) {
            double lim = desired + (double) rng.below(60);
            vs.push_back(newVar(3, lim, fixedWeight, 1.0));
            cs.push_back(new Constraint(vs[index], vs[vs.size() - 1], 0.0));
        }
        prev.push_back(std::make_pair(index, fixed));
    }
    return sepDist;
}

static void genRandom(Lcg &rng, Variables &vs, Constraints &cs, uint32_t kind) {
    uint32_t nv = 1 + rng.below(15);
    const double weights[] = {1.0, 0.001, 100000.0, 2.5, 0.00001};
    const double scales[] = {1.0, 2.0, 0.5, 4.0, 3.0, 0.7};
    for (uint32_t i = 0; i < nv; ++i) {
        int id = (int) rng.below(4);
        double desired = (double) rng.below(1000) / 4.0 - 50.0;
        double w = weights[rng.below(5)];
        double scale = (kind == 4) ? scales[rng.below(6)] : 1.0;
        Variable *v = newVar(id, desired, w, scale);
        v->fixedDesiredPosition = (w == 100000.0);
        vs.push_back(v);
    }
    uint32_t nc = rng.below(2 * nv + 1);
    for (uint32_t i = 0; i < nc; ++i) {
        size_t l = rng.below(nv);
        size_t r = rng.below(nv);
        if (kind != 2) {
            if (l == r) continue;
            if (l > r) std::swap(l, r);
        }
        double gap = (double) rng.below(20) * 0.5;
        if (rng.below(5) == 0) gap = -gap;
        bool eq = rng.below(6) == 0;
        cs.push_back(new Constraint(vs[l], vs[r], gap, eq));
    }
    if (kind == 2 && nv >= 2 && rng.below(2) == 0) {
        size_t a = rng.below(nv);
        size_t b = (a + 1) % nv;
        cs.push_back(new Constraint(vs[a], vs[b], 3.0, false));
        cs.push_back(new Constraint(vs[b], vs[a], 3.0, false));
    }
}

// Dense problems with integer data, to provoke exact slack / lm ties.
static void genDense(Lcg &rng, Variables &vs, Constraints &cs) {
    uint32_t nv = 5 + rng.below(26);
    const double weights[] = {1.0, 1.0, 2.0, 0.001};
    for (uint32_t i = 0; i < nv; ++i) {
        int id = (int) rng.below(4);
        double desired = (double) rng.below(40);
        double w = weights[rng.below(4)];
        vs.push_back(newVar(id, desired, w, 1.0));
    }
    uint32_t nc = rng.below(6 * nv + 1);
    for (uint32_t i = 0; i < nc; ++i) {
        size_t l = rng.below(nv);
        size_t r = rng.below(nv);
        if (l == r) continue;
        if (rng.below(10) != 0 && l > r) std::swap(l, r);
        double gap = (double) rng.below(4);
        bool eq = rng.below(15) == 0;
        cs.push_back(new Constraint(vs[l], vs[r], gap, eq));
    }
}

// Direct stress of the constraint heaps: mergeLeft then mergeRight on every
// block of a fresh solver.
static void heapTest(Variables &vs, Constraints &cs) {
    IncSolver s(vs, cs);
    size_t nb = s.bs->size();
    for (size_t i = 0; i < nb; ++i) {
        Block *b = s.bs->at(i);
        if (!b->deleted) s.bs->mergeLeft(b);
    }
    s.bs->cleanup();
    nb = s.bs->size();
    for (size_t i = 0; i < nb; ++i) {
        Block *b = s.bs->at(i);
        if (!b->deleted) s.bs->mergeRight(b);
    }
    s.bs->cleanup();
    s.copyResult();
    printf("heap %zu %ld\n", s.bs->size(), s.bs->blockTimeCtr);
    dumpState(vs, cs);
    char buf[32];
    double cost = s.bs->cost();
    printf("cost %s # %.17g\n", hex(cost, buf), cost);
}

static void runCase(uint32_t k) {
    Lcg rng((uint64_t) k * 7919 + 1);
    uint32_t kind = k % 6;
    printf("case %u %u\n", k, kind);
    Variables vs;
    Constraints cs;
    if (kind == 0 || kind == 3) {
        double sep = genNudging(rng, vs, cs);
        const int rounds = 3;
        for (int round = 0; round < rounds; ++round) {
            printf("round %d\n", round);
            {
                IncSolver s(vs, cs);
                bool ok = doSolve(s);
                if (round == rounds - 1 && ok) {
                    dumpState(vs, cs);
                    extras(s, vs, cs);
                    continue;
                }
            }
            dumpState(vs, cs);
            for (size_t j = 0; j < cs.size(); ++j) {
                if (cs[j]->gap > 0) {
                    cs[j]->gap -= sep / 5.0;
                }
            }
            if (kind == 3 && rng.below(2) == 0 && vs.size() >= 2) {
                size_t a = rng.below((uint32_t) vs.size());
                size_t b = rng.below((uint32_t) vs.size());
                if (a != b) {
                    cs.push_back(new Constraint(vs[a], vs[b], 0.0, true));
                }
            }
        }
    } else {
        if (kind == 5) {
            genDense(rng, vs, cs);
        } else {
            genRandom(rng, vs, cs, kind);
        }
        Constraints kept = constraintsRemovingRedundantEqualities(vs, cs);
        printf("kept");
        for (size_t i = 0; i < kept.size(); ++i) {
            size_t idx = 0;
            while (cs[idx] != kept[i]) ++idx;
            printf(" %zu", idx);
        }
        printf("\n");
        IncSolver s(vs, cs);
        bool ok = doSolve(s);
        dumpState(vs, cs);
        if (ok) {
            extras(s, vs, cs);
        }
    }
    heapTest(vs, cs);
    for (size_t i = 0; i < vs.size(); ++i) delete vs[i];
    for (size_t i = 0; i < cs.size(); ++i) delete cs[i];
}

int main() {
    const uint32_t NUM_CASES = 600;
    for (uint32_t k = 0; k < NUM_CASES; ++k) {
        runCase(k);
    }
    return 0;
}
