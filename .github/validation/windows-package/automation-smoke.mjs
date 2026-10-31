// Runs only an explicitly supplied installed console; never launches the GUI.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, existsSync } from 'node:fs';
import { isAbsolute } from 'node:path';

// Bound captured text by UTF-8 bytes without splitting a code point.
export function boundedUtf8(value, limit = 4096) {
  let result = '', bytes = 0;
  for (const point of value) {
    const size = Buffer.byteLength(point, 'utf8');
    if (bytes + size > limit) break;
    result += point; bytes += size;
  }
  return result;
}
export function digest(path) { return createHash('sha256').update(readFileSync(path)).digest('hex'); }
export function run(file, args, env, input = '', timeout = 10000, windowsVerbatimArguments = false, failureDiagnostics = false) {
  return new Promise((resolve, reject) => {
    const child = spawn(file, args, { env, windowsHide: true, windowsVerbatimArguments, stdio: ['pipe', 'pipe', 'pipe'] });
    let out = '', err = '', failure;
    // Opt-in CI synthetic fixture diagnostics only; never capture environment.
    const diagnosticError = (error, code = null, signal = null) => {
      if (failureDiagnostics) error.processEvidence = {
        executable: file, argv: args, timeout_ms: timeout, exit_code: code, signal,
        stdout: boundedUtf8(out), stderr: boundedUtf8(err),
      };
      return error;
    };
    const stop = (reason) => {
      if (failure) return;
      failure = reason;
      // Stop this directly-created child only. Do not target a numeric PID with
      // taskkill: PID reuse could escape ownership. The mandatory outer Job owns
      // descendant termination/accounting; this adapter never claims tree cleanup.
      child.kill();
      reaper = setTimeout(() => {
        cleanup();
        child.stdin.destroy(); child.stdout.destroy(); child.stderr.destroy();
        child.unref();
        reject(diagnosticError(new Error('Child did not reap after termination; descendant cleanup is unverified', { cause: failure })));
      }, 5000);
    };
    const interrupt = () => stop(new Error('Child interrupted by SIGINT'));
    const terminate = () => stop(new Error('Child interrupted by SIGTERM'));
    process.once('SIGINT', interrupt);
    process.once('SIGTERM', terminate);
    const cleanup = () => { clearTimeout(timer); clearTimeout(reaper); process.removeListener('SIGINT', interrupt); process.removeListener('SIGTERM', terminate); };
    let reaper;
    const timer = setTimeout(() => stop(new Error('Child exceeded deadline')), timeout);
    const append = (kind, data) => {
      if (kind === 'out') out += data; else err += data;
      if (out.length + err.length > 1024 * 1024) stop(new Error('Child output exceeded limit'));
    };
    child.stdout.on('data', data => append('out', data));
    child.stderr.on('data', data => append('err', data));
    child.stdin.on('error', error => { if (error.code !== 'EPIPE') stop(error); });
    child.on('error', error => {
      if (child.pid) stop(error);
      else { cleanup(); child.stdin.destroy(); child.stdout.destroy(); child.stderr.destroy(); reject(diagnosticError(error)); }
    });
    child.on('close', (code, signal) => {
      cleanup();
      if (failure) reject(diagnosticError(failure, code, signal)); else resolve({ code, signal, out, err });
    });
    child.stdin.end(input);
  });
}
export function validateResponses(cli, mcp) {
  assert.equal(cli.ok, true);
  assert.deepEqual(mcp.map(x => x.id), [1, 2, 3, 4, 5]);
  assert.equal(mcp[0].result.protocolVersion, '2025-11-25');
  assert.deepEqual(mcp[1].result.tools.map(x => x.name), ['backup_normalize_path', 'gateway_supported_cli_keys']);
  assert.equal(mcp[2].result.isError, false);
  assert.deepEqual(mcp[2].result.structuredContent, cli.data);
  assert.equal(mcp[3].result.isError, false);
  assert.equal(mcp[3].result.structuredContent.path, '~/missing/中文');
  assert.equal(mcp[4].error.code, -32602);
}
export async function smoke(binary, sourceBinary, env) {
  assert(isAbsolute(binary) && isAbsolute(sourceBinary));
  assert.equal(digest(binary), digest(sourceBinary), 'NSIS-installed console differs from built input');
  const call = async (args, input = '') => {
    const result = await run(binary, args, env, input);
    assert.equal(result.code, 0); assert.equal(result.signal, null); assert.equal(result.err, '');
    return result.out.trim().split(/\r?\n/).map(x => JSON.parse(x));
  };
  const [cli] = await call(['--format', 'json', 'gateway', 'supported-cli-keys']);
  const [path] = await call(['--format', 'json', 'backup', 'normalize-path', '--path', '$HOME/missing/中文']);
  assert.equal(path.ok, true); assert.equal(path.data.path, '~/missing/中文');
  const frames = [
    { id: 1, method: 'initialize', params: { protocolVersion: '2025-11-25', capabilities: {}, clientInfo: { name: 'installed-fixture', version: '1' } } },
    { method: 'notifications/initialized' },
    { id: 2, method: 'tools/list' },
    { id: 3, method: 'tools/call', params: { name: 'gateway_supported_cli_keys', arguments: {} } },
    { id: 4, method: 'tools/call', params: { name: 'backup_normalize_path', arguments: { path: '$HOME/missing/中文' } } },
    { id: 5, method: 'tools/call', params: { name: 'gateway_supported_cli_keys', arguments: { unexpected: true } } },
  ];
  const mcp = await call(['mcp', 'serve'], frames.map(x => JSON.stringify({ jsonrpc: '2.0', ...x }) + '\n').join(''));
  validateResponses(cli, mcp);
  const invalid = await run(binary, ['--format', 'json', 'not-a-command'], env);
  assert.notEqual(invalid.code, 0); assert.equal(invalid.signal, null);
  assert.equal(JSON.parse(invalid.out).ok, false);
  const version = await call(['--format', 'json', '--version']);
  // These roots must stay absent: discovery must not bootstrap application state.
  for (const key of ['AI_TOOLBOX_CACHE_DIR', 'CODEX_HOME']) assert.equal(existsSync(env[key]), false, key);
  return { sha256: digest(binary), version: version[0], cli_mcp_installed_calls_verified: true };
}
