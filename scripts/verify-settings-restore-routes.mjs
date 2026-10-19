import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifySettingsRestoreRoutes } from '../web/test/features/settings/settingsRestoreRoutesBrowserChecks.mjs';

const fixtureDirectory = fileURLToPath(new URL(
  '../web/test/features/settings/fixtures/restoreRoutes/', import.meta.url,
));

await runBrowserFixture({
  fixtureDirectory,
  fixtureFilename: 'GeneralSettingsRestoreFixture.mjs',
  artifactPrefix: 'settings-restore-routes-',
  verify: verifySettingsRestoreRoutes,
  fixtureAliases: [
    // Narrow barrels only re-export production services/stores. The page,
    // children, confirmation, password prompt and restore flow are not aliased.
    { find: /^@\/services$/, replacement: path.join(fixtureDirectory, 'services.ts') },
    { find: /^@\/stores$/, replacement: path.join(fixtureDirectory, 'stores.ts') },
    { find: /^@tauri-apps\/plugin-os$/, replacement: path.join(fixtureDirectory, 'tauri.mjs') },
    { find: /^@tauri-apps\/api\/event$/, replacement: path.join(fixtureDirectory, 'tauri.mjs') },
  ],
});
