import type { AntigravityProvider } from '@/types/antigravity';
import {
  LOCAL_CONFIG_ID,
  isLocalConfigId,
  shouldLoadOfficialAccounts,
  shouldShowOfficialAccounts,
} from '../../shared/localConfig';

/**
 * Antigravity 的本地文件桥接态判定。
 *
 * id 与判定逻辑全仓共用（见 shared/localConfig.ts），这里只保留本 CLI 的
 * 命名与类型签名——调用点不用改，改动集中在共享模块一处。
 */

export const ANTIGRAVITY_LOCAL_PROVIDER_ID = LOCAL_CONFIG_ID;

export const isAntigravityLocalProviderId = isLocalConfigId;

export const shouldLoadAntigravityOfficialAccounts = shouldLoadOfficialAccounts;

export const shouldShowAntigravityOfficialAccounts = (
  provider: Pick<AntigravityProvider, 'id' | 'category'>,
  officialAccountCount: number,
): boolean => shouldShowOfficialAccounts(provider, officialAccountCount);
