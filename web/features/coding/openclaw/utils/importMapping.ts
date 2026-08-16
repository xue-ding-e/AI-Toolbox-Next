import type { CcSwitchProviderCandidate } from '@/services/ccSwitchApi';
import type { OpenClawProviderConfig } from '@/types/openclaw';

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

/**
 * Convert a CC Switch Claude candidate into an OpenClaw provider config. OpenClaw stores
 * `apiKey`/`baseUrl` directly on the provider object, so the candidate's Anthropic env vars
 * map straight onto it. Returns null when the candidate carries neither base URL nor key.
 */
export const extractOpenClawProviderFromCcSwitch = (
  candidate: CcSwitchProviderCandidate,
): OpenClawProviderConfig | null => {
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

  const provider: OpenClawProviderConfig = {
    api: 'anthropic',
    models: modelId ? [{ id: modelId }] : [],
  };
  if (baseUrl) provider.baseUrl = baseUrl;
  if (apiKey) provider.apiKey = apiKey;
  return provider;
};
