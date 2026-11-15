// Replays a comparison scene (see gen.mjs) through C++ libavoid and prints
// every connector's display route after each processTransaction().
#include <cstdio>
#include <fstream>
#include <sstream>
#include <map>
#include <string>
#include <chrono>
#include "libavoid/libavoid.h"
using namespace Avoid;
int main(int argc, char **argv) {
  std::ifstream in(argv[1]);
  std::string line; Router *router = nullptr;
  std::map<unsigned, ShapeRef *> shapes; std::map<unsigned, ConnRef *> conns;
  int t = 0; double total = 0;
  auto readEnd = [&](std::istringstream &ss) -> ConnEnd {
    std::string k; ss >> k;
    if (k == "P") { double x, y; ss >> x >> y; return ConnEnd(Point(x, y)); }
    unsigned s, c; ss >> s >> c; return ConnEnd(shapes[s], c);
  };
  while (std::getline(in, line)) {
    std::istringstream ss(line); std::string op; ss >> op;
    if (op == "router") { std::string k; ss >> k; router = new Router(k == "orth" ? OrthogonalRouting : PolyLineRouting); }
    else if (op == "param") { int p; double v; ss >> p >> v; router->setRoutingParameter((RoutingParameter) p, v); }
    else if (op == "option") { int o, v; ss >> o >> v; router->setRoutingOption((RoutingOption) o, v != 0); }
    else if (op == "shape") { unsigned id; double x, y, w, h; ss >> id >> x >> y >> w >> h;
      Rectangle r(Point(x, y), Point(x + w, y + h)); shapes[id] = new ShapeRef(router, r, id); }
    else if (op == "pin") { unsigned id, c; double px, py; int d; ss >> id >> c >> px >> py >> d;
      new ShapeConnectionPin(shapes[id], c, px, py, true, 0, (ConnDirFlags) d); }
    else if (op == "conn") { unsigned id; ss >> id; ConnEnd a = readEnd(ss); ConnEnd b = readEnd(ss);
      conns[id] = new ConnRef(router, a, b, id); }
    else if (op == "move") { unsigned id; double dx, dy; ss >> id >> dx >> dy; router->moveShape(shapes[id], dx, dy); }
    else if (op == "delconn") { unsigned id; ss >> id; router->deleteConnector(conns[id]); conns.erase(id); }
    else if (op == "process") {
      auto t0 = std::chrono::steady_clock::now();
      router->processTransaction();
      total += std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count();
      printf("T %d\n", t++);
      for (auto &kv : conns) { const PolyLine &r = kv.second->displayRoute();
        printf("c %u %zu", kv.first, r.ps.size());
        for (auto &p : r.ps) printf(" %.17g %.17g", p.x, p.y);
        printf("\n"); }
    }
  }
  fprintf(stderr, "time_ms %.3f\n", total);
  delete router;
  return 0;
}
