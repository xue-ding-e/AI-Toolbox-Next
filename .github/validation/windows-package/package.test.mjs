import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validateIdentity, validateNativeResult } from './package.mjs';
const sha='a'.repeat(40);
const config={version:'1.2.1-next.1',identifier:'com.xue-ding-e.ai-toolbox-next',bundle:{createUpdaterArtifacts:false},plugins:{}};
test('identity includes commit while retaining app version',()=>assert.equal(validateIdentity(config,sha,sha,'xue-ding-e/AI-Toolbox-Next'),'AI-Toolbox-Next_1.2.1-next.1_aaaaaaaaaaaa_windows-x64-setup.exe'));
for (const [name,c,s,h,r] of [
 ['wrong SHA',config,sha,'b'.repeat(40),'xue-ding-e/AI-Toolbox-Next'],
 ['malformed SHA',config,'../x','../x','xue-ding-e/AI-Toolbox-Next'],
 ['wrong repo',config,sha,sha,'coulsontl/ai-toolbox'],
 ['updater',{...config,plugins:{updater:{}}},sha,sha,'xue-ding-e/AI-Toolbox-Next'],
 ['official identity',{...config,identifier:'com.ai.toolbox'},sha,sha,'xue-ding-e/AI-Toolbox-Next'],
 ['unsafe version',{...config,version:'../bad'},sha,sha,'xue-ding-e/AI-Toolbox-Next'],
]) test(`rejects ${name}`,()=>assert.throws(()=>validateIdentity(c,s,h,r)));
const good='test m::a ... ok\ntest m::b ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 ignored; 3 filtered out;';
test('native positive and fail-closed controls',()=>{
 validateNativeResult(good,0,'m::',2);
 for(const [log,exit] of [[good,1],['',0],[good.replace('m::b','m::a'),0],[good.replace('m::b','other::b'),0],[good.replace('0 ignored','1 ignored'),0]]) assert.throws(()=>validateNativeResult(log,exit,'m::',2));
});

test('workflow retains least privilege and ordered exact-source gates', () => {
  const workflow = readFileSync(new URL('../../workflows/windows-manual-build.yml', import.meta.url), 'utf8');
  assert.match(workflow, /branches: \[main\]/);
  assert.match(workflow, /contents: read/);
  const jobEnv = workflow.slice(workflow.indexOf('    env:'), workflow.indexOf('    steps:'));
  assert.doesNotMatch(jobEnv, /runner\./, 'runner context is unavailable in job-level env');
  assert.match(workflow, /AI_TOOLBOX_DATA_DIR=\$data/);
  assert.match(workflow, /\$env:GITHUB_ENV/);
  assert.match(workflow, /ref: \$\{\{ github.sha \}\}/);
  assert.match(workflow, /persist-credentials: false/);
  assert.doesNotMatch(workflow, /pull_request_target|secrets\.|contents: write|actions: write|workflow_run:/);
  assert(workflow.indexOf('pnpm test') < workflow.indexOf('pnpm tauri build'));
  assert(workflow.indexOf('node scripts/verify-codex-auth-entry.mjs') < workflow.indexOf('pnpm tauri build'));
  assert(workflow.indexOf('test-native.ps1') < workflow.indexOf('pnpm tauri build'));
  assert(workflow.indexOf('package.mjs seal') > workflow.indexOf('pnpm tauri build'));
});
