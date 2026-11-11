// Differential oracle for the Rust port of the second half of libavoid
// orthogonal.cpp (route improvement / nudging: ImproveOrthogonalRoutes,
// NudgingShiftSegment, CmpLineOrder, linesort, the VPSC relaxation loop).
//
// Scenes are written in a small line-based DSL (documented at runCommand()
// below) that both this program and the Rust test `tests/nudging_oracle.rs`
// interpret.  The golden file `tests/golden/nudging_oracle.txt` contains,
// for every scene, the DSL text followed by this program's output; the Rust
// test replays the DSL and must reproduce the output byte for byte.
//
// Every `process` command runs Router::processTransaction() and dumps
//  * every connector's raw route() and displayRoute() (%.17g), and
//  * a trace of the nudging internals produced by a patched *copy* of
//    orthogonal.cpp (see tools/oracle/nudging_oracle.sh, which inserts
//    nudgeOracleTrace() calls; the vendored sources are never modified):
//    for every region of nudgeOrthogonalRoutes() the sorted segment list
//    (connector, point indexes, position, min/max space limits, flags) and,
//    after the relaxation loop, the VPSC variables (id, desired, final,
//    weight), whether the problem was satisfied and the final separation
//    distance.  Large traces are replaced by a line count + FNV-1a hash
//    (ORACLE_FULL=1 prints them in full).
//
// Scenes are random orthogonal diagrams designed to force many connectors
// through shared channels: shapes on an aligned grid, many connectors
// between a few shape pairs, shared endpoints, connection pins (exclusive
// and not), checkpoints, junctions with >= 3 connectors (hyperedge
// improvement options are turned OFF in scenes with junctions, since the
// HyperedgeImprover is not ported yet), random nudging options/distances
// and penalties (incl. crossing and fixed-shared-path penalties), followed
// by incremental moves / deletions / parameter changes.
//
// Every scene runs in a forked child, so a C++ assertion failure or crash
// in one scene (or a scene running longer than 20 s) only marks that scene
// "#crash <assertion message>".  nudging_oracle.sh runs an asserting and an
// -DNDEBUG build and stores the NDEBUG output, marking scenes in which the
// asserting build fails with "#assert <message>" (see tests/nudging_oracle.rs).
// ORACLE_ONLY=i runs scene i in-process and prints a backtrace on a crash.
//
// Pointer order: as in the other oracles, operator new is a bump allocator
// that never reuses memory and pads every allocation to a multiple of
// sizeof(Avoid::Node) (tools/oracle/bump_alloc.cpp), so pointer order ==
// allocation order, the model of the Rust port.
//
// Build & regenerate: tools/oracle/nudging_oracle.sh

#include <cstdio>
#include <cstdlib>
#include <cstdint>
#include <cstring>
#include <string>
#include <vector>
#include <map>
#include <sstream>
#include <unistd.h>
#include <sys/wait.h>
#include <poll.h>
#include <execinfo.h>
#include <sys/resource.h>
#include <signal.h>
#include <ctime>

#include "libavoid/libavoid.h"
#include "libavoid/router.h"
#include "libavoid/shape.h"
#include "libavoid/junction.h"
#include "libavoid/connector.h"
#include "libavoid/connend.h"
#include "libavoid/connectionpin.h"

using namespace Avoid;

//============================================================================
// Output helpers.
//============================================================================

static std::string g17(double v)
{
    if (v != v)
    {
        return "nan";
    }
    char buf[64];
    snprintf(buf, sizeof(buf), "%.17g", v);
    return buf;
}

static std::vector<std::string> g_out;
static std::vector<std::string> g_trace;
static bool g_full = false;

static void out(const std::string& s)
{
    g_out.push_back(s);
}

// Called from the patched orthogonal.cpp.
namespace Avoid {
void nudgeOracleTrace(const std::string& s)
{
    g_trace.push_back(s);
}
std::string nudgeOracleG17(double v)
{
    return g17(v);
}
}

