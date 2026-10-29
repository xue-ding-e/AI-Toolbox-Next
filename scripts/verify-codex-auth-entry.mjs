import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyCodexAuthEntry } from '../web/test/features/coding/codex/codexAuthEntryBrowserChecks.mjs';
await runBrowserFixture({
  fixtureDirectory: fileURLToPath(new URL('../web/test/features/coding/codex/fixtures', import.meta.url)),
  fixtureFilename: 'CodexCardViewFixture.jsx',
  artifactPrefix: 'codex-auth-entry-',
  verify: verifyCodexAuthEntry,
});
