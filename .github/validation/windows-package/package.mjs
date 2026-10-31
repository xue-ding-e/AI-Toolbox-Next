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
  const sha = process.env.SOURCE_SHA ?? process.env.GITHUB_SHA;
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
  const owner = JSON.parse(readFileSync(`${process.env.RUNNER_TEMP}/automation-lifecycle.result.json`, 'utf8'));
  if (owner.exitCode !== 0 || owner.timedOut !== false || owner.outputExceeded !== false || owner.quiescenceConfirmed !== true || owner.naturalQuiescence !== true || owner.toolCleanup !== null) throw new Error('Installed automation process tree did not exit naturally');
  const automationInput = readFileSync('tauri/target/automation-sidecars/ai-toolbox-cli-x86_64-pc-windows-msvc.exe');
  const automationSha = createHash('sha256').update(automationInput).digest('hex');
  const automationVersion = readFileSync('crates/ai-toolbox-automation/Cargo.toml', 'utf8').match(/^version = "([^"]+)"$/m)?.[1];
  const lifecycle = JSON.parse(readFileSync(`${process.env.RUNNER_TEMP}/automation-install.json`, 'utf8'));
  const hashFile = path => createHash('sha256').update(readFileSync(path)).digest('hex');
  const binding = owner.binding;
  if (!binding || binding.source_commit !== sha || binding.control_commit !== process.env.GITHUB_SHA || binding.workflow_run !== process.env.GITHUB_RUN_ID || binding.workflow_attempt !== process.env.GITHUB_RUN_ATTEMPT || binding.installer_sha256 !== sha256 || binding.helper_sha256 !== hashFile('.github/validation/windows-package/windows-owned-process.ps1') || binding.root_script !== '.github/validation/windows-package/automation-install.mjs' || binding.root_script_sha256 !== hashFile(binding.root_script) || binding.lifecycle_sha256 !== hashFile(`${process.env.RUNNER_TEMP}/automation-install.json`) || binding.compiler_exception_enabled !== false || !/^[a-f0-9]{32}$/.test(binding.invocation_id ?? '') || binding.invocation_id !== lifecycle.invocation_id) throw new Error('Owned lifecycle receipt binding mismatch');
  if (lifecycle.source_commit !== sha || lifecycle.installer_sha256 !== sha256 || lifecycle.automation?.sha256 !== automationSha || lifecycle.automation?.cli_mcp_installed_calls_verified !== true || lifecycle.synthetic_install_verified !== true || lifecycle.same_version_reinstall_verified !== true || lifecycle.synthetic_sentinel_preserved !== true || lifecycle.registry_path_unchanged !== true || lifecycle.uninstall_console_removed !== true || lifecycle.gui_only_upgrade_verified !== true || lifecycle.previous_installer_sha256 !== '1451c7cf8eec647edaaf284d6ab451a65b2433d2867cb201d4554310f590a658' || lifecycle.foreign_install_file_preserved !== true || lifecycle.external_roots_unchanged !== true || lifecycle.uninstall_registration_verified !== true) throw new Error('Installed automation provenance mismatch');
  if (lifecycle.automation?.version?.ok !== true || lifecycle.automation?.version?.data?.name !== 'ai-toolbox-cli' || lifecycle.automation?.version?.data?.version !== automationVersion) throw new Error('Installed automation version mismatch');
  if (!automationVersion) throw new Error('Automation package version missing');
  mkdirSync('release-assets');
  copyFileSync(`${dir}/${files[0]}`, `release-assets/${name}`);
  writeFileSync('release-assets/SHA256SUMS', `${sha256}  ${name}\n`);
  writeFileSync('release-assets/build-provenance.json', JSON.stringify({
    repository: process.env.GITHUB_REPOSITORY, source_commit: sha, source_tree: git('rev-parse', 'HEAD^{tree}'),
    version: config.version, build_label: `${config.version}+${sha.slice(0,12)}`, installer: name, bytes: bytes.length, sha256,
    workflow_run: process.env.GITHUB_RUN_ID, workflow_attempt: process.env.GITHUB_RUN_ATTEMPT,
    toolchain: { node: process.version, rust: execFileSync('rustc', ['--version'], { encoding: 'utf8' }).trim() },
    frontend_tests: 'pnpm test', native_tests: '3 account export + 9 OAuth diagnostic + 5 HTTP client',
    automation: { executable: 'ai-toolbox-cli.exe', sha256: automationSha, bytes: automationInput.length,
      ownership: { binding, exit_code: owner.exitCode, natural_quiescence: owner.naturalQuiescence, quiescence_confirmed: owner.quiescenceConfirmed, timed_out: owner.timedOut, output_exceeded: owner.outputExceeded, tool_cleanup: owner.toolCleanup },
      package_version: automationVersion, mcp_args: ['mcp', 'serve'], lifecycle },
    signed: false, update_mode: 'manual', installation_verified: false, native_gui_verified: false,
  }, null, 2) + '\n');
}
if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv[2]);
