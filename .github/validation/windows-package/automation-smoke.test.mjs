import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { validateResponses } from './automation-smoke.mjs';
const cli = { ok: true, data: { cli_keys: ['fixture'] } };
const good = [
  { id: 1, result: { protocolVersion: '2025-11-25' } },
  { id: 2, result: { tools: [{name:'backup_normalize_path'}, {name:'gateway_supported_cli_keys'}] } },
  { id: 3, result: { isError: false, structuredContent: cli.data } },
  { id: 4, result: { isError: false, structuredContent: {path:'~/missing/中文'} } },
  { id: 5, error: {code:-32602} },
];
test('installed transport response gate rejects partial or forged success', () => {
  validateResponses(cli, good);
  for (const mutate of [x => x.pop(), x => x[0].result.protocolVersion = 'old', x => x[2].result.isError = true, x => x[3].result.structuredContent.path = 'wrong', x => x[4].error.code = 0, x => x[2].result.structuredContent.cli_keys = []]) {
    const bad = structuredClone(good); mutate(bad);
    assert.throws(() => validateResponses(cli, bad));
  }
});
test('Windows packaging builds console before bundling and exercises installed bytes before sealing', () => {
  const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
  const workflow = read('../../workflows/windows-manual-build.yml');
  assert(workflow.indexOf('cargo build --locked --release --manifest-path crates/ai-toolbox-automation') < workflow.indexOf('pnpm tauri build'));
  assert(workflow.indexOf('$result = Invoke-NextOwnedProcess') > workflow.indexOf('pnpm tauri build'));
  assert(workflow.indexOf('$result = Invoke-NextOwnedProcess') < workflow.indexOf('package.mjs seal'));
  assert.match(workflow, /--config tauri\/tauri.automation.conf.json/);
  const overlay = JSON.parse(read('../../../tauri/tauri.automation.conf.json'));
  assert.deepEqual(overlay, {bundle:{externalBin:['target/automation-sidecars/ai-toolbox-cli']}});
  assert.match(read('./automation-install.mjs'), /join\(install, 'ai-toolbox-cli.exe'\)/);
});

test('bounded runner observes real success, failure, timeout and output overflow', async () => {
  const { run } = await import('./automation-smoke.mjs');
  const success = await run(process.execPath, ['-e', 'process.stdout.write("ok")'], {});
  assert.equal(success.code, 0); assert.equal(success.out, 'ok');
  assert.equal((await run(process.execPath, ['-e', 'process.exit(7)'], {})).code, 7);
  await assert.rejects(run(process.execPath, ['-e', 'setInterval(()=>{},100)'], {}, '', 100), /deadline/);
  await assert.rejects(run(process.execPath, ['-e', 'process.stdout.write("x".repeat(2*1024*1024));setInterval(()=>{},100)'], {}), /output/);
});

test('synthetic timeout diagnostics are opt-in, bounded and contain no environment', async () => {
  const { run } = await import('./automation-smoke.mjs');
  const argv = ['-e', 'process.stderr.write("d".repeat(6000));setInterval(()=>{},100)'];
  await assert.rejects(run(process.execPath, argv, { FIXTURE_PRIVATE: 'not-logged' }, '', 1000, false, true), error => {
    assert.match(error.message, /deadline/);
    assert.equal(error.processEvidence.executable, process.execPath);
    assert.deepEqual(error.processEvidence.argv, argv);
    assert.equal(error.processEvidence.timeout_ms, 1000);
    assert.equal(error.processEvidence.stderr.length, 4096);
    assert(!JSON.stringify(error.processEvidence).includes('not-logged'));
    return true;
  });
  await assert.rejects(run(process.execPath, ['-e', 'setInterval(()=>{},100)'], {}, '', 100), error => {
    assert.equal(error.processEvidence, undefined); return true;
  });
});

test('Windows lifecycle reuses the reviewed Job Object owner without compiler exceptions', async () => {
  const { createHash } = await import('node:crypto');
  const owner = readFileSync(new URL('./windows-owned-process.ps1', import.meta.url), 'utf8');
  const original = owner;
  assert.equal(createHash('sha256').update(original).digest('hex'), '9460335fec78be0453ce8ec294a21fa78a3b4105e45913b398951cfd19965c6f');
  assert.match(owner, /0x0002000d/);
  assert.match(owner, /0x08080004/);
  assert.doesNotMatch(owner, /AssignProcessToJobObject/);
  assert.match(owner, /Copyright \(c\) 2026 DeepSeek/);
  const workflow = readFileSync(new URL('../../workflows/windows-manual-build.yml', import.meta.url), 'utf8');
  assert.match(workflow, /Invoke-NextOwnedProcess -FilePath/);
  assert.doesNotMatch(workflow, /-VerifiedToolPath|-VerifiedToolSha256/);
  assert(workflow.indexOf('windows-owned-process.test.ps1') < workflow.indexOf('$result = Invoke-NextOwnedProcess'));
});

test('diagnostic capture enforces UTF-8 bytes for multibyte text', async () => {
  const { boundedUtf8 } = await import('./automation-smoke.mjs');
  for (const input of ['中'.repeat(3000), '🙂'.repeat(3000), 'a'.repeat(4095) + '中']) {
    const result = boundedUtf8(input);
    assert(Buffer.byteLength(result, 'utf8') <= 4096);
    assert(input.startsWith(result)); assert(!result.includes('�'));
  }
  assert.equal(boundedUtf8('a'.repeat(5000)).length, 4096);
});
