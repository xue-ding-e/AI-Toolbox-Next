import type { OmoNativeProvider } from '@/types/omoNative';
import type { OpenCodeProvider } from '@/types/opencode';
import {
  buildFavoriteProviderOptions,
  type OmoNativeFavoriteProviderPayload,
} from '@/features/coding/shared/favoriteProviders';
import { asStringRecord, getOmoNativeModelEntries, getStringField, omoNativeApiToNpm } from './omoNativeProviders';

/**
 * Builds the favorites-store record for one OmO Native provider.
 *
 * Two halves, deliberately: the OpenCode-shaped outer object drives the shared
 * import list (grouping by SDK family, endpoint and model count), while the
 * payload carries the `models.json` entry verbatim. Only the payload is
 * replayed on import — `models.json` has no published schema, so anything
 * reconstructed from the outer fields would drop whatever keys the engine
 * wrote beyond the ones this app knows about.
 */
export function buildOmoNativeFavoriteProviderConfig(provider: OmoNativeProvider) {
  const config = provider.config;
  const baseUrl = getStringField(config, 'baseUrl').trim();
  const modelIds = getOmoNativeModelEntries(config).map((entry) => entry.id);
  const headers = asStringRecord(config.headers);

  return buildFavoriteProviderOptions(
    {
      npm: omoNativeApiToNpm(getStringField(config, 'api')),
      name: getStringField(config, 'name') || provider.key,
      options: {
        ...(baseUrl ? { baseURL: baseUrl } : {}),
        ...(Object.keys(headers).length > 0 ? { headers } : {}),
      },
      models: Object.fromEntries(modelIds.map((modelId) => [modelId, {}])),
    } satisfies OpenCodeProvider,
    {
      name: getStringField(config, 'name') || provider.key,
      config,
    } satisfies OmoNativeFavoriteProviderPayload,
  );
}
