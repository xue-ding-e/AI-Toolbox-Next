import assert from 'node:assert/strict';
import test from 'node:test';
import { createStore } from 'zustand/vanilla';

import { defaultSettings, type AppSettings } from '../../services/settingsApi.ts';
import { createKeepAwakeSettingsSlice, type KeepAwakeSettingsState } from '../../stores/keepAwakeSettings.ts';

function createHarness() {
  const backend = {
    settings: structuredClone({ ...defaultSettings, theme: 'dark' }),
    enabled: false,
    savedStates: [] as boolean[],
    appliedStates: [] as boolean[],
  };
  const dependencies = {
    getSettings: async () => structuredClone(backend.settings),
    saveSettings: async (settings: AppSettings) => {
      backend.settings = structuredClone(settings);
      backend.savedStates.push(settings.keep_computer_awake);
    },
    setKeepAwake: async (enabled: boolean) => {
      backend.enabled = enabled;
      backend.appliedStates.push(enabled);
    },
  };
  const store = createStore<KeepAwakeSettingsState>()((set) =>
    createKeepAwakeSettingsSlice(set, dependencies));
  return { store, backend, dependencies };
}

test('keep-awake toggles round-trip the saved preference and apply the same system state', async () => {
  const { store, backend, dependencies } = createHarness();
  for (const enabled of [true, false]) {
    await store.getState().setKeepComputerAwake(enabled);
    assert.equal(store.getState().keepComputerAwake, enabled);
    assert.equal((await dependencies.getSettings()).keep_computer_awake, enabled);
    assert.equal(backend.enabled, enabled);
    assert.equal(backend.settings.theme, 'dark');
    assert.equal(store.getState().keepComputerAwakeUpdating, false);
    assert.equal(store.getState().keepComputerAwakeError, null);
  }
});

test('rapid enable then disable stays ordered while the first save waits for tray refresh', async () => {
  const { store, backend, dependencies } = createHarness();
  const saveSettings = dependencies.saveSettings;
  let finishTrayRefresh!: () => void;
  const trayRefresh = new Promise<void>((resolve) => { finishTrayRefresh = resolve; });
  dependencies.saveSettings = async (settings) => {
    await saveSettings(settings);
    // A second save would return immediately, as the backend coalesces refreshes.
    if (settings.keep_computer_awake) await trayRefresh;
  };

  const enable = store.getState().setKeepComputerAwake(true);
  const disable = store.getState().setKeepComputerAwake(false);
  await new Promise<void>((resolve) => setImmediate(resolve));
  const savedWhileWaiting = [...backend.savedStates];
  const updatingWhileWaiting = store.getState().keepComputerAwakeUpdating;

  finishTrayRefresh();
  await Promise.all([enable, disable]);
  assert.deepEqual(savedWhileWaiting, [true]);
  assert.equal(updatingWhileWaiting, true);
  assert.deepEqual(backend.appliedStates, [true, false]);
  assert.equal(backend.settings.keep_computer_awake, false);
  assert.equal(backend.enabled, false);
  assert.equal(store.getState().keepComputerAwake, false);
  assert.equal(store.getState().keepComputerAwakeUpdating, false);
});

test('OS failure preserves the saved preference, exposes an error, and permits retry', async () => {
  const { store, backend, dependencies } = createHarness();
  const applySystemState = dependencies.setKeepAwake;
  dependencies.setKeepAwake = async () => { throw new Error('OS rejected the request'); };

  await assert.rejects(store.getState().setKeepComputerAwake(true), /OS rejected/);
  assert.equal(store.getState().keepComputerAwake, true);
  assert.equal(backend.settings.keep_computer_awake, true);
  assert.equal(backend.enabled, false);
  assert.equal(store.getState().keepComputerAwakeError, 'OS rejected the request');
  assert.equal(store.getState().keepComputerAwakeUpdating, false);

  dependencies.setKeepAwake = applySystemState;
  await store.getState().setKeepComputerAwake(true);
  assert.equal(backend.enabled, true);
  assert.equal(store.getState().keepComputerAwakeError, null);
});

test('save failure keeps the previous preference and never applies a system change', async () => {
  const { store, backend, dependencies } = createHarness();
  dependencies.saveSettings = async () => { throw new Error('database write failed'); };

  await assert.rejects(store.getState().setKeepComputerAwake(true), /database write failed/);
  assert.equal(store.getState().keepComputerAwake, false);
  assert.equal(backend.settings.keep_computer_awake, false);
  assert.deepEqual(backend.appliedStates, []);
  assert.equal(store.getState().keepComputerAwakeError, 'database write failed');
  assert.equal(store.getState().keepComputerAwakeUpdating, false);
});

test('a failed enable does not prevent an already queued disable from completing', async () => {
  const { store, backend, dependencies } = createHarness();
  const applySystemState = dependencies.setKeepAwake;
  dependencies.setKeepAwake = async (enabled) => {
    if (enabled) throw new Error('enable failed');
    await applySystemState(enabled);
  };

  const results = await Promise.allSettled([
    store.getState().setKeepComputerAwake(true),
    store.getState().setKeepComputerAwake(false),
  ]);
  assert.deepEqual(results.map((result) => result.status), ['rejected', 'fulfilled']);
  assert.equal(store.getState().keepComputerAwake, false);
  assert.equal(backend.settings.keep_computer_awake, false);
  assert.equal(backend.enabled, false);
  assert.equal(store.getState().keepComputerAwakeError, null);
  assert.equal(store.getState().keepComputerAwakeUpdating, false);
});