static uint64_t fnv(const std::vector<std::string>& lines)
{
    uint64_t h = 1469598103934665603ULL;
    for (size_t i = 0; i < lines.size(); ++i)
    {
        const std::string& s = lines[i];
        for (size_t k = 0; k < s.size(); ++k)
        {
            h ^= (unsigned char) s[k];
            h *= 1099511628211ULL;
        }
        h ^= (unsigned char) '\n';
        h *= 1099511628211ULL;
    }
    return h;
}

static const size_t kFullTraceLimit = 200;

static std::string polyStr(const PolyLine& p)
{
    std::string s = std::to_string(p.size()) + ":";
    for (size_t i = 0; i < p.size(); ++i)
    {
        s += " " + g17(p.ps[i].x) + "," + g17(p.ps[i].y);
    }
    return s;
}

//============================================================================
// Scene DSL interpreter.
//============================================================================

struct Scene
{
    Router *router;
    std::map<std::string, ShapeRef *> shapes;
    std::map<std::string, JunctionRef *> juncs;
    std::map<std::string, ConnRef *> conns;
    std::map<std::string, ShapeConnectionPin *> pins;
    int stage;
};

static double num(std::istringstream& in)
{
    std::string t;
    in >> t;
    if (t.empty())
    {
        fprintf(stderr, "missing number\n");
        abort();
    }
    return strtod(t.c_str(), nullptr);
}

static unsigned unum(std::istringstream& in)
{
    std::string t;
    in >> t;
    return (unsigned) strtoul(t.c_str(), nullptr, 10);
}

static std::string word(std::istringstream& in)
{
    std::string t;
    in >> t;
    return t;
}

// end := P x y dirs | S shape classId | J junction
static ConnEnd readEnd(Scene& sc, std::istringstream& in)
{
    std::string k = word(in);
    if (k == "P")
    {
        double x = num(in);
        double y = num(in);
        unsigned dirs = unum(in);
        return ConnEnd(Point(x, y), (ConnDirFlags) dirs);
    }
    else if (k == "S")
    {
        std::string s = word(in);
        unsigned cls = unum(in);
        return ConnEnd(sc.shapes.at(s), cls);
    }
    else if (k == "J")
    {
        std::string j = word(in);
        return ConnEnd(sc.juncs.at(j));
    }
    fprintf(stderr, "bad end %s\n", k.c_str());
    abort();
}

static void dumpRoutes(Scene& sc)
{
    Router *r = sc.router;
    char buf[256];
    snprintf(buf, sizeof(buf), "stage %d", sc.stage++);
    out(buf);
    if (g_full || g_trace.size() <= kFullTraceLimit)
    {
        for (size_t i = 0; i < g_trace.size(); ++i)
        {
            out(g_trace[i]);
        }
    }
    else
    {
        snprintf(buf, sizeof(buf), "trace lines=%u fnv=%016llx",
                (unsigned) g_trace.size(), (unsigned long long) fnv(g_trace));
        out(buf);
    }
    g_trace.clear();
    for (ConnRefList::const_iterator it = r->connRefs.begin();
            it != r->connRefs.end(); ++it)
    {
        ConnRef *c = *it;
        out("C " + std::to_string(c->id()) + " t=" + std::to_string((int) c->routingType()) +
                " raw " + polyStr(c->route()));
        out("  display " + polyStr(c->displayRoute()));
    }
}

