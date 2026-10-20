import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyDeepLinkHandshake } from '../web/test/features/shared/deepLink/deepLinkHandshakeBrowserChecks.mjs';
const fixtureDirectory = fileURLToPath(new URL('../web/test/features/shared/deepLink/fixtures/', import.meta.url));
await runBrowserFixture({
  fixtureDirectory, fixtureFilename: 'DeepLinkHandshakeFixture.jsx', artifactPrefix: 'deeplink-handshake-',
  verify: verifyDeepLinkHandshake,
  fixtureAliases: [
    { find: /^@tauri-apps\/api\/event$/, replacement: path.join(fixtureDirectory, 'tauri.mjs') },
    { find: /^@\/services\/deeplinkApi$/, replacement: path.join(fixtureDirectory, 'tauri.mjs') },
  ],
});
