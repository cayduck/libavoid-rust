// Recording DebugHandler for the debug-handler oracle
// (SVG_MODE=debug-handler tools/oracle/svg_oracle.sh).  The patched Router
// constructor installs one when ORACLE_DEBUGHANDLER is set; it appends every
// call to debughandler.log in the working directory.  The Rust test
// tests/debughandler_oracle.rs installs a handler that writes the same text.
//
// Objects are printed by value, never by address: a vertex as
// "[objID,vn,props](x,y)", a connector by id with its raw and display
// routes.  Fields are read directly (-fno-access-control) so that the
// handler has no side effects (displayRoute() would cache a route).
#include <cstdio>
#include <set>
#include <vector>
#include "libavoid/libavoid.h"
#include "libavoid/vertices.h"
#include "libavoid/connector.h"
#include "libavoid/debughandler.h"

namespace Avoid {

class OracleDebugHandler : public DebugHandler
{
    public:
        OracleDebugHandler()
        {
            fp = fopen("debughandler.log", "a");
        }
        void vert(VertInf *v)
        {
            fprintf(fp, " [%u,%u,%u](%.17g,%.17g)", v->id.objID,
                    (unsigned) v->id.vn, (unsigned) v->id.props,
                    v->point.x, v->point.y);
        }
        void points(const char *name, const std::vector<Point>& ps)
        {
            fprintf(fp, " %s %u", name, (unsigned) ps.size());
            for (size_t i = 0; i < ps.size(); ++i)
            {
                fprintf(fp, " (%.17g,%.17g)", ps[i].x, ps[i].y);
            }
        }
        virtual void updateObstacleBoxes(std::vector<Avoid::Box> obstacles)
        {
            fprintf(fp, "updateObstacleBoxes %u", (unsigned) obstacles.size());
            for (size_t i = 0; i < obstacles.size(); ++i)
            {
                fprintf(fp, " (%.17g,%.17g,%.17g,%.17g)", obstacles[i].min.x,
                        obstacles[i].min.y, obstacles[i].max.x,
                        obstacles[i].max.y);
            }
            fprintf(fp, "\n");
        }
        virtual void updateConnectorRoute(ConnRef *conn, int index1, int index2)
        {
            fprintf(fp, "updateConnectorRoute %u %d %d", conn->id(), index1,
                    index2);
            points("route", conn->m_route.ps);
            points("display", conn->m_display_route.ps);
            fprintf(fp, "\n");
        }
        virtual void beginningSearchWithEndpoints(VertInf *src, VertInf *tar)
        {
            fprintf(fp, "beginningSearchWithEndpoints");
            vert(src);
            vert(tar);
            fprintf(fp, "\n");
        }
        virtual void updateCurrentSearchPath(Avoid::PolyLine currentPath)
        {
            fprintf(fp, "updateCurrentSearchPath");
            points("path", currentPath.ps);
            fprintf(fp, "\n");
        }
        virtual void beginningHyperedgeReroutingWithEndpoints(std::set<VertInf *> endpoints)
        {
            fprintf(fp, "beginningHyperedgeReroutingWithEndpoints %u",
                    (unsigned) endpoints.size());
            for (std::set<VertInf *>::iterator it = endpoints.begin();
                    it != endpoints.end(); ++it)
            {
                vert(*it);
            }
            fprintf(fp, "\n");
        }
        virtual void mtstGrowForestWithEdge(Avoid::VertInf *u, Avoid::VertInf *v, bool shouldWait)
        {
            fprintf(fp, "mtstGrowForestWithEdge");
            vert(u);
            vert(v);
            fprintf(fp, " %d\n", (int) shouldWait);
        }
        virtual void mtstPotentialBridgingEdge(Avoid::VertInf *u, Avoid::VertInf *v)
        {
            fprintf(fp, "mtstPotentialBridgingEdge");
            vert(u);
            vert(v);
            fprintf(fp, "\n");
        }
        virtual void mtstCommitToEdge(Avoid::VertInf *u, Avoid::VertInf *v, bool isBridge)
        {
            fprintf(fp, "mtstCommitToEdge");
            vert(u);
            vert(v);
            fprintf(fp, " %d\n", (int) isBridge);
        }
    private:
        FILE *fp;
};

DebugHandler *oracleDebugHandler(void)
{
    return new OracleDebugHandler();
}

}
