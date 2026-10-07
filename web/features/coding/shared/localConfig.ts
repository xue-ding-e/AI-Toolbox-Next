/**
 * 「本地文件桥接态」的共享标识与判定。
 *
 * 每个 coding CLI 都有同一个概念：数据库里没有已应用预设时，把磁盘上那份
 * 现成配置当成一条**虚拟记录**展示，用户可以直接「收编」成预设。这条记录的
 * id 与显示名必须全仓一致——否则同一个概念在不同 CLI 上长得不一样。
 *
 * 后端对应常量在 `tauri/src/coding/local_bridge.rs`，**两边必须同值**。
 */

/** 本地文件桥接态的保留 id。 */
export const LOCAL_CONFIG_ID = '__local__';

/**
 * 这条记录是不是本地文件桥接态。
 *
 * 桥接态**不是**可管理的预设：不可 apply、不可删除、不参与拖拽排序、
 * 托盘里也不出现。任何「列出可管理项」的地方都要先把它排除。
 */
export const isLocalConfigId = (configId: string | null | undefined): boolean =>
  configId === LOCAL_CONFIG_ID;

/**
 * 是否应该去拉这个 provider 的官方账号列表。
 *
 * 桥接态没有对应的远端身份，拉取只会白跑一次。
 */
export const shouldLoadOfficialAccounts = (
  provider: { id: string },
): boolean => !isLocalConfigId(provider.id);

/**
 * 是否应该渲染「官方账号」区块。
 *
 * 除了桥接态本身，还要求这个 provider 确实是官方渠道（`category === 'official'`）
 * 或已经有账号——否则给一个纯 API-key 的自定义 provider 渲染空账号区。
 */
export const shouldShowOfficialAccounts = (
  provider: { id: string; category?: string },
  officialAccountCount: number,
): boolean =>
  shouldLoadOfficialAccounts(provider) &&
  (provider.category === 'official' || officialAccountCount > 0);
