import React from 'react';
import {
  getProviderListState,
  recordProviderLastUsed,
  saveProviderSortMode,
  type ProviderListState,
} from '@/services/providerListApi';
import {
  isProviderSortMode,
  type ProviderSortMode,
} from './sortProviders';

/**
 * Module-level cache so the coding tabs share one `get_provider_list_state`
 * round trip (pages are KeepAlive-mounted and hydrate independently).
 * noteProviderUsed/setSortMode also mutate it in place so later-mounting
 * pages see fresh values without a refetch.
 */
let cachedState: ProviderListState = { sort_modes: {}, last_used: {} };
let statePromise: Promise<ProviderListState> | null = null;

const loadProviderListState = (): Promise<ProviderListState> => {
  if (!statePromise) {
    statePromise = getProviderListState()
      .then((state) => {
        cachedState = {
          sort_modes: state.sort_modes ?? {},
          last_used: state.last_used ?? {},
        };
        return cachedState;
      })
      .catch(() => ({ sort_modes: {}, last_used: {} }));
  }
  return statePromise;
};

interface ProviderListSortState {
  /** Current sort mode; 'custom' (drag order) until hydration completes. */
  sortMode: ProviderSortMode;
  setSortMode: (mode: ProviderSortMode) => void;
  /** Last-used timestamp lookup for this module's providers. */
  lastUsedAt: (providerId: string) => string | undefined;
  /**
   * Mark a provider as just used: refreshes the local cache immediately and
   * persists through the backend command. DB-backed tabs call this after a
   * successful apply (the backend apply flow already persisted its own
   * marker; this keeps the in-memory cache in sync without a refetch).
   */
  noteProviderUsed: (providerId: string) => void;
}

export function useProviderListSort(moduleKey: string): ProviderListSortState {
  const [sortMode, setSortModeState] = React.useState<ProviderSortMode>('custom');
  const [lastUsed, setLastUsed] = React.useState<Record<string, string>>({});

  React.useEffect(() => {
    let cancelled = false;
    loadProviderListState().then((state) => {
      if (cancelled) {
        return;
      }
      const saved = state.sort_modes[moduleKey];
      setSortModeState(isProviderSortMode(saved) ? saved : 'custom');
      setLastUsed({ ...state.last_used });
    });
    return () => {
      cancelled = true;
    };
  }, [moduleKey]);

  const setSortMode = React.useCallback(
    (mode: ProviderSortMode) => {
      setSortModeState(mode);
      cachedState.sort_modes[moduleKey] = mode;
      void saveProviderSortMode(moduleKey, mode).catch(() => {});
    },
    [moduleKey],
  );

  const lastUsedAt = React.useCallback(
    (providerId: string) => lastUsed[`${moduleKey}:${providerId}`],
    [moduleKey, lastUsed],
  );

  const noteProviderUsed = React.useCallback(
    (providerId: string) => {
      const key = `${moduleKey}:${providerId}`;
      const now = new Date().toISOString();
      cachedState.last_used[key] = now;
      setLastUsed((prev) => ({ ...prev, [key]: now }));
      void recordProviderLastUsed(moduleKey, providerId).catch(() => {});
    },
    [moduleKey],
  );

  return { sortMode, setSortMode, lastUsedAt, noteProviderUsed };
}
