// Cloud Windows synthetic fixture only. Optional previous installer is pinned by hash;
// without it this verifies reinstall, never claims a version upgrade.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, readdirSync, statSync, realpathSync, lstatSync } from 'node:fs';
import { join, resolve, isAbsolute } from 'node:path';
import { run, smoke, digest } from './automation-smoke.mjs';

assert.equal(process.platform, 'win32');
assert.match(process.env.AUTOMATION_INVOCATION_ID ?? '', /^[a-f0-9]{32}$/);
assert.equal(process.env.GITHUB_ACTIONS, 'true', 'Use only disposable cloud Windows workers');
const [installerArg, sourceArg, evidenceArg, previousArg, previousHash, ...extra] = process.argv.slice(2);
assert.equal(extra.length, 0);
assert(installerArg && sourceArg && evidenceArg);
assert.equal(Boolean(previousArg), Boolean(previousHash));
const installer = resolve(installerArg), source = resolve(sourceArg), evidence = resolve(evidenceArg);
if (previousArg) {
  assert(isAbsolute(previousArg));
  assert.match(previousHash, /^[a-f0-9]{64}$/);
  assert.equal(digest(previousArg), previousHash);
  assert.equal(statSync(previousArg).size, 15501542);
}
const generatedNsi = resolve('tauri/target/x86_64-pc-windows-msvc/release/nsis/x64/installer.nsi');
const template = readFileSync(generatedNsi, 'utf8');
const uninstallNames = [...template.matchAll(/^\s*WriteUninstaller "\$INSTDIR\\([a-zA-Z0-9_.-]+\.exe)"\s*$/gm)].map(x => x[1]);
assert.equal(uninstallNames.length, 1, 'Generated NSIS must define one root-local uninstaller');
assert.match(template, /File \/a "\/oname=ai-toolbox-cli\.exe"/);
const mainName = template.match(/^!define MAINBINARYNAME "([a-zA-Z0-9_. -]+)"\s*$/m)?.[1];
assert(mainName, 'Generated NSIS main binary identity missing');
const root = mkdtempSync(join(process.env.RUNNER_TEMP, 'automation-install-'));
const install = join(root, 'Installed App 中文');
const data = join(root, 'profile'); mkdirSync(data);
const sentinel = join(data, 'preserve-fixture.json');
const marker = '{"fixture":"synthetic-only","preserve":true}\n';
writeFileSync(sentinel, marker);
// Inherit only OS necessities, not runner credentials or developer application paths.
// These variables isolate console paths, not Windows Known Folders or HKCU.
// NSIS runs only on the disposable worker and /NS suppresses shortcut creation.
const env = Object.fromEntries(['SystemRoot', 'WINDIR', 'COMSPEC', 'TEMP', 'TMP'].filter(k => process.env[k]).map(k => [k, process.env[k]]));
Object.assign(env, { HOME: join(root, 'home'), USERPROFILE: join(root, 'home'), APPDATA: join(root, 'roaming'), LOCALAPPDATA: join(root, 'local'), AI_TOOLBOX_DATA_DIR: data, AI_TOOLBOX_CACHE_DIR: join(root, 'cache'), CODEX_HOME: join(root, 'codex') });
const registry = `${process.env.SystemRoot}\\System32\\reg.exe`;
async function paths() {
  return Promise.all([['HKCU\\Environment'], ['HKLM\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment']].map(async ([key]) => {
    const result = await run(registry, ['query', key, '/v', 'Path'], env);
    assert([0, 1].includes(result.code));
    return { code: result.code, out: result.out, err: result.err };
  }));
}
async function registeredUninstaller() {
  const uninstaller = join(install, uninstallNames[0]);
  assert.equal(realpathSync(uninstaller).toLowerCase(), uninstaller.toLowerCase(), 'Uninstaller escapes synthetic root');
  const key = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\AI Toolbox Next';
  for (const field of ['UninstallString', 'InstallLocation']) {
    const registration = await run(registry, ['query', key, '/v', field], env);
    assert.equal(registration.code, 0);
    const registered = registration.out.match(new RegExp(`${field}\\s+REG_SZ\\s+"([^"\\r\\n]+)"\\s*$`, 'm'))?.[1];
    const expected = field === 'UninstallString' ? uninstaller : install;
    assert.equal(registered?.toLowerCase(), expected.toLowerCase(), 'Registration is outside the exact synthetic installation');
  }
  return uninstaller;
}
const beforePaths = await paths();
const externalRoots = ['HOME', 'APPDATA', 'LOCALAPPDATA', 'AI_TOOLBOX_CACHE_DIR', 'CODEX_HOME'];
function snapshot(path) {
  if (!existsSync(path)) return null;
  assert(!lstatSync(path).isSymbolicLink(), 'Synthetic tree must not contain links');
  if (statSync(path).isFile()) return digest(path);
  return Object.fromEntries(readdirSync(path).sort().map(name => [name, snapshot(join(path, name))]));
}
const externalBefore = Object.fromEntries(externalRoots.map(key => [key, snapshot(env[key])]));
const foreignName = 'not-owned-by-installer.fixture';
let foreignCreated = false;
const preserved = () => {
  assert.equal(readFileSync(sentinel, 'utf8'), marker);
  assert.deepEqual(readdirSync(data), ['preserve-fixture.json']);
  for (const key of externalRoots) assert.deepEqual(snapshot(env[key]), externalBefore[key], `Unexpected application state: ${key}`);
  if (foreignCreated) assert.equal(readFileSync(join(install, foreignName), 'utf8'), marker);
};
async function requireExistingWebView() {
  // Match the pinned Tauri NSIS x64 registry probe. Do not permit the installer
  // to bootstrap WebView through an updater/service outside our owned Job.
  assert.match(template, /^!define MINIMUMWEBVIEW2VERSION ""\s*$/m, 'Review a changed minimum runtime before accepting broker-free installation');
  for (const key of [
    'HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'HKCU\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
  ]) {
    const found = await run(registry, ['query', key, '/v', 'pv', '/reg:64'], env);
    assert([0, 1].includes(found.code));
    const version = found.out.match(/pv\s+REG_SZ\s+([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+)\s*$/m)?.[1];
    if (found.code === 0 && version && version !== '0.0.0.0') return version;
  }
  throw new Error('Preinstalled WebView2 required; do not invoke the installer bootstrapper');
}
async function installExe(file) {
  await requireExistingWebView();
  // NSIS requires an unquoted final /D argument, even when it contains spaces. Do not pass /R (run application).
  const result = await run(file, ['/S', '/NS', `/D=${install}`], env, '', 180000, true);
  assert.equal(result.code, 0); assert.equal(result.signal, null);
  preserved(); assert.deepEqual(await paths(), beforePaths);
}
let previousGui = null;
if (previousArg) {
  await installExe(previousArg);
  await registeredUninstaller();
  const gui = join(install, `${mainName}.exe`);
  assert.equal(realpathSync(gui).toLowerCase(), gui.toLowerCase());
  assert.equal(readFileSync(gui).subarray(0, 2).toString(), 'MZ');
  previousGui = { executable: `${mainName}.exe`, bytes: statSync(gui).size, sha256: digest(gui) };
  assert.equal(existsSync(join(install, 'ai-toolbox-cli.exe')), false, 'Expected GUI-only upgrade baseline');
}
if (!existsSync(install)) mkdirSync(install);
writeFileSync(join(install, foreignName), marker); foreignCreated = true;
await installExe(installer);
const binary = join(install, 'ai-toolbox-cli.exe');
const first = await smoke(binary, source, env);
preserved();
await installExe(installer);
const second = await smoke(binary, source, env);
assert.deepEqual(second, first);
const uninstaller = join(install, uninstallNames[0]);
assert.equal(realpathSync(uninstaller).toLowerCase(), uninstaller.toLowerCase(), 'Uninstaller must not escape the synthetic root through a link');
await registeredUninstaller();
// _?= avoids the NSIS temporary-copy handoff so the observed child is the uninstaller.
const removed = await run(uninstaller, ['/S', `_?=${install}`], env, '', 180000, true);
assert.equal(removed.code, 0); assert.equal(removed.signal, null);
assert.equal(existsSync(binary), false);
preserved(); assert.deepEqual(await paths(), beforePaths);
writeFileSync(evidence, JSON.stringify({
  invocation_id: process.env.AUTOMATION_INVOCATION_ID, previous_gui: previousGui, generated_nsis_sha256: digest(generatedNsi), uninstall_registration_verified: true,
  source_commit: process.env.SOURCE_SHA ?? process.env.GITHUB_SHA, installer_sha256: digest(installer), automation: first,
  synthetic_install_verified: true, same_version_reinstall_verified: true,
  // The pinned old and new packages may share an app version: this is a GUI-only to bundled-console transition, not version migration.
  product_version_increase_verified: false, preinstalled_webview_required: true,
  gui_only_upgrade_verified: Boolean(previousArg), previous_installer_sha256: previousHash ?? null,
  synthetic_sentinel_preserved: true, foreign_install_file_preserved: true, external_roots_unchanged: true, registry_path_unchanged: true, uninstall_console_removed: true,
  // No real settings DB migration or interactive GUI acceptance is inferred.
  real_configuration_migration_verified: false, native_gui_verified: false,
}, null, 2) + '\n');
