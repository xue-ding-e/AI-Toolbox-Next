import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyGrokFetchedModels } from '../web/test/features/coding/grok/fetchedModels/grokFetchedModelsBrowserChecks.mjs';
await runBrowserFixture({
  fixtureDirectory: fileURLToPath(new URL('../web/test/features/coding/grok/fetchedModels/fixtures/', import.meta.url)),
  fixtureFilename: 'GrokFetchModelsFixture.jsx',
  artifactPrefix: 'grok-fetched-models-',
  verify: verifyGrokFetchedModels,
});
