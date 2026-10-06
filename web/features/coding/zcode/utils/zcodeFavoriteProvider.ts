import type { ZcodeProvider } from '@/types/zcode';
import {
  buildFavoriteProviderOptions,
  type ZcodeFavoriteProviderPayload,
} from '@/features/coding/shared/favoriteProviders';
import { parseZcodeProviderSettings } from './zcodeSettingsConfig';

/**
 * npm SDK family for each ZCode API format.
 *
 * The favorites store groups providers by the AI SDK package they speak, so the
 * shared import list can show a ZCode provider next to an equivalent one from
 * another CLI. An unknown format falls back to the openai-compatible family,
 * which is what an unspecified OpenAI-shaped endpoint most likely is.
 */
const ZCODE_FAVORITE_NPM_BY_API_TYPE: Record<string, string> = {
  'anthropic-messages': '@ai-sdk/anthropic',
  'openai-chat-completions': '@ai-sdk/openai',
  'openai-responses': '@ai-sdk/openai',
};

/**
 * Builds the favorites-store record for one ZCode provider.
 *
 * Two halves, deliberately: the OpenCode-shaped outer object drives the shared
 * import list (grouping, credentials preview, model count), while the payload
 * carries ZCode's own `settingsConfig` verbatim. Only the payload is replayed on
 * import — ZCode's config is structured (access / api / models), so anything
 * reconstructed from the outer fields would lose the parts it cannot express.
 */
export function buildZcodeFavoriteProviderConfig(provider: ZcodeProvider) {
  const settings = parseZcodeProviderSettings(provider.settingsConfig);
  const baseUrl = settings?.config?.api?.baseUrl?.trim();
  const apiKey = settings?.config?.access?.apiKey?.trim();
  const apiType = settings?.config?.api?.type ?? '';
  const modelIds = (settings?.models ?? [])
    .map((model) => model.modelId.trim())
    .filter(Boolean);

  return buildFavoriteProviderOptions(
    {
      npm: ZCODE_FAVORITE_NPM_BY_API_TYPE[apiType] ?? '@ai-sdk/openai-compatible',
      name: provider.name,
      options: {
        ...(baseUrl ? { baseURL: baseUrl } : {}),
        ...(apiKey ? { apiKey } : {}),
      },
      models: Object.fromEntries(modelIds.map((modelId) => [modelId, {}])),
    },
    {
      name: provider.name,
      category: provider.category,
      settingsConfig: provider.settingsConfig,
      ...(provider.notes ? { notes: provider.notes } : {}),
    } satisfies ZcodeFavoriteProviderPayload,
  );
}