export interface AuthConfigModeState {
  enabled: boolean;
  loading: boolean;
}

/** Shared by the page probe, migration toggle and auth editor save path. */
export function createAuthConfigMode() {
  let state: AuthConfigModeState = { enabled: false, loading: true };
  const listeners = new Set<() => void>();
  const update = (next: AuthConfigModeState) => {
    state = next;
    for (const listener of listeners) listener();
  };
  const readOnly = () => !state.loading && state.enabled;
  return {
    getSnapshot: () => state,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    readOnly,
    async run(operation: () => Promise<boolean>, isCurrent = () => true) {
      update({ ...state, loading: true });
      try {
        const enabled = await operation();
        if (!isCurrent()) return undefined;
        update({ enabled, loading: true });
        return enabled;
      } finally {
        if (isCurrent()) update({ ...state, loading: false });
      }
    },
    async save(operation: () => Promise<void>) {
      await operation();
    },
  };
}
