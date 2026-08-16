import type { CcSwitchProviderCandidate } from '@/services/ccSwitchApi';

/** Parse the CC Switch Claude candidate's `env` blob (settings_config is `{env:{...}}` JSON). */
const parseCcSwitchEnv = (
  candidate: CcSwitchProviderCandidate,
): Record<string, unknown> | null => {
  try {
    const settings =
      typeof candidate.settingsConfig === 'string'
        ? JSON.parse(candidate.settingsConfig)
        : candidate.settingsConfig;
    return settings?.env && typeof settings.env === 'object' ? settings.env : {};
  } catch {
    return null;
  }
};

/** Pull the first non-empty value from the candidate env for the given keys. */
const firstEnvValue = (env: Record<string, unknown>, keys: string[]): string | undefined => {
  for (const key of keys) {
    const value = env[key];
    if (typeof value === 'string' && value.trim()) {
      return value.trim();
    }
  }
  return undefined;
};

export interface PiCcSwitchImport {
  providerKey: string;
  modelsProvider: Record<string, unknown>;
  displayName?: string;
}

/**
 * Convert a CC Switch Claude candidate into a Pi provider import record. Pi stores the API key
 * inline on the provider config (`apiKey` field), so the candidate's Anthropic env vars map
 * straight onto the modelsProvider. Returns null when the candidate carries neither base URL
 * nor key.
 */
export const extractPiProviderFromCcSwitch = (
  candidate: CcSwitchProviderCandidate,
): PiCcSwitchImport | null => {
  const env = parseCcSwitchEnv(candidate);
  if (!env) return null;

  const baseUrl = firstEnvValue(env, ['ANTHROPIC_BASE_URL']);
  const apiKey = firstEnvValue(env, ['ANTHROPIC_AUTH_TOKEN', 'ANTHROPIC_API_KEY']);
  const modelId = firstEnvValue(env, [
    'ANTHROPIC_MODEL',
    'ANTHROPIC_DEFAULT_SONNET_MODEL',
    'ANTHROPIC_DEFAULT_OPUS_MODEL',
    'ANTHROPIC_DEFAULT_HAIKU_MODEL',
  ]);

  if (!baseUrl && !apiKey) return null;

  const modelsProvider: Record<string, unknown> = {
    api: 'anthropic-messages',
    models: modelId ? [{ id: modelId }] : [],
  };
  if (baseUrl) modelsProvider.baseUrl = baseUrl;
  if (apiKey) modelsProvider.apiKey = apiKey;

  return {
    providerKey: candidate.providerId,
    modelsProvider,
    displayName: candidate.name,
  };
};
