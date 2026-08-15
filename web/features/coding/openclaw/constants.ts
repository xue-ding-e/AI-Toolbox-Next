/**
 * OpenClaw 配置常量
 *
 * `tools.profile` 上游合法枚举为 minimal / coding / messaging / full;
 * 旧的 default / strict / permissive / custom 已废弃。
 */
export const OPENCLAW_TOOLS_PROFILES = ['minimal', 'coding', 'messaging', 'full'] as const;

export type OpenClawToolsProfile = (typeof OPENCLAW_TOOLS_PROFILES)[number];

export const OPENCLAW_PROFILE_OPTIONS: { value: OpenClawToolsProfile; labelKey: string }[] = [
  { value: 'minimal', labelKey: 'openclaw.tools.profileMinimal' },
  { value: 'coding', labelKey: 'openclaw.tools.profileCoding' },
  { value: 'messaging', labelKey: 'openclaw.tools.profileMessaging' },
  { value: 'full', labelKey: 'openclaw.tools.profileFull' },
];

/**
 * 发送 User-Agent 时写入 `headers["User-Agent"]` 的默认值(对齐 cc-switch)。
 * 部分供应商需要浏览器 User-Agent 才能正常访问。
 */
export const OPENCLAW_DEFAULT_USER_AGENT =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:148.0) Gecko/20100101 Firefox/148.0';