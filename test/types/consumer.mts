// Type-level smoke test for the ES module entry (index.d.mts).
import init, { AvoidRouter, RouterFlag, reset } from "../../index.mjs";

await init();
const router = AvoidRouter.withFlags(RouterFlag.PolyLineRouting);
router.addConnector("c", 0, 0, 50, 50);
const ok: boolean = router.processTransaction();
const poisoned: boolean = router.isPoisoned();
await reset();
void ok;
void poisoned;
