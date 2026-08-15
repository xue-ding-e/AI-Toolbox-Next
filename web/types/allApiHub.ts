/** 通用 All API Hub 导入主数据(与后端各模块 list/resolve 命令返回一致)。 */
export interface AllApiHubProviderItem {
  providerId: string;
  name: string;
  apiProtocol: string;
  baseUrl?: string;
  requiresBrowserOpen: boolean;
  isDisabled: boolean;
  hasApiKey: boolean;
  apiKeyPreview?: string;
  balanceUsd?: number;
  balanceCny?: number;
  siteName?: string;
  siteType?: string;
  accountLabel: string;
  sourceProfileName: string;
  sourceExtensionId: string;
  /** 已按目标模块 converter 转换后的 provider 配置(任意形态)。 */
  config: Record<string, unknown>;
}

export interface AllApiHubProvidersResult {
  found: boolean;
  profiles: { profileName: string; extensionId: string; path: string }[];
  providers: AllApiHubProviderItem[];
  message?: string;
}