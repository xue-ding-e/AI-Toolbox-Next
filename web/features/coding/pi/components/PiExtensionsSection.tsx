import React from 'react';

import CliExtensionsSection, {
  type RecommendedCliExtension,
} from '@/features/coding/shared/cliExtensions/CliExtensionsSection';
import type { CliExtensionsService } from '@/features/coding/shared/cliExtensions/types';
import { MagicContextSettings } from '@/features/coding/shared/magicContext';
import {
  installPiExtension,
  listPiExtensions,
  setPiExtensionEnabled,
  uninstallPiExtension,
  updatePiExtensions,
} from '@/services/piApi';

const PI_PACKAGES_URL = 'https://pi.dev/packages';

/**
 * 推荐扩展清单。
 *
 * 描述文案在 `pi.extensions.recommended.*`——共享组件的 `descriptionKeySuffix`
 * 拼上本页的 i18n 前缀即是完整键。
 */
const RECOMMENDED_PI_EXTENSIONS: RecommendedCliExtension[] = [
  {
    name: 'pi-cometix-footer',
    installSource: 'npm:pi-cometix-footer',
    descriptionKeySuffix: 'recommended.cometixFooter',
    detailUrl: 'https://pi.dev/packages/pi-cometix-footer?name=pi-cometix-footer',
  },
  {
    name: 'pi-hashline-edit-pro',
    installSource: 'npm:pi-hashline-edit-pro',
    descriptionKeySuffix: 'recommended.hashlineEditPro',
    detailUrl: 'https://pi.dev/packages/pi-hashline-edit-pro?name=pi-hashline-edit-pro',
  },
  {
    name: 'pi-slopchop',
    installSource: 'npm:pi-slopchop',
    descriptionKeySuffix: 'recommended.slopchop',
    detailUrl: 'https://pi.dev/packages/pi-slopchop?name=pi-slopchop',
  },
  {
    name: '@narumitw/pi-goal',
    installSource: 'npm:@narumitw/pi-goal',
    descriptionKeySuffix: 'recommended.goal',
    detailUrl: 'https://pi.dev/packages/@narumitw/pi-goal?name=%40narumitw%2Fpi-goal',
  },
  {
    name: '@narumitw/pi-plan-mode',
    installSource: 'npm:@narumitw/pi-plan-mode',
    descriptionKeySuffix: 'recommended.planMode',
    detailUrl:
      'https://pi.dev/packages/@narumitw/pi-plan-mode?name=%40narumitw%2Fpi-plan-mode',
  },
  {
    name: '@narumitw/pi-subagents',
    installSource: 'npm:@narumitw/pi-subagents',
    descriptionKeySuffix: 'recommended.subagents',
    detailUrl:
      'https://pi.dev/packages/@narumitw/pi-subagents?name=%40narumitw%2Fpi-subagents',
  },
  {
    name: 'pi-autoresearch',
    installSource: 'npm:pi-autoresearch',
    descriptionKeySuffix: 'recommended.autoresearch',
    detailUrl: 'https://pi.dev/packages/pi-autoresearch?name=pi-autoresearch',
  },
  {
    name: '@juicesharp/rpiv-ask-user-question',
    installSource: 'npm:@juicesharp/rpiv-ask-user-question',
    descriptionKeySuffix: 'recommended.askUserQuestion',
    detailUrl:
      'https://pi.dev/packages/@juicesharp/rpiv-ask-user-question?name=%40juicesharp%2Frpiv-ask-user-question',
  },
  {
    name: '@juicesharp/rpiv-todo',
    installSource: 'npm:@juicesharp/rpiv-todo',
    descriptionKeySuffix: 'recommended.todo',
    detailUrl: 'https://pi.dev/packages/@juicesharp/rpiv-todo?name=%40juicesharp%2Frpiv-todo',
  },
  {
    name: '@narumitw/pi-btw',
    installSource: 'npm:@narumitw/pi-btw',
    descriptionKeySuffix: 'recommended.btw',
    detailUrl: 'https://pi.dev/packages/@narumitw/pi-btw?name=%40narumitw%2Fpi-btw',
  },
  {
    name: 'pi-mcp-adapter',
    installSource: 'npm:pi-mcp-adapter',
    descriptionKeySuffix: 'recommended.mcpAdapter',
    detailUrl: 'https://pi.dev/packages/pi-mcp-adapter?name=pi-mcp-adapter',
  },
  {
    name: '@ff-labs/pi-fff',
    installSource: 'npm:@ff-labs/pi-fff',
    descriptionKeySuffix: 'recommended.fff',
    detailUrl: 'https://pi.dev/packages/@ff-labs/pi-fff?name=%40ff-labs%2Fpi-fff',
  },
  {
    name: 'pi-rtk-optimizer',
    installSource: 'npm:pi-rtk-optimizer',
    descriptionKeySuffix: 'recommended.rtkOptimizer',
    detailUrl: 'https://pi.dev/packages/pi-rtk-optimizer?name=pi-rtk-optimizer',
  },
  {
    name: 'pi-cache-optimizer',
    installSource: 'npm:pi-cache-optimizer',
    descriptionKeySuffix: 'recommended.cacheOptimizer',
    detailUrl: 'https://pi.dev/packages/pi-cache-optimizer?name=pi-cache-optimizer',
  },
  {
    name: '@narumitw/pi-lsp',
    installSource: 'npm:@narumitw/pi-lsp',
    descriptionKeySuffix: 'recommended.lsp',
    detailUrl: 'https://pi.dev/packages/@narumitw/pi-lsp?name=%40narumitw%2Fpi-lsp',
  },
  {
    name: 'pi-agent-browser-native',
    installSource: 'npm:pi-agent-browser-native',
    descriptionKeySuffix: 'recommended.agentBrowserNative',
    detailUrl:
      'https://pi.dev/packages/pi-agent-browser-native?name=pi-agent-browser-native',
  },
  {
    name: 'pi-add-dir',
    installSource: 'npm:pi-add-dir',
    descriptionKeySuffix: 'recommended.addDir',
    detailUrl: 'https://pi.dev/packages/pi-add-dir?name=pi-add-dir',
  },
  {
    name: 'pi-workspace-history',
    installSource: 'npm:pi-workspace-history',
    descriptionKeySuffix: 'recommended.workspaceHistory',
    detailUrl: 'https://pi.dev/packages/pi-workspace-history?name=pi-workspace-history',
  },
  {
    name: '@narumitw/pi-caffeinate',
    installSource: 'npm:@narumitw/pi-caffeinate',
    descriptionKeySuffix: 'recommended.caffeinate',
    detailUrl:
      'https://pi.dev/packages/@narumitw/pi-caffeinate?name=%40narumitw%2Fpi-caffeinate',
  },
  {
    name: '@tmustier/pi-raw-paste',
    installSource: 'npm:@tmustier/pi-raw-paste',
    descriptionKeySuffix: 'recommended.rawPaste',
    detailUrl:
      'https://pi.dev/packages/@tmustier/pi-raw-paste?name=%40tmustier%2Fpi-raw-paste',
  },
  {
    name: '@victor-software-house/pi-curated-themes',
    installSource: 'npm:@victor-software-house/pi-curated-themes',
    descriptionKeySuffix: 'recommended.curatedThemes',
    detailUrl:
      'https://pi.dev/packages/@victor-software-house/pi-curated-themes?name=%40victor-software-house%2Fpi-curated-themes',
  },
  {
    name: '@cortexkit/pi-magic-context',
    installSource: 'npm:@cortexkit/pi-magic-context',
    descriptionKeySuffix: 'recommended.magicContext',
    detailUrl: 'https://github.com/cortexkit/magic-context',
  },
];

