import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { validateArtifacts } from './publish.mjs';
const expected = { sha: 'a'.repeat(40), version: '1.2.1-next.1', runId: '123', runAttempt: '2' };
async function fixture(t) {
  const dir = await mkdtemp(join(tmpdir(), 'aitb-publication-test-'));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const installer = `AI-Toolbox-Next_${expected.version}_${expected.sha.slice(0, 12)}_windows-x64-setup.exe`;
  const bytes = Buffer.from('MZsynthetic-not-a-real-installer');
  const digest = createHash('sha256').update(bytes).digest('hex');
  const provenance = { repository: 'xue-ding-e/AI-Toolbox-Next', source_commit: expected.sha, source_tree: 'b'.repeat(40), version: expected.version, build_label: `${expected.version}+${expected.sha.slice(0, 12)}`, installer, bytes: bytes.length, sha256: digest, workflow_run: expected.runId, workflow_attempt: expected.runAttempt, signed: false, update_mode: 'manual', installation_verified: false, native_gui_verified: false };
  await writeFile(join(dir, installer), bytes);
  await writeFile(join(dir, 'SHA256SUMS'), `${digest}  ${installer}\n`);
  await writeFile(join(dir, 'build-provenance.json'), JSON.stringify(provenance));
  return { dir, installer, provenance };
}
test('accepts sealed same-run fixture and derives deterministic immutable tag', async t => {
  const { dir, installer } = await fixture(t);
  const result = await validateArtifacts(dir, expected);
  assert.equal(result.tag, 'v1.2.1-next.1-build.aaaaaaaaaaaa');
  assert.equal(result.installer, installer);
  assert.equal(result.assets.length, 3);
  assert.equal(result.installation_verified, false);
});
for (const [field, value] of Object.entries({ repository: 'other/repo', source_commit: 'c'.repeat(40), source_tree: '', version: '1.2.2', build_label: 'wrong', installer: '../bad.exe', workflow_run: '124', workflow_attempt: '1', bytes: 1, sha256: 'f'.repeat(64), signed: true, installation_verified: true, native_gui_verified: true, update_mode: 'auto' })) {
  test(`rejects provenance ${field} mismatch`, async t => {
    const { dir, provenance } = await fixture(t);
    await writeFile(join(dir, 'build-provenance.json'), JSON.stringify({ ...provenance, [field]: value }));
    await assert.rejects(validateArtifacts(dir, expected));
  });
}
for (const kind of ['extra', 'missing', 'digest', 'sums', 'symlink', 'empty', 'malformed']) {
  test(`rejects ${kind} artifact`, async t => {
    const { dir, installer } = await fixture(t);
    if (kind === 'extra') await writeFile(join(dir, 'unexpected.txt'), 'x');
    if (kind === 'missing') await rm(join(dir, installer));
    if (kind === 'digest') { const b = await readFile(join(dir, installer)); b[b.length - 1] ^= 1; await writeFile(join(dir, installer), b); }
    if (kind === 'sums') await writeFile(join(dir, 'SHA256SUMS'), 'wrong');
    if (kind === 'symlink') { await rm(join(dir, installer)); await symlink('SHA256SUMS', join(dir, installer)); }
    if (kind === 'empty') await writeFile(join(dir, installer), '');
    if (kind === 'malformed') await writeFile(join(dir, 'build-provenance.json'), '{');
    await assert.rejects(validateArtifacts(dir, expected));
  });
}
test('rejects unsafe expected identity inputs before artifact reads', async t => {
  const { dir } = await fixture(t);
  for (const changes of [{ sha: 'x' }, { version: '../x' }, { version: '1.2.3\n' }, { version: '1.2.3--' }, { runId: '0' }, { runAttempt: '2\n' }]) {
    await assert.rejects(validateArtifacts(dir, { ...expected, ...changes }));
  }
});
test('rejects non-MZ payload even with matching digest and size', async t => {
  const { dir, installer, provenance } = await fixture(t);
  const bytes = Buffer.from('not-an-executable');
  provenance.bytes = bytes.length;
  provenance.sha256 = createHash('sha256').update(bytes).digest('hex');
  await writeFile(join(dir, installer), bytes);
  await writeFile(join(dir, 'SHA256SUMS'), `${provenance.sha256}  ${installer}\n`);
  await writeFile(join(dir, 'build-provenance.json'), JSON.stringify(provenance));
  await assert.rejects(validateArtifacts(dir, expected), /Invalid Windows executable/);
});
test('rejects oversized provenance before parsing', async t => {
  const { dir } = await fixture(t);
  await writeFile(join(dir, 'build-provenance.json'), ' '.repeat(65537));
  await assert.rejects(validateArtifacts(dir, expected), /file type or size/);
});
