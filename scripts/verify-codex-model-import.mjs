import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyCodexModelImport } from '../web/test/features/coding/codex/components/codexModelImportBrowserChecks.mjs';

const fixtureDirectory = fileURLToPath(new URL("../web/test/features/coding/codex/components/fixtures", import.meta.url));
await runBrowserFixture({
  fixtureDirectory,
  fixtureFilename: "CodexModelImportFixture.jsx",
  artifactPrefix: "codex-model-import-",
  verify: verifyCodexModelImport,
  fixtureAliases: [
    { find: '@/components/common/TomlEditor', replacement: path.join(fixtureDirectory, 'TomlEditorStub.jsx') },
  ],
});
