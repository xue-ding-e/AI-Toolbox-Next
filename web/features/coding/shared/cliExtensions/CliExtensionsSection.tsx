import React from 'react';
import {
  Alert,
  App,
  Button,
  Collapse,
  Empty,
  Input,
  Modal,
  Space,
  Switch,
  Tag,
  Tooltip,
  Typography,
} from 'antd';
import {
  AppstoreAddOutlined,
  DeleteOutlined,
  DownloadOutlined,
  FolderOpenOutlined,
  LinkOutlined,
  PlusOutlined,
  ReloadOutlined,
  SyncOutlined,
} from '@ant-design/icons';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useTranslation } from 'react-i18next';

import styles from './CliExtensionsSection.module.less';
import type {
  CliExtensionCommandResult,
  CliExtensionKind,
  CliExtensionListResult,
  CliExtensionSummary,
  CliExtensionsService,
} from './types';

const { Text, Paragraph } = Typography;

/** 一条推荐扩展。`descriptionKey` 走调用方的 i18n 前缀。 */
export interface RecommendedCliExtension {
  name: string;
  installSource: string;
  /** 相对前缀的键尾，例如 `recommended.cometixFooter`。 */
  descriptionKeySuffix: string;
  detailUrl: string;
}

interface CliExtensionsSectionProps {
  /**
   * i18n 前缀（`pi` / `omoNative`）。共享键走 `common.cliExtensions.*`，
   * CLI 专属的推荐列表描述走 `<prefix>.extensions.*`。
   */
  i18nPrefix: string;
  /** 扩展 API。各页面注入自己的实现。 */
  service: CliExtensionsService;
  /** 推荐扩展清单。两个 CLI 的生态不同，所以由调用方给。 */
  recommendedExtensions: RecommendedCliExtension[];
  /** 包市场 / 扩展目录的链接（推荐列表标题旁的外链）。 */
  packagesUrl?: string;
  /**
   * 变更后要刷新页面其余部分的回调（安装 / 卸载 / 启停都会调）。
   *
   * 两个页面都有「扩展列表」以外的状态依赖同一个 `settings.json`
   * （例如 magic-context 区块），所以不只是刷新本区块。
   */
  onChanged?: () => void | Promise<void>;
  /** 递增即重新拉列表（父组件用它触发刷新）。 */
  refreshKey?: number;
  /**
   * 扩展列表下方的附加区块，由调用方决定是否渲染。
   *
   * Pi 用它挂 magic-context 设置（只在装了那个包时出现）；OmO 没有对应物
   * （`MagicContextHarness` 只有 `opencode` / `pi` 两个取值），所以不传。
   */
  footer?: (extensions: CliExtensionSummary[]) => React.ReactNode;
  /**
   * 区块底部的间距。
   *
   * 两种页面骨架的间距来源不同，所以必须由调用方指定：
   * - **Pi 页**：外层容器 `display: flex; gap: 16px` 统一给间距，区块自己不能再加，
   *   否则间距翻倍 —— 传 `0`；
   * - **OmO 页**：容器没有 gap，每个区块自带下边距 —— 传默认的 `16`。
   *
   * 之前写死 `margin-bottom: 0`，OmO 页上就出现了「扩展管理与全局提示词贴在一起」。
   */
  marginBottom?: number;
}

const normalizeSource = (source: string): string => source.trim().toLowerCase();

const getSourceDisplayName = (source: string): string =>
  source.replace(/^(?:npm|file|github|git):/i, '');

const isRecommendedInstalled = (
  extensions: CliExtensionSummary[],
  installSource: string,
): boolean => {
  const normalizedInstallSource = normalizeSource(installSource);
  const normalizedPackageName = normalizedInstallSource.startsWith('npm:')
    ? normalizedInstallSource.slice(4)
    : normalizedInstallSource;

  return extensions.some((extension) => {
    const normalizedSource = normalizeSource(extension.source);
    return (
      normalizedSource === normalizedInstallSource ||
      normalizedSource === normalizedPackageName
    );
  });
};

/**
 * Pi 系 CLI（Pi / OmO Native）的「扩展管理」区块。
 *
 * 两个页面的这一块**逐字相同**——同一份 `list` 输出、同一套 `settings.json`
 * 过滤语义、同一组 CLI 子命令。差异只有三处，全部是 prop：i18n 前缀、
 * API 实现、推荐扩展清单（生态不同）。
 *
 * 后端同理共用 `coding/cli_extensions.rs`，所以这里的每一处行为改动
 * 都会同时作用于两个 CLI。
 */
