import type { OpenClawHealthWarning } from '@/types/openclaw';
import { OPENCLAW_TOOLS_PROFILES } from './constants';

/** warning code → i18n key(兜底返回后端原始 message) */
export const WARNING_CODE_KEYS: Record<string, string> = {
  config_parse_failed: 'openclaw.healthBanner.warning.configParseFailed',
  invalid_tools_profile: 'openclaw.healthBanner.warning.invalidToolsProfile',
  legacy_agents_timeout: 'openclaw.healthBanner.warning.legacyTimeout',
  stringified_env_vars: 'openclaw.healthBanner.warning.stringifiedEnvVars',
  stringified_env_shell_env: 'openclaw.healthBanner.warning.stringifiedEnvShellEnv',
};

/** Map a health warning to a localized label; falls back to the backend message. */
export const buildHealthBannerItem = (
  warning: OpenClawHealthWarning,
  t: (key: string) => string
): string => {
  const key = WARNING_CODE_KEYS[warning.code];
  if (!key) return warning.message;
  if (warning.code === 'invalid_tools_profile') {
    return `${t(key)} (${OPENCLAW_TOOLS_PROFILES.join(' / ')})`;
  }
  return t(key);
};