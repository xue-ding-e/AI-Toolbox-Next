import type { OpenCodeDiagnosticsConfig, OpenCodeFavoriteProvider } from '@/services/opencodeApi';
import type { OpenCodeProvider } from '@/types/opencode';

export type FavoriteProviderSource = 'opencode' | 'claudecode' | 'codex' | 'openclaw';

export interface ClaudeFavoriteProviderPayload {
  name: string;
  category: string;
  settingsConfig: string;
  notes?: string;
}

export interface CodexFavoriteProviderPayload {
  name: string;
  category: string;
  settingsConfig: string;
  notes?: string;
}

export interface OpenClawFavoriteProviderPayload {
  providerId: string;
  config: Record<string, unknown>;
}

const SOURCE_PREFIX_SEPARATOR = ':';
const STORAGE_KEY_PREFIX: Record<FavoriteProviderSource, string> = {
  opencode: 'opencode',
  claudecode: 'claudecode',
  codex: 'codex',
  openclaw: 'openclaw',
};
const SOURCE_PAYLOAD_KEY = '__aiToolboxSourcePayload';

function getStoragePrefix(source: FavoriteProviderSource): string {
  return `${STORAGE_KEY_PREFIX[source]}${SOURCE_PREFIX_SEPARATOR}`;
}

function startsWithKnownStoragePrefix(providerId: string): boolean {
  return Object.values(STORAGE_KEY_PREFIX).some((prefix) =>
    providerId.startsWith(`${prefix}${SOURCE_PREFIX_SEPARATOR}`),
  );
}

export function buildFavoriteProviderStorageKey(
  source: FavoriteProviderSource,
  providerId: string,
): string {
  return `${getStoragePrefix(source)}${providerId}`;
}

export function extractFavoriteProviderRawId(
  source: FavoriteProviderSource,
  storageProviderId: string,
): string {
  const prefix = getStoragePrefix(source);
  if (storageProviderId.startsWith(prefix)) {
    return storageProviderId.slice(prefix.length);
  }

  if (source === 'opencode') {
    return storageProviderId;
  }

  return storageProviderId;
}

export function isFavoriteProviderForSource(
  source: FavoriteProviderSource,
  favoriteProvider: OpenCodeFavoriteProvider,
): boolean {
  const providerId = favoriteProvider.providerId;

  if (source === 'opencode') {
    return providerId.startsWith(getStoragePrefix('opencode')) || !startsWithKnownStoragePrefix(providerId);
  }

  return providerId.startsWith(getStoragePrefix(source));
}

export function buildFavoriteProviderOptions(
  provider: OpenCodeProvider,
  payload: unknown,
): OpenCodeProvider {
  return {
    ...provider,
    options: {
      ...(provider.options || {}),
      [SOURCE_PAYLOAD_KEY]: payload,
    },
  };
}

export function getFavoriteProviderPayload<T>(
  favoriteProvider: OpenCodeFavoriteProvider,
): T | null {
  const payload = favoriteProvider.providerConfig.options?.[SOURCE_PAYLOAD_KEY];
  return payload && typeof payload === 'object' ? (payload as T) : null;
}

export function mergeDiagnosticsIntoFavoriteProviders(
  previousProviders: OpenCodeFavoriteProvider[],
  nextProvider: OpenCodeFavoriteProvider,
  source: FavoriteProviderSource,
): OpenCodeFavoriteProvider[] {
  if (!isFavoriteProviderForSource(source, nextProvider)) {
    return previousProviders;
  }

  const targetStorageKey = nextProvider.providerId;
  const existingIndex = previousProviders.findIndex(
    (provider) => provider.providerId === targetStorageKey,
  );

  if (existingIndex >= 0) {
    const nextProviders = [...previousProviders];
    nextProviders[existingIndex] = nextProvider;
    return nextProviders;
  }

  return [...previousProviders, nextProvider];
}

export function dedupeFavoriteProvidersByPayload(
  favoriteProviders: OpenCodeFavoriteProvider[],
  currentStorageKeys: Set<string>,
): {
  keptProviders: OpenCodeFavoriteProvider[];
  duplicateIds: string[];
} {
  const providerBySignature = new Map<string, OpenCodeFavoriteProvider>();
  const duplicateIds: string[] = [];

  for (const favoriteProvider of favoriteProviders) {
    const payload = getFavoriteProviderPayload<Record<string, unknown>>(favoriteProvider);
    const signature = payload ? JSON.stringify(payload) : favoriteProvider.providerId;
    const existingProvider = providerBySignature.get(signature);

    if (!existingProvider) {
      providerBySignature.set(signature, favoriteProvider);
      continue;
    }

    const existingIsCurrent = currentStorageKeys.has(existingProvider.providerId);
    const nextIsCurrent = currentStorageKeys.has(favoriteProvider.providerId);
    const shouldReplaceExisting =
      (!existingIsCurrent && nextIsCurrent) ||
      (existingIsCurrent === nextIsCurrent &&
        favoriteProvider.updatedAt > existingProvider.updatedAt);

    if (shouldReplaceExisting) {
      duplicateIds.push(existingProvider.providerId);
      providerBySignature.set(signature, favoriteProvider);
    } else {
      duplicateIds.push(favoriteProvider.providerId);
    }
  }

  return {
    keptProviders: Array.from(providerBySignature.values()),
    duplicateIds,
  };
}

export function findDiagnosticsForProvider(
  favoriteProviders: OpenCodeFavoriteProvider[],
  source: FavoriteProviderSource,
  providerId: string,
): OpenCodeDiagnosticsConfig | undefined {
  const storageKey = buildFavoriteProviderStorageKey(source, providerId);
  return favoriteProviders.find((provider) => {
    if (provider.providerId === storageKey) {
      return true;
    }

    return source === 'opencode' && extractFavoriteProviderRawId('opencode', provider.providerId) === providerId;
  })?.diagnostics;
}
