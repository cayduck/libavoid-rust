// Type-level smoke test for the ES module entry (index.d.mts).
import init, * as Avoid from "../../index.mjs";
import { Router, initSync, reset } from "../../index.mjs";

await init();
const router = new Router(Avoid.PolyLineRouting);
const conn = new Avoid.ConnRef(router, new Avoid.ConnEnd(new Avoid.Point(0, 0)), new Avoid.ConnEnd(new Avoid.Point(50, 50)));
const ok: boolean = router.processTransaction();
const route: Avoid.PolyLine = conn.displayRoute();
const poisoned: boolean = Avoid.internalError() !== null;
await reset();
await init(new Uint8Array(0));
initSync({ module: new Uint8Array(0) });
void ok;
void route;
void poisoned;
