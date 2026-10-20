import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyOfficialAccountPending } from '../web/test/features/coding/shared/providerCardVariants/officialAccountPendingBrowserChecks.mjs';

await runBrowserFixture({
  fixtureDirectory: fileURLToPath(new URL('../web/test/features/coding/shared/providerCardVariants/fixtures', import.meta.url)),
  fixtureFilename: 'OfficialAccountPendingFixture.jsx',
  artifactPrefix: 'official-account-pending-',
  verify: verifyOfficialAccountPending,
});
