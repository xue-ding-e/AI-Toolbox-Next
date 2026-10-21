import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {runBrowserFixture} from './lib/browser-fixture.mjs';
import {verifySSHRequestRace} from '../web/test/features/settings/sshRequest/sshRequestBrowserChecks.mjs';
const fixtureDirectory=fileURLToPath(new URL('../web/test/features/settings/sshRequest/fixtures/',import.meta.url));
await runBrowserFixture({fixtureDirectory,fixtureFilename:'SSHRequestFixture.jsx',artifactPrefix:'ssh-request-race-',verify:verifySSHRequestRace,fixtureAliases:[{find:/^@\/services\/sshSyncApi$/,replacement:path.join(fixtureDirectory,'boundary.mjs')},{find:/^@\/stores$/,replacement:path.join(fixtureDirectory,'boundary.mjs')},{find:/^@tauri-apps\/api\/(event|core)$/,replacement:path.join(fixtureDirectory,'boundary.mjs')}]});
