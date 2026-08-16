import type { CcSwitchProviderCandidate } from '@/services/ccSwitchApi';

/** 解析 CC Switch 候选的 `env` blob(settings_config 为 `{env:{...}}` 的 JSON 字符串)。 */
export const parseCcSwitchEnv = (
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

/**
 * 把 CC Switch 的 Claude 供应商候选提取成 Hermes `custom_providers` 条目。
 * CC Switch 只携带 env blob(无 api_mode),因此按 Anthropic 形态默认 `api_mode: "anthropic"`。
 * 既无 base_url 也无 api_key 时返回 null(不可导入)。
 */
export const extractHermesProviderFromCcSwitch = (
  candidate: CcSwitchProviderCandidate
): Record<string, unknown> | null => {
  const env = parseCcSwitchEnv(candidate);
  if (!env) return null;

  const baseUrl = env.ANTHROPIC_BASE_URL as string | undefined;
  const apiKey =
    (env.ANTHROPIC_AUTH_TOKEN as string | undefined) || (env.ANTHROPIC_API_KEY as string | undefined);

  if (!baseUrl && !apiKey) return null;

  const provider: Record<string, unknown> = {
    api_mode: 'anthropic',
    models: [],
  };
  if (baseUrl) provider.base_url = baseUrl;
  if (apiKey) provider.api_key = apiKey;
  // 友好显示名(与身份 key 解耦):后端对 custom providers 仍把 `name` 写为 key,
  // 这里另写 `display_name`,卡片/下拉/收藏列表据此显示渠道名而非 slug。
  if (candidate.name) provider.display_name = candidate.name;
  return provider;
};