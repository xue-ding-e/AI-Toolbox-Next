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
const SOURCE_PAYLOAD_KEY = '__aiToolboxSourcePayload';

export function buildFavoriteProviderStorageKey(
  source: FavoriteProviderSource,
  providerId: string,
): string {
  if (source === 'opencode') {
    return providerId;
  }

  return `${source}${SOURCE_PREFIX_SEPARATOR}${providerId}`;
}

export function extractFavoriteProviderRawId(
  source: FavoriteProviderSource,
  storageProviderId: string,
): string {
  if (source === 'opencode') {
    return storageProviderId;
  }

  const prefix = `${source}${SOURCE_PREFIX_SEPARATOR}`;
  return storageProviderId.startsWith(prefix)
    ? storageProviderId.slice(prefix.length)
    : storageProviderId;
}

export function isFavoriteProviderForSource(
  source: FavoriteProviderSource,
  favoriteProvider: OpenCodeFavoriteProvider,
): boolean {
  if (source === 'opencode') {
    return !favoriteProvider.providerId.includes(SOURCE_PREFIX_SEPARATOR);
  }

  return favoriteProvider.providerId.startsWith(`${source}${SOURCE_PREFIX_SEPARATOR}`);
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
  return favoriteProviders.find((provider) => provider.providerId === storageKey)?.diagnostics;
}
