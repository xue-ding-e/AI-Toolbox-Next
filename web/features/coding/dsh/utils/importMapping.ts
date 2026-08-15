import type { CcSwitchProviderCandidate } from '@/services/ccSwitchApi';
import type { AllApiHubProviderItem } from '@/types/allApiHub';

/** 解析 CC Switch 候选的 `env` blob(settings_config 为 `{env:{...}}` 的 JSON 字符串)。 */
const parseCcSwitchEnv = (
  candidate: CcSwitchProviderCandidate
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

/** 把候选名/route 合成为 DSH 凭据 ref(仅保留字母数字下划线,大写)。 */
export const buildDshCredentialRef = (base: string): string =>
  `${base}_API_KEY`.replace(/[^A-Za-z0-9_]/g, '_').toUpperCase();

/**
 * 把 CC Switch 的 Claude 候选提取成 DSH provider route + 凭据写入所需信息。
 * DSH 密钥单独存 `.credentials.yaml`,因此返回 `apiKey` 与合成 `credentialRef`。
 */
export const extractDshProviderFromCcSwitch = (
  candidate: CcSwitchProviderCandidate
): { provider: Record<string, unknown>; apiKey: string | undefined; credentialRef: string } | null => {
  const env = parseCcSwitchEnv(candidate);
  if (!env) return null;

  const baseUrl = env.ANTHROPIC_BASE_URL as string | undefined;
  const apiKey =
    (env.ANTHROPIC_AUTH_TOKEN as string | undefined) || (env.ANTHROPIC_API_KEY as string | undefined);

  if (!baseUrl && !apiKey) return null;

  const credentialRef = buildDshCredentialRef(candidate.name || candidate.providerId || 'provider');
  const provider: Record<string, unknown> = {
    api: 'anthropic-messages',
    models: [],
    apiKeyEnv: credentialRef,
  };
  if (baseUrl) provider.baseURL = baseUrl;
  return { provider, apiKey, credentialRef };
};

/**
 * 把 All API Hub 候选转成 DSH provider route(密钥单独经 `.credentials.yaml` 写入)。
 */
export const buildDshProviderFromAllApiHub = (
  item: AllApiHubProviderItem
): { providerKey: string; provider: Record<string, unknown>; apiKey: string | undefined; credentialRef: string } => {
  const credentialRef = buildDshCredentialRef(item.providerId || item.name);
  const config = { ...item.config };
  const apiKey = (config.api_key as string | undefined) || undefined;
  delete config.api_key;
  return {
    providerKey: item.providerId,
    provider: {
      ...config,
      apiKeyEnv: credentialRef,
    },
    apiKey,
    credentialRef,
  };
};