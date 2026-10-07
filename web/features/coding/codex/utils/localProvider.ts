import type { CodexProvider } from '@/types/codex';
import {
  LOCAL_CONFIG_ID,
  isLocalConfigId,
  shouldLoadOfficialAccounts,
  shouldShowOfficialAccounts,
} from '../../shared/localConfig';

/**
 * Codex 的本地文件桥接态判定。
 *
 * id 与判定逻辑全仓共用（见 shared/localConfig.ts），这里只保留本 CLI 的
 * 命名与类型签名——调用点不用改，改动集中在共享模块一处。
 */

export const CODEX_LOCAL_PROVIDER_ID = LOCAL_CONFIG_ID;

export const isCodexLocalProviderId = isLocalConfigId;

export const shouldLoadCodexOfficialAccounts = shouldLoadOfficialAccounts;

export const shouldShowCodexOfficialAccounts = (
  provider: Pick<CodexProvider, 'id' | 'category'>,
  officialAccountCount: number,
): boolean => shouldShowOfficialAccounts(provider, officialAccountCount);
