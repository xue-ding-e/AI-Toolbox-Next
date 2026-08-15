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