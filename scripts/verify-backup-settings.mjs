import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';
import { verifyBackupSettings } from '../web/test/features/settings/backupSettingsBrowserChecks.mjs';

const fixtureDirectory = fileURLToPath(new URL("../web/test/features/settings/fixtures", import.meta.url));
await runBrowserFixture({
  fixtureDirectory,
  fixtureFilename: "BackupSettingsFixture.jsx",
  artifactPrefix: "backup-settings-",
  verify: verifyBackupSettings,
  fixtureAliases: [
    { find: /^@\/services$/, replacement: path.join(fixtureDirectory, 'services.ts') },
    { find: /^@\/stores$/, replacement: path.join(fixtureDirectory, 'stores.ts') },
  ],
});
