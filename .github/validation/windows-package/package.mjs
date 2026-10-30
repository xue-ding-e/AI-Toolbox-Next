import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync, mkdirSync, copyFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

export function validateIdentity(config, sha, head, repository) {
  if (!/^[a-f0-9]{40}$/.test(sha ?? '') || sha !== head) throw new Error('Source SHA mismatch');
  if (repository !== 'xue-ding-e/AI-Toolbox-Next') throw new Error('Repository mismatch');
  if (config.identifier !== 'com.xue-ding-e.ai-toolbox-next') throw new Error('Installer identity mismatch');
  if (config.bundle?.createUpdaterArtifacts !== false || config.plugins?.updater) throw new Error('Updater must remain disabled');
  if (config.plugins?.['deep-link']?.desktop?.schemes?.includes('aitoolbox')) throw new Error('Upstream protocol forbidden');
  if (!/^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?$/.test(config.version)) throw new Error('Invalid version');
  return `AI-Toolbox-Next_${config.version}_${sha.slice(0, 12)}_windows-x64-setup.exe`;
}

export function validateNativeResult(log, exit, filter, expected) {
  if (exit !== 0) throw new Error('Native test process failed');
  const successes = [...log.matchAll(/^test ([\w:]+) \.\.\. ok\r?$/gm)].map(x => x[1]);
  if (successes.length !== expected || successes.some(x => !x.startsWith(filter)) || new Set(successes).size !== expected) throw new Error('Native test identity/count mismatch');
  if (!log.includes(`test result: ok. ${expected} passed; 0 failed; 0 ignored;`)) throw new Error('Native summary missing');
}

function main(mode) {
  const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
  const config = JSON.parse(readFileSync('tauri/tauri.conf.json', 'utf8'));
  const sha = process.env.GITHUB_SHA;
  const name = validateIdentity(config, sha, git('rev-parse', 'HEAD'), process.env.GITHUB_REPOSITORY);
  if (process.env.VITE_BUILD_SHA !== sha) throw new Error('Visible build identity mismatch');
  if (git('status', '--porcelain', '--untracked-files=normal')) throw new Error('Source changed or untracked non-ignored files appeared during build');
  if (mode === 'verify') return;
  if (mode !== 'seal') throw new Error('Unknown command');
  const dir = 'tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis';
  const files = readdirSync(dir).filter(x => x.endsWith('-setup.exe'));
  if (files.length !== 1) throw new Error('Expected exactly one installer');
  const bytes = readFileSync(`${dir}/${files[0]}`);
  if (bytes.length < 2 || bytes.subarray(0, 2).toString() !== 'MZ') throw new Error('Invalid Windows executable');
  const sha256 = createHash('sha256').update(bytes).digest('hex');
  mkdirSync('release-assets');
  copyFileSync(`${dir}/${files[0]}`, `release-assets/${name}`);
  writeFileSync('release-assets/SHA256SUMS', `${sha256}  ${name}\n`);
  writeFileSync('release-assets/build-provenance.json', JSON.stringify({
    repository: process.env.GITHUB_REPOSITORY, source_commit: sha, source_tree: git('rev-parse', 'HEAD^{tree}'),
    version: config.version, build_label: `${config.version}+${sha.slice(0,12)}`, installer: name, bytes: bytes.length, sha256,
    workflow_run: process.env.GITHUB_RUN_ID, workflow_attempt: process.env.GITHUB_RUN_ATTEMPT,
    toolchain: { node: process.version, rust: execFileSync('rustc', ['--version'], { encoding: 'utf8' }).trim() },
    frontend_tests: 'pnpm test', native_tests: '3 account export + 9 OAuth diagnostic + 5 HTTP client',
    signed: false, update_mode: 'manual', installation_verified: false, native_gui_verified: false,
  }, null, 2) + '\n');
}
if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv[2]);
