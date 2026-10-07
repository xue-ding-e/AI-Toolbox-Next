import type { GrokProvider } from '@/types/grok';
import {
  LOCAL_CONFIG_ID,
  isLocalConfigId,
  shouldLoadOfficialAccounts,
  shouldShowOfficialAccounts,
} from '../../shared/localConfig';

/**
 * Grok 的本地文件桥接态判定。
 *
 * id 与判定逻辑全仓共用（见 shared/localConfig.ts），这里只保留本 CLI 的
 * 命名与类型签名——调用点不用改，改动集中在共享模块一处。
 */

export const GROK_LOCAL_PROVIDER_ID = LOCAL_CONFIG_ID;

export const isGrokLocalProviderId = isLocalConfigId;

export const shouldLoadGrokOfficialAccounts = shouldLoadOfficialAccounts;

export const shouldShowGrokOfficialAccounts = (
  provider: Pick<GrokProvider, 'id' | 'category'>,
  officialAccountCount: number,
): boolean => shouldShowOfficialAccounts(provider, officialAccountCount);