// Commands (one per line; '#' starts a comment line):
//   router FLAGS
//   param K V                    setRoutingParameter((RoutingParameter) K, V)
//   option K 0|1                 setRoutingOption((RoutingOption) K, b)
//   rect NAME ID x1 y1 x2 y2     new ShapeRef(router, Rectangle(P1, P2), ID)
//   junction NAME ID x y         new JunctionRef(router, Point, ID)
//   fixj NAME 0|1                junction->setPositionFixed(b)
//   pin NAME SHAPE CLASS XO YO PROP INSIDE DIRS
//                                new ShapeConnectionPin(shape, ...)
//   pinexcl NAME 0|1             pin->setExclusive(b)
//   connse NAME ID END END       new ConnRef(router, src, dst, ID)
//   src CONN END / dst CONN END  setSourceEndpoint / setDestEndpoint
//   type CONN T                  setRoutingType((ConnType) T)
//   checkpoints CONN N (x y ad dd)*   setRoutingCheckpoints
//   move SHAPE dx dy             router->moveShape(shape, dx, dy)
//   movej JUNC dx dy             router->moveJunction(junction, dx, dy)
//   delshape SHAPE / delconn CONN
//   process                      processTransaction + dump
static void runCommand(Scene& sc, const std::string& line)
{
    std::istringstream in(line);
    std::string cmd = word(in);
    Router *r = sc.router;
    if (cmd.empty() || cmd[0] == '#')
    {
        return;
    }
    if (cmd == "router")
    {
        unsigned flags = unum(in);
        sc.router = new Router(flags);
        return;
    }
    if (cmd == "param")
    {
        unsigned k = unum(in);
        double v = num(in);
        r->setRoutingParameter((RoutingParameter) k, v);
    }
    else if (cmd == "option")
    {
        unsigned k = unum(in);
        unsigned v = unum(in);
        r->setRoutingOption((RoutingOption) k, v != 0);
    }
    else if (cmd == "rect")
    {
        std::string name = word(in);
        unsigned id = unum(in);
        double x1 = num(in);
        double y1 = num(in);
        double x2 = num(in);
        double y2 = num(in);
        Rectangle rect(Point(x1, y1), Point(x2, y2));
        sc.shapes[name] = new ShapeRef(r, rect, id);
    }
    else if (cmd == "junction")
    {
        std::string name = word(in);
        unsigned id = unum(in);
        double x = num(in);
        double y = num(in);
        sc.juncs[name] = new JunctionRef(r, Point(x, y), id);
    }
    else if (cmd == "fixj")
    {
        std::string name = word(in);
        unsigned v = unum(in);
        sc.juncs.at(name)->setPositionFixed(v != 0);
    }
    else if (cmd == "pin")
    {
        std::string name = word(in);
        std::string shape = word(in);
        unsigned cls = unum(in);
        double xo = num(in);
        double yo = num(in);
        unsigned prop = unum(in);
        double inside = num(in);
        unsigned dirs = unum(in);
        sc.pins[name] = new ShapeConnectionPin(sc.shapes.at(shape), cls, xo, yo,
                prop != 0, inside, (ConnDirFlags) dirs);
    }
    else if (cmd == "pinexcl")
    {
        std::string name = word(in);
        unsigned v = unum(in);
        sc.pins.at(name)->setExclusive(v != 0);
    }
    else if (cmd == "connse")
    {
        std::string name = word(in);
        unsigned id = unum(in);
        ConnEnd src = readEnd(sc, in);
        ConnEnd dst = readEnd(sc, in);
        sc.conns[name] = new ConnRef(r, src, dst, id);
    }
    else if (cmd == "src" || cmd == "dst")
    {
        std::string name = word(in);
        ConnEnd end = readEnd(sc, in);
        if (cmd == "src")
        {
            sc.conns.at(name)->setSourceEndpoint(end);
        }
        else
        {
            sc.conns.at(name)->setDestEndpoint(end);
        }
    }
    else if (cmd == "type")
    {
        std::string name = word(in);
        unsigned t = unum(in);
        sc.conns.at(name)->setRoutingType((ConnType) t);
    }
    else if (cmd == "checkpoints")
    {
        std::string name = word(in);
        unsigned n = unum(in);
        std::vector<Checkpoint> cps;
        for (unsigned i = 0; i < n; ++i)
        {
            double x = num(in);
            double y = num(in);
            unsigned ad = unum(in);
            unsigned dd = unum(in);
            cps.push_back(Checkpoint(Point(x, y), (ConnDirFlags) ad,
                    (ConnDirFlags) dd));
        }
        sc.conns.at(name)->setRoutingCheckpoints(cps);
    }
    else if (cmd == "move")
    {
        std::string name = word(in);
        double dx = num(in);
        double dy = num(in);
        r->moveShape(sc.shapes.at(name), dx, dy);
    }
    else if (cmd == "movej")
    {
        std::string name = word(in);
        double dx = num(in);
        double dy = num(in);
        r->moveJunction(sc.juncs.at(name), dx, dy);
    }
    else if (cmd == "delshape")
    {
        std::string name = word(in);
        r->deleteShape(sc.shapes.at(name));
        sc.shapes.erase(name);
    }
    else if (cmd == "delconn")
    {
        std::string name = word(in);
        r->deleteConnector(sc.conns.at(name));
        sc.conns.erase(name);
    }
    else if (cmd == "process")
    {
        g_trace.clear();
        r->processTransaction();
        dumpRoutes(sc);
    }
    else
    {
        fprintf(stderr, "unknown command: %s\n", line.c_str());
        abort();
    }
}

