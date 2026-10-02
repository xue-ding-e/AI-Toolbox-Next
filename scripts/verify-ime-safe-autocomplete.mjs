import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyImeSafeAutoComplete } from '../web/test/components/common/ImeSafeAutoComplete/imeSafeAutoCompleteBrowserChecks.mjs';

const fixtureDirectory = fileURLToPath(new URL('../web/test/components/common/ImeSafeAutoComplete/fixtures', import.meta.url));
await runBrowserFixture({
  fixtureDirectory,
  fixtureFilename: 'ImeSafeAutoCompleteFixture.jsx',
  artifactPrefix: 'ime-safe-autocomplete-',
  verify: verifyImeSafeAutoComplete,
});
