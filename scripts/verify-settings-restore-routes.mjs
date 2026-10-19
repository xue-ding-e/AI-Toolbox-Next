import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifySettingsRestoreRoutes } from '../web/test/features/settings/settingsRestoreRoutesBrowserChecks.mjs';
import { verifySettingsRestoreErrors } from '../web/test/features/settings/settingsRestoreErrorsBrowserChecks.mjs';

const group = process.env.SETTINGS_RESTORE_CHECK_GROUP || 'routes';
if (!['routes', 'error-recovery'].includes(group)) {
  throw new Error('Unknown settings restore check group: ' + group);
}

const fixtureDirectory = fileURLToPath(new URL(
  '../web/test/features/settings/fixtures/restoreRoutes/', import.meta.url,
));

await runBrowserFixture({
  fixtureDirectory,
  fixtureFilename: 'GeneralSettingsRestoreFixture.mjs',
  artifactPrefix: 'settings-restore-' + group + '-',
  verify: group === 'error-recovery' ? verifySettingsRestoreErrors : verifySettingsRestoreRoutes,
  fixtureAliases: [
    // Narrow barrels only re-export production services/stores. The page,
    // children, confirmation, password prompt and restore flow are not aliased.
    { find: /^@\/services$/, replacement: path.join(fixtureDirectory, 'services.ts') },
    { find: /^@\/stores$/, replacement: path.join(fixtureDirectory, 'stores.ts') },
    { find: /^@tauri-apps\/plugin-os$/, replacement: path.join(fixtureDirectory, 'tauri.mjs') },
    { find: /^@tauri-apps\/api\/event$/, replacement: path.join(fixtureDirectory, 'tauri.mjs') },
  ],
});
