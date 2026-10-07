import React from 'react';

import CliExtensionsSection, {
  type RecommendedCliExtension,
} from '@/features/coding/shared/cliExtensions/CliExtensionsSection';
import type { CliExtensionsService } from '@/features/coding/shared/cliExtensions/types';
import {
  installOmoNativeExtension,
  listOmoNativeExtensions,
  setOmoNativeExtensionEnabled,
  uninstallOmoNativeExtension,
  updateOmoNativeExtensions,
} from '@/services/omoNativeApi';

/**
 * 推荐扩展清单。
 *
 * **为什么列出来的全是 `pi-*` 包**（用户 2026-10-07 问过）：`omo` 装好后做了一次
 * 一次性迁移，把 `~/.pi/agent` 的配置**复制**到 `~/.omo/agent`
 * （`migrations-state.json` 的 `legacyPiAgentDir`，实测时间戳与 `install.json`
 * 的安装时间只差 2 小时）。两个目录是**独立副本**，不是软链，之后各走各的。
 * 所以这里的 20 个包是那次迁移带过来的，OmO 侧真正拥有它们。
 *
 * **推荐清单有意为空**：OmO 的包生态就是 Pi 的（`omo list` 装的是 `pi-*` 命名的包），
 * 推荐清单是 Pi 那边维护的；在这里再抄一份会变成两处要同步的名单。
 * 用户要装什么可以直接粘贴来源，或去 Pi 页看推荐。
 */
const RECOMMENDED_OMO_EXTENSIONS: RecommendedCliExtension[] = [];

/** OmO 的扩展 API 适配（共享组件要求的形状）。 */
const OMO_EXTENSIONS_SERVICE: CliExtensionsService = {
  list: listOmoNativeExtensions,
  install: (source) => installOmoNativeExtension({ source }),
  uninstall: (input) => uninstallOmoNativeExtension(input),
  update: (source) => updateOmoNativeExtensions(source ? { source } : undefined),
  setEnabled: setOmoNativeExtensionEnabled,
};

interface OmoNativeExtensionsSectionProps {
  refreshKey?: number;
  /** 扩展变更后刷新页面其余部分。 */
  onChanged?: () => void | Promise<void>;
}

/**
 * OmO Native 的「扩展管理」区块。
 *
 * 实现在共享的 `CliExtensionsSection`——`omo` 与 `pi` 共用 senpi 引擎的包体系
 * （`omo list` 的输出与 `pi list` 逐字相同，过滤语义也一致，2026-10-07 实测）。
 *
 * **不挂 magic-context 设置**：后端 `MagicContextHarness` 只有 `opencode` / `pi`
 * 两个取值，那个包的配置目录也是按这两个 harness 解析的，OmO 没有对应物。
 */
const OmoNativeExtensionsSection: React.FC<OmoNativeExtensionsSectionProps> = ({
  refreshKey,
  onChanged,
}) => (
  <CliExtensionsSection
    i18nPrefix="omoNative"
    service={OMO_EXTENSIONS_SERVICE}
    recommendedExtensions={RECOMMENDED_OMO_EXTENSIONS}
    refreshKey={refreshKey}
    onChanged={onChanged}
  />
);

export default OmoNativeExtensionsSection;