const MAGIC_CONTEXT_INSTALL_SOURCE = 'npm:@cortexkit/pi-magic-context';

const isRecommendedInstalled = (
  extensions: { source: string }[],
  installSource: string,
): boolean => {
  const normalizedInstallSource = installSource.trim().toLowerCase();
  const normalizedPackageName = normalizedInstallSource.startsWith('npm:')
    ? normalizedInstallSource.slice(4)
    : normalizedInstallSource;
  return extensions.some((extension) => {
    const normalizedSource = extension.source.trim().toLowerCase();
    return (
      normalizedSource === normalizedInstallSource ||
      normalizedSource === normalizedPackageName
    );
  });
};

/**
 * Pi 的扩展 API 适配（共享组件要求的形状）。
 *
 * `list` 要把可选字段补齐：`web/types/pi.ts` 里这几个是 `?`（历史写法），
 * 而后端契约里它们总是有值（Rust 侧带 `serde(default)`，序列化时必出）。
 * 补一次比在共享组件里到处写 `?? false` 干净。
 */
const PI_EXTENSIONS_SERVICE: CliExtensionsService = {
  list: async () => {
    const result = await listPiExtensions();
    return {
      ...result,
      extensions: result.extensions.map((extension) => ({
        ...extension,
        builtIn: extension.builtIn ?? false,
        updateAvailable: extension.updateAvailable ?? false,
        enabled: extension.enabled ?? true,
        switchSupported: extension.switchSupported ?? true,
      })),
    };
  },
  install: (source) => installPiExtension({ source }),
  uninstall: (input) => uninstallPiExtension(input),
  update: (source) => updatePiExtensions(source ? { source } : undefined),
  setEnabled: setPiExtensionEnabled,
};

interface PiExtensionsSectionProps {
  refreshKey?: number;
  /** 扩展变更后刷新页面其余部分。 */
  onChanged?: () => void | Promise<void>;
}

/**
 * Pi 的「扩展管理」区块。
 *
 * 主体实现在共享的 `CliExtensionsSection`——与 OmO Native 逐字相同的部分都在那里
 * （同一份 `pi list` 输出格式、同一套 settings 过滤语义、同一组 CLI 子命令）。
 * 这里只提供 Pi 的 API 实现、推荐清单、外链，以及装在列表下方的 magic-context 设置。
 */
const PiExtensionsSection: React.FC<PiExtensionsSectionProps> = ({ refreshKey, onChanged }) => (
  <CliExtensionsSection
    i18nPrefix="pi"
    service={PI_EXTENSIONS_SERVICE}
    recommendedExtensions={RECOMMENDED_PI_EXTENSIONS}
    packagesUrl={PI_PACKAGES_URL}
    refreshKey={refreshKey}
    onChanged={onChanged}
    // Pi 页外层是 `display: flex; gap: 16px`，间距由容器统一给。
    marginBottom={0}
    // magic-context 只在装了那个包时出现（它是可选扩展，没装就没有配置可谈）。
    // OmO 侧不挂：`MagicContextHarness` 只有 `opencode` / `pi` 两个取值。
    footer={(extensions) =>
      isRecommendedInstalled(extensions, MAGIC_CONTEXT_INSTALL_SOURCE) ? (
        <MagicContextSettings harness="pi" />
      ) : null
    }
  />
);

export default PiExtensionsSection;
