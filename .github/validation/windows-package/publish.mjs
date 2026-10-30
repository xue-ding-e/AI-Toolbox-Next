// Validate downloaded same-run artifacts before a separate authorized release job.
// This module never accesses credentials, creates releases, or replaces assets.
import { lstat, readdir, readFile } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPOSITORY = 'xue-ding-e/AI-Toolbox-Next';
const SHA = /^[a-f0-9]{40}$(?![\s\S])/;
const VERSION = /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[a-zA-Z0-9]+(?:[.-][a-zA-Z0-9]+)*)?$(?![\s\S])/;
function requireValue(condition, message) {
  if (!condition) throw new Error(message);
}
async function regularFile(path, maximum) {
  const stat = await lstat(path);
  requireValue(stat.isFile() && !stat.isSymbolicLink() && stat.size > 0 && stat.size <= maximum, 'Invalid artifact file type or size');
  return stat;
}
export async function validateArtifacts(directory, expected) {
  const { sha, version, runId, runAttempt } = expected;
  requireValue(typeof sha === 'string' && SHA.test(sha), 'Invalid expected source SHA');
  requireValue(typeof version === 'string' && version.length <= 100 && VERSION.test(version), 'Invalid expected version');
  requireValue(typeof runId === 'string' && /^[1-9]\d*$(?![\s\S])/.test(runId), 'Invalid expected workflow run');
  requireValue(typeof runAttempt === 'string' && /^[1-9]\d*$(?![\s\S])/.test(runAttempt), 'Invalid expected workflow attempt');
  const root = resolve(directory);
  const rootStat = await lstat(root);
  requireValue(rootStat.isDirectory() && !rootStat.isSymbolicLink(), 'Invalid artifact directory');
  const installer = `AI-Toolbox-Next_${version}_${sha.slice(0, 12)}_windows-x64-setup.exe`;
  const names = [installer, 'SHA256SUMS', 'build-provenance.json'];
  const entries = await readdir(root);
  requireValue(entries.length === names.length && names.every(name => entries.includes(name)), 'Unexpected or missing artifacts');
  const exeStat = await regularFile(join(root, installer), 1024 * 1024 * 1024);
  await regularFile(join(root, 'SHA256SUMS'), 1024);
  await regularFile(join(root, 'build-provenance.json'), 64 * 1024);
  const provenance = JSON.parse(await readFile(join(root, 'build-provenance.json'), 'utf8'));
  requireValue(provenance && provenance.repository === REPOSITORY && provenance.source_commit === sha && SHA.test(provenance.source_tree ?? ''), 'Provenance source mismatch');
  requireValue(provenance.version === version && provenance.build_label === `${version}+${sha.slice(0, 12)}` && provenance.installer === installer, 'Provenance version or filename mismatch');
  requireValue(provenance.workflow_run === runId && provenance.workflow_attempt === runAttempt, 'Provenance workflow mismatch');
  requireValue(Number.isSafeInteger(provenance.bytes) && provenance.bytes === exeStat.size && exeStat.size >= 2, 'Provenance size mismatch');
  requireValue(typeof provenance.sha256 === 'string' && /^[a-f0-9]{64}$(?![\s\S])/.test(provenance.sha256), 'Invalid provenance digest');
  requireValue(provenance.signed === false && provenance.update_mode === 'manual' && provenance.installation_verified === false && provenance.native_gui_verified === false, 'Unexpected validation or signing claims');
  const sums = await readFile(join(root, 'SHA256SUMS'), 'utf8');
  requireValue(sums === `${provenance.sha256}  ${installer}\n`, 'Checksum manifest mismatch');
  const hash = createHash('sha256');
  let bytes = 0;
  let magic = Buffer.alloc(0);
  for await (const chunk of createReadStream(join(root, installer))) {
    bytes += chunk.length;
    requireValue(bytes <= exeStat.size, 'Installer changed during validation');
    if (magic.length < 2) magic = Buffer.concat([magic, chunk.subarray(0, 2 - magic.length)]);
    hash.update(chunk);
  }
  requireValue(bytes === exeStat.size && magic.toString('ascii') === 'MZ', 'Invalid Windows executable');
  requireValue(hash.digest('hex') === provenance.sha256, 'Installer digest mismatch');
  return { repository: REPOSITORY, source_commit: sha, version,
    tag: `v${version}-build.${sha.slice(0, 12)}`, installer, bytes,
    sha256: provenance.sha256, workflow_run: runId, workflow_attempt: runAttempt,
    assets: names.map(name => join(root, name)), signed: false,
    installation_verified: false, native_gui_verified: false };
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const [mode, directory, sha, version, runId, runAttempt, ...extra] = process.argv.slice(2);
    requireValue(mode === 'validate' && directory && extra.length === 0, 'Usage: publish.mjs validate DIR SHA VERSION RUN_ID RUN_ATTEMPT');
    console.log(JSON.stringify(await validateArtifacts(directory, { sha, version, runId, runAttempt })));
  } catch (error) {
    console.error(`Artifact validation failed: ${error.message}`);
    process.exitCode = 1;
  }
}
