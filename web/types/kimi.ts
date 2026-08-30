export type KimiProviderCategory = 'official' | 'custom' | string;

/** Temporary provider projected from the on-disk config when the DB has none. */
export const KIMI_LOCAL_PROVIDER_ID = '__local__';

export interface KimiProviderFormData {
  name: string;
  category: KimiProviderCategory;
  settingsConfig: string;
  notes?: string;
  meta?: Record<string, unknown>;
}

export interface KimiCatalogModel {
  key: string;
  model: string;
  provider: string;
  displayName?: string;
  maxContextSize?: number;
  capabilities?: string[];
  supportEfforts?: string[];
  defaultEffort?: string;
  [key: string]: unknown;
}

export interface KimiProviderConfig {
  type?: string;
  base_url?: string;
  [key: string]: unknown;
}

export interface KimiSettingsConfig {
  auth?: {
    API_KEY?: string;
    [key: string]: unknown;
  };
  defaultModelKey?: string;
  providerConfigs?: Record<string, KimiProviderConfig>;
  modelCatalog?: {
    models?: KimiCatalogModel[];
    [key: string]: unknown;
  };
  config?: string;
  [key: string]: unknown;
}

export interface KimiProvider {
  id: string;
  name: string;
  category: KimiProviderCategory;
  settingsConfig: string;
  sourceProviderId?: string;
  websiteUrl?: string;
  notes?: string;
  icon?: string;
  iconColor?: string;
  sortIndex?: number;
  meta?: Record<string, unknown>;
  isApplied?: boolean;
  isDisabled?: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface KimiProviderInput {
  id?: string;
  name: string;
  category: KimiProviderCategory;
  settingsConfig: string;
  sourceProviderId?: string;
  websiteUrl?: string;
  notes?: string;
  icon?: string;
  iconColor?: string;
  sortIndex?: number;
  meta?: Record<string, unknown>;
  isDisabled?: boolean;
}

export interface KimiCommonConfig {
  config: string;
  rootDir?: string | null;
  updatedAt?: string;
}

export interface KimiCommonConfigInput {
  config: string;
  rootDir?: string | null;
  clearRootDir?: boolean;
}

export interface KimiPathInfo {
  path: string;
  source: 'custom' | 'env' | 'shell' | 'default';
}

/** Live on-disk Kimi settings returned by `read_kimi_settings`. */
export interface KimiSettings {
  config: string | null;
}

export interface KimiOfficialAccount {
  id: string;
  providerId: string;
  name: string;
  kind: string;
  email?: string;
  subject?: string;
  tokenEndpoint?: string;
  expiresAt?: number;
  lastRefresh?: string;
  lastError?: string;
  planType?: string;
  limitWeeklyText?: string;
  limitMonthlyText?: string;
  limitWeeklyResetAt?: number;
  limitMonthlyResetAt?: number;
  lastLimitsFetchedAt?: string;
  isApplied: boolean;
  sortIndex?: number;
  createdAt: string;
  updatedAt: string;
}

export interface KimiPlugin {
  name: string;
  version?: string;
  description?: string;
  enabled?: boolean;
}

export interface KimiDeviceAuthStartResult {
  sessionId: string;
  verificationUri: string;
  verificationUriComplete?: string;
  userCode: string;
  expiresAt: number;
  pollIntervalSeconds: number;
}

export interface KimiAuthStatusEvent {
  sessionId: string;
  status: string;
  message?: string;
  accountId?: string;
}