// Exit immediately on abort()/crash: a core dump of a process with the
// 1 TiB bump arena mapping takes minutes (a piped core_pattern, as on WSL,
// ignores RLIMIT_CORE) and the process cannot be killed meanwhile.
static void crashHandler(int sig)
{
    if (getenv("ORACLE_ONLY"))
    {
        void *frames[64];
        int n = backtrace(frames, 64);
        backtrace_symbols_fd(frames, n, 2);
    }
    _exit(128 + sig);
}

static void installCrashHandlers(void)
{
    struct rlimit noCore;
    noCore.rlim_cur = 0;
    noCore.rlim_max = 0;
    setrlimit(RLIMIT_CORE, &noCore);
    signal(SIGABRT, crashHandler);
    signal(SIGSEGV, crashHandler);
    signal(SIGBUS, crashHandler);
    signal(SIGFPE, crashHandler);
}

// Runs the scene in a forked child; returns its output, or "#crash ..."
// (with the last assertion/exception line the child wrote to stderr).
static std::string runSceneChild(const std::vector<std::string>& dsl)
{
    int fds[2];
    if (pipe(fds) != 0)
    {
        abort();
    }
    FILE *errFile = tmpfile();
    fflush(stdout);
    fflush(stderr);
    pid_t pid = fork();
    if (pid == 0)
    {
        close(fds[0]);
        dup2(fileno(errFile), 2);
        // Scenes running longer than 20 s are recorded as "#crash" and
        // skipped.
        alarm(20);
        installCrashHandlers();
        Scene sc;
        sc.router = nullptr;
        sc.stage = 0;
        for (size_t i = 0; i < dsl.size(); ++i)
        {
            runCommand(sc, dsl[i]);
        }
        delete sc.router;
        std::string all;
        for (size_t i = 0; i < g_out.size(); ++i)
        {
            all += g_out[i] + "\n";
        }
        size_t off = 0;
        while (off < all.size())
        {
            ssize_t w = write(fds[1], all.data() + off, all.size() - off);
            if (w <= 0) _exit(3);
            off += (size_t) w;
        }
        close(fds[1]);
        _exit(0);
    }
    close(fds[1]);
    std::string result;
    char buf[65536];
    ssize_t n;
    // Watchdog in case the child's SIGALRM is not acted on.
    time_t start = time(nullptr);
    for (;;)
    {
        struct pollfd pfd;
        pfd.fd = fds[0];
        pfd.events = POLLIN;
        int pr = poll(&pfd, 1, 1000);
        if (pr > 0)
        {
            n = read(fds[0], buf, sizeof(buf));
            if (n <= 0)
            {
                break;
            }
            result.append(buf, (size_t) n);
        }
        if (time(nullptr) - start > 22)
        {
            kill(pid, SIGKILL);
        }
    }
    close(fds[0]);
    int status = 0;
    waitpid(pid, &status, 0);
    std::string lastErr;
    rewind(errFile);
    char line[4096];
    while (fgets(line, sizeof(line), errFile))
    {
        if (strstr(line, "Assertion") || strstr(line, "terminate") || strstr(line, "what()"))
        {
            lastErr = line;
        }
    }
    fclose(errFile);
    if (!lastErr.empty() && lastErr[lastErr.size() - 1] == '\n')
    {
        lastErr.erase(lastErr.size() - 1);
    }
    size_t slash = lastErr.find("libavoid/");
    if (slash != std::string::npos)
    {
        lastErr = lastErr.substr(slash);
    }
    if (!WIFEXITED(status) || WEXITSTATUS(status) != 0)
    {
        char b[128];
        if (WIFEXITED(status))
        {
            snprintf(b, sizeof(b), "#crash exit=%d", WEXITSTATUS(status));
        }
        else
        {
            snprintf(b, sizeof(b), "#crash signal=%d", WTERMSIG(status));
        }
        return std::string(b) + (lastErr.empty() ? "" : " " + lastErr) + "\n";
    }
    return result;
}

