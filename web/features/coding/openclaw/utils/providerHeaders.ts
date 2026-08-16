import type { OpenClawProviderConfig } from '@/types/openclaw';
import { OPENCLAW_DEFAULT_USER_AGENT } from '../constants';

/**
 * 按 User-Agent 开关应用/移除 provider 的 `headers["User-Agent"]`(对齐 cc-switch):
 * 开启时整体覆盖为 `{ "User-Agent": <默认值> }`;关闭时仅删除 `User-Agent` 键,
 * 同时保留 `Authorization` 等其他自定义 header(若剩余 header 为空则一并移除 `headers` 字段)。
 */
export const applyOpenClawUserAgent = (
  config: OpenClawProviderConfig,
  enabled: boolean
): OpenClawProviderConfig => {
  if (enabled) {
    return { ...config, headers: { 'User-Agent': OPENCLAW_DEFAULT_USER_AGENT } };
  }
  if (!config.headers) {
    return config;
  }
  const { 'User-Agent': _removed, ...restHeaders } = config.headers;
  if (Object.keys(restHeaders).length === 0) {
    const { headers: _dropped, ...rest } = config;
    return rest as OpenClawProviderConfig;
  }
  return { ...config, headers: restHeaders };
};

/** 该 provider 当前是否启用了 User-Agent 头。 */
export const hasOpenClawUserAgent = (config?: OpenClawProviderConfig | null): boolean =>
  Boolean(config?.headers && 'User-Agent' in config.headers);