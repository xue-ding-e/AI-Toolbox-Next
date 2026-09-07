import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const readSource = (relativePath: string) =>
  readFile(new URL(relativePath, import.meta.url), 'utf8');

test('frontend readiness is emitted before normal and recovery app branches', async () => {
  const app = await readSource('../../app/App.tsx');
  const providers = await readSource('../../app/providers.tsx');
  const readyPosition = app.indexOf("emit('frontend-ready')");
  assert.ok(readyPosition >= 0);
  assert.ok(readyPosition < app.indexOf("if (state.status === 'loading')"));
  assert.ok(readyPosition < app.indexOf('<RecoveryApp'));
  assert.ok(!providers.includes("emit('frontend-ready')"));
});

test('backend startup and close handlers respect intentionally absent runtime state', async () => {
  const backend = await readSource('../../../tauri/src/lib.rs');
  const watchdog = backend.slice(
    backend.indexOf('fn start_linux_wayland_webview_auto_downgrade_watchdog'),
    backend.indexOf('fn setup_linux_wayland_webview_workaround'),
  );
  assert.match(watchdog, /if lightweight::is_lightweight_mode\(\)\s*\{[\s\S]*?return;/);
  const closeHandler = backend.slice(backend.indexOf('.on_window_event('));
  assert.match(
    closeHandler,
    /if app_handle.try_state::<SqliteDbState>\(\).is_none\(\)\s*\{\s*startup_recovery::exit_app\(app_handle\);\s*return;/,
  );
});