static void runScene(const std::string& name, const std::vector<std::string>& dsl)
{
    printf("#scene %s\n", name.c_str());
    for (size_t i = 0; i < dsl.size(); ++i)
    {
        printf("%s\n", dsl[i].c_str());
    }
    printf("#out\n");
    time_t t0 = time(nullptr);
    printf("%s", runSceneChild(dsl).c_str());
    if (getenv("ORACLE_TIMING"))
    {
        fprintf(stderr, "scene %s: %ld s\n", name.c_str(), (long) (time(nullptr) - t0));
    }
    printf("#end\n");
    fflush(stdout);
}

//============================================================================
// Random scenes.
//============================================================================

static uint64_t g_state = 0x2545f4914f6cdd1dULL;

static uint64_t next_u64()
{
    g_state = g_state * 6364136223846793005ULL + 1442695040888963407ULL;
    return g_state;
}

// Uniform integer in [0, n).
static unsigned rnd(unsigned n)
{
    return (unsigned) ((next_u64() >> 33) % n);
}

static std::string f17(double v)
{
    return g17(v);
}

struct GenBox
{
    std::string name;
    double x1, y1, x2, y2;
    std::vector<unsigned> pinClasses;
};

struct Gen
{
    std::vector<std::string> lines;
    std::vector<GenBox> boxes;
    std::vector<std::string> juncNames;
    std::vector<double> juncX, juncY;
    std::vector<std::string> connNames;
    unsigned nextId;
    double gapX, gapY;
};

// A point on the boundary of box b (side 0..3), at one of a few aligned
// offsets, with a matching visibility direction (or all/none).
static std::string boundaryEnd(Gen& g, const GenBox& b)
{
    unsigned side = rnd(4);
    unsigned k = rnd(5);
    double fx = b.x1 + ((b.x2 - b.x1) * (1 + k)) / 6;
    double fy = b.y1 + ((b.y2 - b.y1) * (1 + k)) / 6;
    double x, y;
    unsigned dir;
    if (side == 0)      { x = b.x1; y = fy; dir = ConnDirLeft; }
    else if (side == 1) { x = b.x2; y = fy; dir = ConnDirRight; }
    else if (side == 2) { x = fx; y = b.y1; dir = ConnDirUp; }
    else                { x = fx; y = b.y2; dir = ConnDirDown; }
    unsigned dmode = rnd(6);
    if (dmode == 0)
    {
        dir = ConnDirAll;
    }
    else if (dmode == 1)
    {
        dir = ConnDirNone;
    }
    // Sometimes slightly inside the shape, like real diagram editors do.
    if (rnd(3) == 0)
    {
        if (side == 0) x += 5;
        else if (side == 1) x -= 5;
        else if (side == 2) y += 5;
        else y -= 5;
    }
    (void) g;
    return "P " + f17(x) + " " + f17(y) + " " + std::to_string(dir);
}

