import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyJsonEditorBlur } from '../web/test/components/common/JsonEditor/jsonEditorBlurBrowserChecks.mjs';

const fixtureDirectory = fileURLToPath(new URL('../web/test/components/common/JsonEditor/fixtures', import.meta.url));
await runBrowserFixture({
  fixtureDirectory,
  fixtureFilename: 'JsonEditorBlurFixture.jsx',
  artifactPrefix: 'json-editor-blur-',
  verify: verifyJsonEditorBlur,
  fixtureAliases: [
    {
      find: 'monaco-editor/esm/vs/language/json/monaco.contribution',
      replacement: path.join(fixtureDirectory, 'JsonLanguageContributionStub.js'),
    },
  ],
});