import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {runBrowserFixture} from '../../../../../scripts/lib/browser-fixture.mjs';
import {verifySSHRequestRace} from '../sshRequest/sshRequestBrowserChecks.mjs';
import {verifySSHCapability,verifySSHCapabilityBaseline} from './sshCapabilityBrowserChecks.mjs';
const mode=process.env.SSH_CAPABILITY_CHECK_MODE || 'full';
if(!['full','baseline'].includes(mode))throw Error('Unknown SSH_CAPABILITY_CHECK_MODE');
const fixtureDirectory=fileURLToPath(new URL('../sshRequest/fixtures/',import.meta.url));
await runBrowserFixture({
  fixtureDirectory,fixtureFilename:'SSHRequestFixture.jsx',artifactPrefix:'ssh-capability-',
  verify:async options=>{
    const failures=[];const checks=[];
    for(const verify of mode==='baseline'?[verifySSHCapabilityBaseline]:[verifySSHRequestRace,verifySSHCapability]) {
      try {checks.push(...await verify(options));} catch(error) {failures.push(error);}
    }
    if(failures.length)throw new AggregateError(failures,'SSH fixture expectations failed');
    return checks;
  },
  fixtureAliases:[
    {find:/^@\/services\/sshSyncApi$/,replacement:path.join(fixtureDirectory,'boundary.mjs')},
    {find:/^@\/stores$/,replacement:path.join(fixtureDirectory,'boundary.mjs')},
    {find:/^@tauri-apps\/api\/(event|core)$/,replacement:path.join(fixtureDirectory,'boundary.mjs')},
  ],
});
