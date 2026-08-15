import type { OpenClawProviderConfig } from '@/types/openclaw';
import { OPENCLAW_DEFAULT_USER_AGENT } from '../constants';

/**
 * 按 User-Agent 开关应用/移除 provider 的 `headers["User-Agent"]`(对齐 cc-switch):
 * 开启时整体覆盖为 `{ "User-Agent": <默认值> }`,关闭时删除整个 `headers`。
 */
export const applyOpenClawUserAgent = (
  config: OpenClawProviderConfig,
  enabled: boolean
): OpenClawProviderConfig => {
  if (enabled) {
    return { ...config, headers: { 'User-Agent': OPENCLAW_DEFAULT_USER_AGENT } };
  }
  const { headers: _removed, ...rest } = config;
  return rest as OpenClawProviderConfig;
};

/** 该 provider 当前是否启用了 User-Agent 头。 */
export const hasOpenClawUserAgent = (config?: OpenClawProviderConfig | null): boolean =>
  Boolean(config?.headers && 'User-Agent' in config.headers);