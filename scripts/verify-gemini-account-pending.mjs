import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyGeminiAccountPending } from '../web/test/features/coding/shared/providerCardVariants/geminiAccountPendingBrowserChecks.mjs';

await runBrowserFixture({
  fixtureDirectory: fileURLToPath(new URL('../web/test/features/coding/shared/providerCardVariants/fixtures', import.meta.url)),
  fixtureFilename: 'GeminiAccountPendingFixture.jsx',
  artifactPrefix: 'gemini-account-pending-',
  verify: verifyGeminiAccountPending,
});
