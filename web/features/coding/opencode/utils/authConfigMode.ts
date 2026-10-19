export interface AuthConfigModeState {
  enabled: boolean;
  loading: boolean;
  confirmed: boolean;
}

/** Shared by the page probe, migration toggle and auth editor save path. */
export function createAuthConfigMode() {
  // Start locked: a failed or unfinished probe must never imply V1. Keeping
  // enabled separately lets the settings switch retain its last displayed value.
  let state: AuthConfigModeState = { enabled: false, loading: true, confirmed: false };
  let generation = 0;
  let pendingChanges = 0;
  const listeners = new Set<() => void>();
  const update = (next: AuthConfigModeState) => {
    state = next;
    for (const listener of listeners) listener();
  };
  const readOnly = () => state.loading || !state.confirmed || state.enabled;
  return {
    getSnapshot: () => state,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    readOnly,
    async run(operation: () => Promise<boolean>, isCurrent = () => true) {
      const request = ++generation;
      const current = () => request === generation && isCurrent();
      update({ ...state, loading: true, confirmed: false });
      try {
        const enabled = await operation();
        if (!current()) return undefined;
        update({ enabled, loading: pendingChanges > 0, confirmed: pendingChanges === 0 });
        return enabled;
      } catch (error) {
        if (current()) update({ ...state, loading: pendingChanges > 0, confirmed: false });
        throw error;
      }
    },
    async change(operation: () => Promise<boolean>) {
      // A probe may finish while migration's reload/tray work is pending.
      // It must not unlock the editor or clear the migration busy indicator.
      pendingChanges++;
      generation++;
      update({ ...state, loading: true, confirmed: false });
      try {
        return await operation();
      } finally {
        pendingChanges--;
        generation++;
        // Invalidate overlapping probes. Re-probe after the entire change to
        // establish the mode actually left on disk, even when change failed.
        update({ ...state, loading: pendingChanges > 0, confirmed: false });
      }
    },
    async save(operation: () => Promise<void>) {
      // Read the live controller, not the render's potentially stale closure.
      if (readOnly()) throw new Error('auth.json can only be edited in confirmed OpenCode V1 mode');
      await operation();
    },
  };
}