static std::string randomEnd(Gen& g, unsigned boxIndex)
{
    const GenBox& b = g.boxes[boxIndex];
    unsigned kind = rnd(10);
    if (kind < 4 && !b.pinClasses.empty())
    {
        unsigned cls = b.pinClasses[rnd((unsigned) b.pinClasses.size())];
        return "S " + b.name + " " + std::to_string(cls);
    }
    if (kind < 8)
    {
        return boundaryEnd(g, b);
    }
    if (kind == 8 && !g.juncNames.empty())
    {
        return "J " + g.juncNames[rnd((unsigned) g.juncNames.size())];
    }
    // A free point in a channel next to the box.
    double x = b.x2 + g.gapX / 2;
    double y = b.y1 + (b.y2 - b.y1) / 2;
    if (rnd(2) == 0)
    {
        x = b.x1 + (b.x2 - b.x1) / 2;
        y = b.y2 + g.gapY / 2;
    }
    return "P " + f17(x) + " " + f17(y) + " " + std::to_string(rnd(2) ? 15 : 0);
}

static std::vector<std::string> genScene(unsigned index)
{
    Gen g;
    g.nextId = 1;
    std::vector<std::string>& L = g.lines;
    bool polyToo = (rnd(6) == 0);
    L.push_back(std::string("router ") + (polyToo ? "3" : "2"));

    bool withJunctions = (rnd(3) == 0);

    // Penalties / parameters.
    if (rnd(10) != 0)
    {
        L.push_back("param 0 " + f17(rnd(4) ? 50 : 10 + rnd(100)));
    }
    if (rnd(3) == 0) L.push_back("param 1 " + f17(rnd(2) ? 0 : 50));
    if (rnd(3) == 0) L.push_back("param 2 " + f17(rnd(2) ? 200 : 50));
    if (rnd(3) == 0) L.push_back("param 4 " + f17(rnd(2) ? 110 : 9000));
    if (rnd(3) == 0) L.push_back("param 5 " + f17(100));
    if (rnd(3) == 0) L.push_back("param 6 " + f17((double) rnd(3) * 4));
    if (rnd(4) != 0)
    {
        static const double dists[] = { 4, 10, 12.5, 25, 30, 3.5 };
        L.push_back("param 7 " + f17(dists[rnd(6)]));
    }
    if (rnd(5) == 0) L.push_back("param 8 " + f17(500));

    // Options.
    for (unsigned k = 0; k < 7; ++k)
    {
        bool hyper = (k == 1) || (k == 5);
        if (hyper && withJunctions)
        {
            // HyperedgeImprover is not ported yet: keep it off with junctions.
            L.push_back("option " + std::to_string(k) + " 0");
            continue;
        }
        if (rnd(2) == 0)
        {
            L.push_back("option " + std::to_string(k) + " " + std::to_string(rnd(2)));
        }
    }

    // Shapes on an aligned grid with shared channels.
    unsigned rows = 1 + rnd(3);
    unsigned cols = 2 + rnd(3);
    double cellW = 60 + 20 * rnd(4);
    double cellH = 40 + 20 * rnd(4);
    g.gapX = 30 + 10 * rnd(5);
    g.gapY = 30 + 10 * rnd(5);
    for (unsigned rI = 0; rI < rows; ++rI)
    {
        for (unsigned cI = 0; cI < cols; ++cI)
        {
            if (rnd(7) == 0)
            {
                continue;
            }
            GenBox b;
            b.name = "s" + std::to_string(rI) + "_" + std::to_string(cI);
            b.x1 = cI * (cellW + g.gapX);
            b.y1 = rI * (cellH + g.gapY);
            b.x2 = b.x1 + cellW;
            b.y2 = b.y1 + cellH;
            // Occasionally a narrower/shorter shape, so channels are ragged.
            if (rnd(4) == 0) b.x2 -= 10 * (1 + rnd(2));
            if (rnd(4) == 0) b.y1 += 10 * (1 + rnd(2));
            L.push_back("rect " + b.name + " " + std::to_string(g.nextId++) + " " +
                    f17(b.x1) + " " + f17(b.y1) + " " + f17(b.x2) + " " + f17(b.y2));
            unsigned nPins = rnd(4);
            for (unsigned p = 0; p < nPins; ++p)
            {
                unsigned cls = 1 + rnd(3);
                std::string pname = b.name + "p" + std::to_string(p);
                unsigned where = rnd(5);
                double xo = 0.5, yo = 0.5;
                unsigned dirs = ConnDirNone;
                if (where == 1) { xo = 0; yo = 0.25 * (1 + rnd(3)); dirs = ConnDirLeft; }
                else if (where == 2) { xo = 1; yo = 0.25 * (1 + rnd(3)); dirs = ConnDirRight; }
                else if (where == 3) { yo = 0; xo = 0.25 * (1 + rnd(3)); dirs = ConnDirUp; }
                else if (where == 4) { yo = 1; xo = 0.25 * (1 + rnd(3)); dirs = ConnDirDown; }
                double inside = (where == 0) ? 0 : (double) (rnd(3) * 5);
                L.push_back("pin " + pname + " " + b.name + " " + std::to_string(cls) + " " +
                        f17(xo) + " " + f17(yo) + " 1 " + f17(inside) + " " +
                        std::to_string(dirs));
                if (rnd(3) == 0)
                {
                    L.push_back("pinexcl " + pname + " 0");
                }
                b.pinClasses.push_back(cls);
            }
            g.boxes.push_back(b);
        }
    }
    if (g.boxes.size() < 2)
    {
        GenBox b;
        b.name = "extra";
        b.x1 = -200; b.y1 = -150; b.x2 = -120; b.y2 = -90;
        L.push_back("rect extra " + std::to_string(g.nextId++) + " -200 -150 -120 -90");
        g.boxes.push_back(b);
    }

    if (withJunctions)
    {
        unsigned nJ = 1 + rnd(2);
        for (unsigned j = 0; j < nJ; ++j)
        {
            // In a channel between grid cells.
            unsigned cI = rnd(cols);
            unsigned rI = rnd(rows + 1);
            double x = cI * (cellW + g.gapX) + cellW + g.gapX / 2;
            double y = rI * (cellH + g.gapY) - g.gapY / 2;
            std::string name = "j" + std::to_string(j);
            L.push_back("junction " + name + " " + std::to_string(g.nextId++) + " " +
                    f17(x) + " " + f17(y));
            if (rnd(2) == 0)
            {
                L.push_back("fixj " + name + " 1");
            }
            g.juncNames.push_back(name);
            g.juncX.push_back(x);
            g.juncY.push_back(y);
        }
    }

    // Connectors: many between a few "busy" pairs.
    unsigned nb = (unsigned) g.boxes.size();
    std::vector<std::pair<unsigned, unsigned> > pairs;
    unsigned nPairs = 1 + rnd(3);
    for (unsigned p = 0; p < nPairs; ++p)
    {
        unsigned a = rnd(nb);
        unsigned b = rnd(nb);
        if (a == b) b = (a + 1) % nb;
        pairs.push_back(std::make_pair(a, b));
    }
    unsigned nConns = 4 + rnd(16);
    for (unsigned c = 0; c < nConns; ++c)
    {
        unsigned a, b;
        if (rnd(3) != 0)
        {
            const std::pair<unsigned, unsigned>& pr = pairs[rnd((unsigned) pairs.size())];
            a = pr.first;
            b = pr.second;
        }
        else
        {
            a = rnd(nb);
            b = rnd(nb);
            if (a == b) b = (a + 1) % nb;
        }
        std::string name = "c" + std::to_string(c);
        std::string src = randomEnd(g, a);
        std::string dst = randomEnd(g, b);
        // Shared endpoints: reuse the previous connector's source sometimes.
        if (c > 0 && rnd(5) == 0)
        {
            std::string prev = g.lines.back();
            // (prev may not be a connse line; only reuse when it is.)
            if (prev.compare(0, 7, "connse ") == 0)
            {
                std::istringstream in(prev);
                std::string w, n, id, k;
                in >> w >> n >> id >> k;
                if (k == "P")
                {
                    std::string x, y, d;
                    in >> x >> y >> d;
                    src = "P " + x + " " + y + " " + d;
                }
            }
        }
        L.push_back("connse " + name + " " + std::to_string(g.nextId++) + " " + src + " " + dst);
        g.connNames.push_back(name);
        if (polyToo && rnd(4) == 0)
        {
            L.push_back("type " + name + " 1");
        }
        if (rnd(7) == 0)
        {
            // Checkpoints in the channels around the source box.
            const GenBox& bx = g.boxes[a];
            unsigned n = 1 + rnd(2);
            std::string s = "checkpoints " + name + " " + std::to_string(n);
            for (unsigned k = 0; k < n; ++k)
            {
                double x = (rnd(2) ? bx.x2 + g.gapX / 2 : bx.x1 + (bx.x2 - bx.x1) / 2);
                double y = (rnd(2) ? bx.y2 + g.gapY / 2 : bx.y1 - g.gapY / 2);
                unsigned ad = rnd(3) ? 0 : (1u << rnd(4));
                unsigned dd = rnd(3) ? 0 : (1u << rnd(4));
                s += " " + f17(x) + " " + f17(y) + " " + std::to_string(ad) + " " +
                        std::to_string(dd);
            }
            L.push_back(s);
        }
    }
    // Make sure every junction has at least three connectors.
    for (size_t j = 0; j < g.juncNames.size(); ++j)
    {
        for (unsigned k = 0; k < 3; ++k)
        {
            std::string name = "jc" + std::to_string(j) + "_" + std::to_string(k);
            std::string other = randomEnd(g, rnd(nb));
            if (other[0] == 'J')
            {
                other = boundaryEnd(g, g.boxes[rnd(nb)]);
            }
            if (k % 2)
            {
                L.push_back("connse " + name + " " + std::to_string(g.nextId++) + " J " +
                        g.juncNames[j] + " " + other);
            }
            else
            {
                L.push_back("connse " + name + " " + std::to_string(g.nextId++) + " " +
                        other + " J " + g.juncNames[j]);
            }
            g.connNames.push_back(name);
        }
    }
    L.push_back("process");

    // Incremental changes.
    unsigned steps = rnd(3);
    for (unsigned s = 0; s < steps; ++s)
    {
        unsigned what = rnd(5);
        if (what == 0 || what == 1)
        {
            const GenBox& b = g.boxes[rnd(nb)];
            double dx = (double) ((int) rnd(5) - 2) * 10;
            double dy = (double) ((int) rnd(5) - 2) * 5;
            L.push_back("move " + b.name + " " + f17(dx) + " " + f17(dy));
        }
        else if (what == 2 && g.connNames.size() > 1)
        {
            unsigned k = rnd((unsigned) g.connNames.size());
            L.push_back("delconn " + g.connNames[k]);
            g.connNames.erase(g.connNames.begin() + k);
        }
        else if (what == 3)
        {
            static const double dists[] = { 4, 8, 16, 25 };
            L.push_back("param 7 " + f17(dists[rnd(4)]));
        }
        else if (!g.juncNames.empty())
        {
            L.push_back("movej " + g.juncNames[0] + " " + f17(10) + " " + f17(0));
        }
        else
        {
            L.push_back("option 0 " + std::to_string(rnd(2)));
        }
        L.push_back("process");
    }
    (void) index;
    return L;
}

int main(int argc, char **argv)
{
    g_full = getenv("ORACLE_FULL") != nullptr;
    unsigned n = 400;
    if (argc > 1)
    {
        n = (unsigned) strtoul(argv[1], nullptr, 10);
    }
    // ORACLE_ONLY=i runs only scene i, in-process (no fork; for debugging).
    const char *only = getenv("ORACLE_ONLY");
    printf("# nudging oracle: %u random scenes\n", n);
    for (unsigned i = 0; i < n; ++i)
    {
        std::vector<std::string> dsl = genScene(i);
        if (only)
        {
            if (i == (unsigned) atoi(only))
            {
                installCrashHandlers();
                Scene sc;
                sc.router = nullptr;
                sc.stage = 0;
                for (size_t k = 0; k < dsl.size(); ++k)
                {
                    runCommand(sc, dsl[k]);
                }
                for (size_t k = 0; k < g_out.size(); ++k)
                {
                    printf("%s\n", g_out[k].c_str());
                }
            }
            continue;
        }
        runScene("random-" + std::to_string(i), dsl);
    }
    return 0;
}