const CliExtensionsSection: React.FC<CliExtensionsSectionProps> = ({
  i18nPrefix,
  service,
  recommendedExtensions,
  packagesUrl,
  onChanged,
  refreshKey = 0,
  footer,
  marginBottom = 16,
}) => {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [data, setData] = React.useState<CliExtensionListResult | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [customSource, setCustomSource] = React.useState('');
  const [installingSources, setInstallingSources] = React.useState<Set<string>>(() => new Set());
  const [uninstallingSource, setUninstallingSource] = React.useState<string | null>(null);
  const [pendingUninstall, setPendingUninstall] = React.useState<CliExtensionSummary | null>(null);
  const [updating, setUpdating] = React.useState(false);
  const [updatingSource, setUpdatingSource] = React.useState<string | null>(null);
  const [togglingIds, setTogglingIds] = React.useState<Set<string>>(() => new Set());
  const [commandResult, setCommandResult] = React.useState<CliExtensionCommandResult | null>(null);

  // 共享键统一走 `common.cliExtensions.*`，避免两个 CLI 各写一份一模一样的文案。
  const text = React.useCallback(
    (key: string, options?: Record<string, unknown>) => t(`common.cliExtensions.${key}`, options),
    [t],
  );

  const loadExtensions = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await service.list();
      setData(result);
    } catch (loadError) {
      const messageText = loadError instanceof Error ? loadError.message : String(loadError);
      setError(messageText);
    } finally {
      setLoading(false);
    }
  }, [service]);

  React.useEffect(() => {
    void loadExtensions();
  }, [loadExtensions, refreshKey]);

  const extensions = data?.extensions ?? [];
  const updateAvailableCount = extensions.filter((extension) => extension.updateAvailable).length;

  const handleInstall = async (source: string) => {
    const normalizedSource = source.trim();
    if (!normalizedSource) {
      void message.warning(text('sourceRequired'));
      return;
    }

    setInstallingSources((current) => new Set(current).add(normalizedSource));
    try {
      await service.install(normalizedSource);
      void message.success(text('installSuccess'));
      setCustomSource('');
      await loadExtensions();
      await onChanged?.();
    } catch (installError) {
      void message.error(
        installError instanceof Error ? installError.message : String(installError),
      );
    } finally {
      setInstallingSources((current) => {
        const next = new Set(current);
        next.delete(normalizedSource);
        return next;
      });
    }
  };

  const handleConfirmUninstall = async () => {
    if (!pendingUninstall) {
      return;
    }
    const extension = pendingUninstall;
    setUninstallingSource(extension.source);
    try {
      await service.uninstall({
        source: extension.source,
        scope: extension.scope,
        kind: extension.kind,
        path: extension.path,
      });
      void message.success(
        extension.kind === 'package' ? text('uninstallSuccess') : text('deleteSuccess'),
      );
      setPendingUninstall(null);
      await loadExtensions();
      await onChanged?.();
    } catch (uninstallError) {
      void message.error(
        uninstallError instanceof Error ? uninstallError.message : String(uninstallError),
      );
    } finally {
      setUninstallingSource(null);
    }
  };

  const handleToggleEnabled = async (extension: CliExtensionSummary, enabled: boolean) => {
    setTogglingIds((current) => new Set(current).add(extension.id));
    try {
      await service.setEnabled({
        source: extension.source,
        kind: extension.kind,
        enabled,
      });
      // 后端已确认落盘，所以只补这一行、不重拉整个列表
      // （重拉会再跑一次 `list` 与 npm 版本查询）。
      setData((current) =>
        current
          ? {
              ...current,
              extensions: current.extensions.map((item) =>
                item.id === extension.id ? { ...item, enabled } : item,
              ),
            }
          : current,
      );
      await onChanged?.();
    } catch (toggleError) {
      void message.error(
        toggleError instanceof Error ? toggleError.message : String(toggleError),
      );
    } finally {
      setTogglingIds((current) => {
        const next = new Set(current);
        next.delete(extension.id);
        return next;
      });
    }
  };

  const handleUpdateAll = async () => {
    setUpdating(true);
    try {
      const result = await service.update();
      setCommandResult(result);
      await loadExtensions();
      await onChanged?.();
    } catch (updateError) {
      void message.error(
        updateError instanceof Error ? updateError.message : String(updateError),
      );
    } finally {
      setUpdating(false);
    }
  };

  const handleUpdateOne = async (source: string) => {
    const normalizedSource = source.trim();
    if (!normalizedSource) {
      return;
    }
    setUpdatingSource(normalizedSource);
    try {
      const result = await service.update(normalizedSource);
      setCommandResult(result);
      await loadExtensions();
      await onChanged?.();
    } catch (updateError) {
      void message.error(
        updateError instanceof Error ? updateError.message : String(updateError),
      );
    } finally {
      setUpdatingSource(null);
    }
  };

  const handleOpenFolder = async (path: string | undefined) => {
    if (!path) {
      return;
    }
    try {
      await invoke('open_folder', { path });
    } catch (openError) {
      void message.error(openError instanceof Error ? openError.message : String(openError));
    }
  };

  const renderRecommendedExtension = (extension: RecommendedCliExtension) => {
    const installed = isRecommendedInstalled(extensions, extension.installSource);
    const installing = installingSources.has(extension.installSource);

    return (
      <div key={extension.installSource} className={styles.extensionItem}>
        <div className={styles.extensionContent}>
          <div className={styles.extensionTitleRow}>
            <Space size={6} wrap>
              <Text strong>{extension.name}</Text>
              <Text code className={styles.inlineMetaText}>
                {extension.installSource}
              </Text>
              {installed && <Tag color="success">{text('installed')}</Tag>}
            </Space>
          </div>
          <Text type="secondary" className={styles.extensionSecondary}>
            {t(`${i18nPrefix}.extensions.${extension.descriptionKeySuffix}`)}
          </Text>
        </div>
        <Space size={6} className={styles.itemActions}>
          <Tooltip title={text('openPackage')}>
            <Button
              type="text"
              size="small"
              icon={<LinkOutlined />}
              onClick={() => {
                void openUrl(extension.detailUrl);
              }}
            />
          </Tooltip>
          <Button
            size="small"
            icon={<DownloadOutlined />}
            disabled={installed}
            loading={installing}
            onClick={() => {
              void handleInstall(extension.installSource);
            }}
          >
            {installed ? text('installed') : text('install')}
          </Button>
        </Space>
      </div>
    );
  };

  const renderInstalledExtension = (extension: CliExtensionSummary, index: number) => {
    const isPackage = extension.kind === 'package';
    const actionText = isPackage ? text('uninstall') : text('deleteLocal');
    const versionLabel =
      extension.updateAvailable && extension.currentVersion && extension.latestVersion
        ? `${extension.currentVersion} → ${extension.latestVersion}`
        : extension.currentVersion;
    const isUpdatingThis = updatingSource === extension.source;
    const isDisabled = extension.enabled === false;
    // 内置扩展（`pi-deck-*` / `ai-toolbox-*`）不能从本页停用，但卡在停用状态
    // （手改或别的工具写的）仍要能恢复，所以开关只在它已经是关闭时出现。
    //
    // `switchSupported === false` 时**整个开关不出现**（而不是显示成禁用态）：
    // 后端说这个包/本地扩展的过滤器改不动它实际加载什么，一个点了没反应的开关
    // 比没有开关更让人困惑。
    const canToggle =
      (!extension.builtIn || isDisabled) && extension.switchSupported !== false;
    // 项目级条目在项目的 settings 里，本页从不写它。
    const isProjectScope = extension.scope === 'project';

    return (
      <div
        // `list` 可能把同一个 source 打印两次（普通条目 + 过滤条目），
        // 所以后端的 id 不保证唯一；索引让 React key 稳定。
        key={`${extension.id}#${index}`}
        className={[styles.extensionItem, isDisabled ? styles.extensionItemDisabled : '']
          .filter(Boolean)
          .join(' ')}
      >
        <div className={styles.extensionContent}>
          <div className={styles.extensionTitleRow}>
            <Space size={6} wrap>
              <Text strong>{getSourceDisplayName(extension.source)}</Text>
              {isProjectScope && <Tag>{text('scopeProject')}</Tag>}
              {isDisabled && <Tag color="default">{text('disabledTag')}</Tag>}
              {versionLabel && (
                <Text code className={styles.inlineMetaText}>
                  {versionLabel}
                </Text>
              )}
              {extension.updateAvailable && (
                <Button
                  type="link"
                  size="small"
                  className={styles.updateAvailableButton}
                  icon={<SyncOutlined />}
                  loading={isUpdatingThis}
                  disabled={updating || Boolean(updatingSource && !isUpdatingThis)}
                  onClick={() => {
                    void handleUpdateOne(extension.source);
                  }}
                >
                  {text('updateAvailable')}
                </Button>
              )}
              {extension.builtIn && <Tag color="blue">{text('builtIn')}</Tag>}
            </Space>
          </div>
          <Text
            type="secondary"
            className={styles.extensionSecondary}
            title={extension.path || extension.source}
          >
            {extension.source}
          </Text>
        </div>
        <Space size={6} className={styles.itemActions}>
          {canToggle && (
            <Tooltip
              title={
                isProjectScope
                  ? text('enableDisabledReason')
                  : isDisabled
                    ? text('enableAction')
                    : text('disableAction')
              }
            >
              <Switch
                size="small"
                checked={!isDisabled}
                loading={togglingIds.has(extension.id)}
                disabled={isProjectScope || (extension.builtIn && !isDisabled)}
                onChange={(checked) => {
                  void handleToggleEnabled(extension, checked);
                }}
              />
            </Tooltip>
          )}
          {!extension.builtIn && (
            <Tooltip title={actionText}>
              <Button
                danger
                type="text"
                size="small"
                icon={<DeleteOutlined />}
                loading={uninstallingSource === extension.source}
                onClick={() => setPendingUninstall(extension)}
              />
            </Tooltip>
          )}
        </Space>
      </div>
    );
  };

  return (
    <>
      <Collapse
        className={styles.collapseCard}
        style={{ marginBottom }}
        items={[
          {
            key: 'extensions',
            label: (
              <Space>
                <AppstoreAddOutlined />
                <Text strong>{text('title')}</Text>
              </Space>
            ),
            extra: (
              <Space onClick={(event) => event.stopPropagation()}>
                <Button
                  type="link"
                  size="small"
                  icon={<FolderOpenOutlined />}
                  disabled={!data?.extensionsPath}
                  onClick={() => void handleOpenFolder(data?.extensionsPath)}
                >
                  {text('openDirectory')}
                </Button>
                <Button
                  type="link"
                  size="small"
                  icon={<FolderOpenOutlined />}
                  disabled={!data?.packagesPath}
                  onClick={() => void handleOpenFolder(data?.packagesPath)}
                >
                  {text('openPackagesDirectory')}
                </Button>
                <Button
                  type="link"
                  size="small"
                  icon={<ReloadOutlined />}
                  loading={loading}
                  onClick={loadExtensions}
                >
                  {t('common.refresh')}
                </Button>
              </Space>
            ),
            children: (
              <div className={styles.content}>
                {error && (
                  <Alert
                    type="error"
                    showIcon
                    message={text('loadFailed')}
                    description={error}
                  />
                )}
                <div className={styles.metaRow}>
                  <Text type="secondary">{text('cliPathLabel')}</Text>
                  <Text code className={styles.pathText}>
                    {data?.cliPath || '-'}
                  </Text>
                  {data?.cliVersion && (
                    <>
                      <Text type="secondary">{text('cliVersionLabel')}</Text>
                      <Text code className={styles.pathText}>
                        {data.cliVersion}
                      </Text>
                    </>
                  )}
                  <Text type="secondary">{text('pathLabel')}</Text>
                  <Text code className={styles.pathText}>
                    {data?.extensionsPath || '-'}
                  </Text>
                  <Text type="secondary">{text('packagesPathLabel')}</Text>
                  <Text code className={styles.pathText}>
                    {data?.packagesPath || '-'}
                  </Text>
                  <Text type="secondary">{text('restartHint')}</Text>
                </div>

                <div className={styles.customInstallRow}>
                  <Input
                    value={customSource}
                    onChange={(event) => setCustomSource(event.target.value)}
                    onPressEnter={() => {
                      void handleInstall(customSource);
                    }}
                    placeholder={text('sourcePlaceholder')}
                    allowClear
                  />
                  <Button
                    type="primary"
                    icon={<PlusOutlined />}
                    loading={installingSources.has(customSource.trim())}
                    onClick={() => {
                      void handleInstall(customSource);
                    }}
                  >
                    {text('install')}
                  </Button>
                </div>

                {recommendedExtensions.length > 0 && (
                  <Collapse
                    className={styles.innerCollapse}
                    size="small"
                    bordered={false}
                    items={[
                      {
                        key: 'recommended',
                        label: (
                          <Space>
                            <Text strong>{text('recommendedTitle')}</Text>
                            {packagesUrl && (
                              <Button
                                type="link"
                                size="small"
                                className={styles.officialPackagesLink}
                                icon={<LinkOutlined />}
                                onClick={(event) => {
                                  event.stopPropagation();
                                  void openUrl(packagesUrl);
                                }}
                              >
                                {text('officialPackages')}
                              </Button>
                            )}
                            <Text type="secondary">
                              {text('recommendedCount', {
                                count: recommendedExtensions.length,
                              })}
                            </Text>
                          </Space>
                        ),
                        children: (
                          <div className={styles.recommendedList}>
                            {recommendedExtensions.map(renderRecommendedExtension)}
                          </div>
                        ),
                      },
                    ]}
                  />
                )}

                <Collapse
                  className={styles.innerCollapse}
                  size="small"
                  bordered={false}
                  defaultActiveKey={['installed']}
                  items={[
                    {
                      key: 'installed',
                      label: (
                        <Space>
                          <Text strong>{text('installedTitle')}</Text>
                          <Text type="secondary">
                            {text('count', { count: extensions.length })}
                          </Text>
                          {updateAvailableCount > 0 && (
                            <Text type="warning">
                              {text('updateAvailableCount', { count: updateAvailableCount })}
                            </Text>
                          )}
                        </Space>
                      ),
                      extra: (
                        <Button
                          type="link"
                          size="small"
                          icon={<SyncOutlined />}
                          loading={updating}
                          disabled={Boolean(updatingSource)}
                          onClick={(event) => {
                            event.stopPropagation();
                            void handleUpdateAll();
                          }}
                        >
                          {updateAvailableCount > 0
                            ? text('updateAllWithCount', { count: updateAvailableCount })
                            : text('updateAll')}
                        </Button>
                      ),
                      children:
                        loading && !data ? (
                          <div className={styles.loadingText}>{text('loading')}</div>
                        ) : extensions.length > 0 ? (
                          <div className={styles.installedList}>
                            {extensions.map(renderInstalledExtension)}
                          </div>
                        ) : (
                          <Empty
                            image={Empty.PRESENTED_IMAGE_SIMPLE}
                            description={text('emptyInstalled')}
                          />
                        ),
                    },
                  ]}
                />

                {footer?.(extensions)}
              </div>
            ),
          },
        ]}
      />

      <Modal
        title={
          pendingUninstall?.kind === 'package'
            ? text('confirmUninstallTitle')
            : text('confirmDeleteTitle')
        }
        open={!!pendingUninstall}
        okText={pendingUninstall?.kind === 'package' ? text('uninstall') : text('deleteLocal')}
        okButtonProps={{
          danger: true,
          loading: Boolean(pendingUninstall && uninstallingSource === pendingUninstall.source),
        }}
        cancelText={t('common.cancel')}
        onOk={handleConfirmUninstall}
        onCancel={() => setPendingUninstall(null)}
        destroyOnHidden
      >
        {pendingUninstall && (
          <div className={styles.confirmContent}>
            <Paragraph>
              {pendingUninstall.kind === 'package'
                ? text('confirmUninstallContent')
                : text('confirmDeleteContent')}
            </Paragraph>
            <Text code>{pendingUninstall.source}</Text>
            {pendingUninstall.path && (
              <Text type="secondary" className={styles.pathText}>
                {pendingUninstall.path}
              </Text>
            )}
          </div>
        )}
      </Modal>

      <Modal
        title={text('updateResultTitle')}
        open={!!commandResult}
        footer={[
          <Button key="close" type="primary" onClick={() => setCommandResult(null)}>
            {t('common.close')}
          </Button>,
        ]}
        onCancel={() => setCommandResult(null)}
        destroyOnHidden
      >
        {commandResult && (
          <pre className={styles.commandOutput}>
            {`${commandResult.command}\n${commandResult.output || text('emptyCommandOutput')}`}
          </pre>
        )}
      </Modal>
    </>
  );
};

export default CliExtensionsSection;

/** 供调用方构造推荐扩展清单：键尾拼上前缀即是完整 i18n 键。 */
export type { CliExtensionKind };
