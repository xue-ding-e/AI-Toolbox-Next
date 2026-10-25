// One bounded supervised tree, two distinct real-component fixtures. A
// failed editor batch must not silently omit the existing catalog controls.
// Each fixture gets a fresh Node lifetime; only the owning supervisor can
// acknowledge that its previously observed descendants have become quiescent.
import { runSupervisedGrokBatch } from './lib/grok-batch-lifecycle.mjs';
await runSupervisedGrokBatch();
