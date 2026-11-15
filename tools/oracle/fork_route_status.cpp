// Oracle for ConnRef::hasValidRoute() and ConnRef::hasCrossingObstacles(),
// the two methods the port takes from the libavoid-js fork of Adaptagrams
// (built against the vendored upstream with tools/oracle/fork_route_status.patch;
// see tools/oracle/fork_route_status.sh).
//
// Replays a scene in the format of tools/libavoid-js-compare/gen.mjs, plus
//   checkpoints <conn id> <n> <x> <y> ...
// and prints after every processTransaction():
//   T <transaction number>
//   c <conn id> <hasValidRoute> <hasCrossingObstacles> <route point count>
// for every live connector in id order.
#include <cstdio>
#include <fstream>
#include <map>
#include <sstream>
#include <string>
#include <vector>
#include "libavoid/libavoid.h"
using namespace Avoid;

int main(int argc, char **argv)
{
    if (argc != 2) { fprintf(stderr, "usage: %s <scene>\n", argv[0]); return 2; }
    std::ifstream in(argv[1]);
    std::string line;
    Router *router = nullptr;
    std::map<unsigned, ShapeRef *> shapes;
    std::map<unsigned, ConnRef *> conns;
    int t = 0;
    auto readEnd = [&](std::istringstream& ss) -> ConnEnd {
        std::string k; ss >> k;
        if (k == "P") { double x, y; ss >> x >> y; return ConnEnd(Point(x, y)); }
        unsigned s, c; ss >> s >> c; return ConnEnd(shapes[s], c);
    };
    while (std::getline(in, line))
    {
        std::istringstream ss(line);
        std::string op; ss >> op;
        if (op == "router") {
            std::string k; ss >> k;
            router = new Router(k == "orth" ? OrthogonalRouting : PolyLineRouting);
        } else if (op == "param") {
            int p; double v; ss >> p >> v;
            router->setRoutingParameter((RoutingParameter) p, v);
        } else if (op == "option") {
            int o, v; ss >> o >> v;
            router->setRoutingOption((RoutingOption) o, v != 0);
        } else if (op == "shape") {
            unsigned id; double x, y, w, h; ss >> id >> x >> y >> w >> h;
            Rectangle r(Point(x, y), Point(x + w, y + h));
            shapes[id] = new ShapeRef(router, r, id);
        } else if (op == "pin") {
            unsigned id, c; double px, py; int d; ss >> id >> c >> px >> py >> d;
            new ShapeConnectionPin(shapes[id], c, px, py, true, 0, (ConnDirFlags) d);
        } else if (op == "conn") {
            unsigned id; ss >> id;
            ConnEnd a = readEnd(ss); ConnEnd b = readEnd(ss);
            conns[id] = new ConnRef(router, a, b, id);
        } else if (op == "checkpoints") {
            unsigned id; size_t n; ss >> id >> n;
            std::vector<Checkpoint> cps;
            for (size_t i = 0; i < n; ++i) { double x, y; ss >> x >> y; cps.push_back(Checkpoint(Point(x, y))); }
            conns[id]->setRoutingCheckpoints(cps);
        } else if (op == "move") {
            unsigned id; double dx, dy; ss >> id >> dx >> dy;
            router->moveShape(shapes[id], dx, dy);
        } else if (op == "delconn") {
            unsigned id; ss >> id;
            router->deleteConnector(conns[id]);
            conns.erase(id);
        } else if (op == "process") {
            router->processTransaction();
            printf("T %d\n", t++);
            for (auto& kv : conns) {
                printf("c %u %d %d %zu\n", kv.first, (int) kv.second->hasValidRoute(),
                        (int) kv.second->hasCrossingObstacles(), kv.second->route().size());
            }
        } else if (!op.empty()) {
            fprintf(stderr, "unknown op %s\n", op.c_str());
            return 2;
        }
    }
    delete router;
    return 0;
}
