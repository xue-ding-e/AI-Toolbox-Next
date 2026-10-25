import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyGrokEditor } from '../web/test/features/coding/grok/editorReadback/browserChecks.mjs';
await runBrowserFixture({ fixtureDirectory: fileURLToPath(new URL('../web/test/features/coding/grok/editorReadback/fixtures/', import.meta.url)), fixtureFilename: 'GrokEditorFixture.jsx', artifactPrefix: 'grok-editor-', browserDiagnostics: true, fixtureAliases: [{ find: /^@\/app\/routes$/, replacement: fileURLToPath(new URL('../web/test/features/coding/grok/editorReadback/fixtures/routerBoundary.mjs', import.meta.url)) }], verify: verifyGrokEditor });
