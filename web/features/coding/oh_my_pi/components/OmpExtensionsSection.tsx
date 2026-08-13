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

import {
  installOmpExtension,
  listOmpExtensions,
  uninstallOmpExtension,
  updateOmpExtensions,
} from '@/services/ohMyPiApi';
import type {
  OmpExtensionCommandResult,
  OmpExtensionListResult,
  OmpExtensionSummary,
} from '@/types/ohMyPi';

import styles from './OmpExtensionsSection.module.less';

const { Text, Paragraph } = Typography;
const PI_PACKAGES_URL = 'https://omp.sh/docs/plugins';

interface RecommendedOmpExtension {
  name: string;
  installSource: string;
  descriptionKey: string;
  detailUrl: string;
}

interface OmpExtensionsSectionProps {
  refreshKey?: number;
}

// OMP 的插件生态与 Pi 不同,OMP 页面不内置 Pi 的推荐扩展列表。
const RECOMMENDED_OMP_EXTENSIONS: RecommendedOmpExtension[] = [
  {
    name: 'pi-cometix-footer',
    installSource: 'npm:pi-cometix-footer',
    descriptionKey: 'ohMyPi.extensions.recommended.cometixFooter',
    detailUrl: 'https://pi.dev/packages/pi-cometix-footer?name=pi-cometix-footer',
  },
  {
    name: 'pi-hashline-edit-pro',
    installSource: 'npm:pi-hashline-edit-pro',
    descriptionKey: 'ohMyPi.extensions.recommended.hashlineEditPro',
    detailUrl: 'https://pi.dev/packages/pi-hashline-edit-pro?name=pi-hashline-edit-pro',
  },
  {
    name: 'pi-slopchop',
    installSource: 'npm:pi-slopchop',
    descriptionKey: 'ohMyPi.extensions.recommended.slopchop',
    detailUrl: 'https://pi.dev/packages/pi-slopchop?name=pi-slopchop',
  },
  {
    name: '@narumitw/pi-goal',
    installSource: 'npm:@narumitw/pi-goal',
    descriptionKey: 'ohMyPi.extensions.recommended.goal',
    detailUrl: 'https://pi.dev/packages/@narumitw/pi-goal?name=%40narumitw%2Fpi-goal',
  },
  {
    name: '@narumitw/pi-plan-mode',
    installSource: 'npm:@narumitw/pi-plan-mode',
    descriptionKey: 'ohMyPi.extensions.recommended.planMode',
    detailUrl:
      'https://pi.dev/packages/@narumitw/pi-plan-mode?name=%40narumitw%2Fpi-plan-mode',
  },
  {
    name: '@narumitw/pi-subagents',
    installSource: 'npm:@narumitw/pi-subagents',
    descriptionKey: 'ohMyPi.extensions.recommended.subagents',
    detailUrl:
      'https://pi.dev/packages/@narumitw/pi-subagents?name=%40narumitw%2Fpi-subagents',
  },
  {
    name: 'pi-autoresearch',
    installSource: 'npm:pi-autoresearch',
    descriptionKey: 'ohMyPi.extensions.recommended.autoresearch',
    detailUrl: 'https://pi.dev/packages/pi-autoresearch?name=pi-autoresearch',
  },
  {
    name: '@juicesharp/rpiv-ask-user-question',
    installSource: 'npm:@juicesharp/rpiv-ask-user-question',
    descriptionKey: 'ohMyPi.extensions.recommended.askUserQuestion',
    detailUrl:
      'https://pi.dev/packages/@juicesharp/rpiv-ask-user-question?name=%40juicesharp%2Frpiv-ask-user-question',
  },
  {
    name: '@juicesharp/rpiv-todo',
    installSource: 'npm:@juicesharp/rpiv-todo',
    descriptionKey: 'ohMyPi.extensions.recommended.todo',
    detailUrl:
      'https://pi.dev/packages/@juicesharp/rpiv-todo?name=%40juicesharp%2Frpiv-todo',
  },
  {
    name: '@narumitw/pi-btw',
    installSource: 'npm:@narumitw/pi-btw',
    descriptionKey: 'ohMyPi.extensions.recommended.btw',
    detailUrl: 'https://pi.dev/packages/@narumitw/pi-btw?name=%40narumitw%2Fpi-btw',
  },
  {
    name: 'pi-mcp-adapter',
    installSource: 'npm:pi-mcp-adapter',
    descriptionKey: 'ohMyPi.extensions.recommended.mcpAdapter',
    detailUrl: 'https://pi.dev/packages/pi-mcp-adapter?name=pi-mcp-adapter',
  },
  {
    name: '@ff-labs/pi-fff',
    installSource: 'npm:@ff-labs/pi-fff',
    descriptionKey: 'ohMyPi.extensions.recommended.fff',
    detailUrl: 'https://pi.dev/packages/@ff-labs/pi-fff?name=%40ff-labs%2Fpi-fff',
  },
  {
    name: 'pi-rtk-optimizer',
    installSource: 'npm:pi-rtk-optimizer',
    descriptionKey: 'ohMyPi.extensions.recommended.rtkOptimizer',
    detailUrl: 'https://pi.dev/packages/pi-rtk-optimizer?name=pi-rtk-optimizer',
  },
  {
    name: 'pi-cache-optimizer',
    installSource: 'npm:pi-cache-optimizer',
    descriptionKey: 'ohMyPi.extensions.recommended.cacheOptimizer',
    detailUrl: 'https://pi.dev/packages/pi-cache-optimizer?name=pi-cache-optimizer',
  },
  {
    name: '@narumitw/pi-lsp',
    installSource: 'npm:@narumitw/pi-lsp',
    descriptionKey: 'ohMyPi.extensions.recommended.lsp',
    detailUrl: 'https://pi.dev/packages/@narumitw/pi-lsp?name=%40narumitw%2Fpi-lsp',
  },
  {
    name: 'pi-agent-browser-native',
    installSource: 'npm:pi-agent-browser-native',
    descriptionKey: 'ohMyPi.extensions.recommended.agentBrowserNative',
    detailUrl:
      'https://pi.dev/packages/pi-agent-browser-native?name=pi-agent-browser-native',
  },
  {
    name: 'pi-add-dir',
    installSource: 'npm:pi-add-dir',
    descriptionKey: 'ohMyPi.extensions.recommended.addDir',
    detailUrl: 'https://pi.dev/packages/pi-add-dir?name=pi-add-dir',
  },
  {
    name: 'pi-workspace-history',
    installSource: 'npm:pi-workspace-history',
    descriptionKey: 'ohMyPi.extensions.recommended.workspaceHistory',
    detailUrl: 'https://pi.dev/packages/pi-workspace-history?name=pi-workspace-history',
  },
  {
    name: '@narumitw/pi-caffeinate',
    installSource: 'npm:@narumitw/pi-caffeinate',
    descriptionKey: 'ohMyPi.extensions.recommended.caffeinate',
    detailUrl:
      'https://pi.dev/packages/@narumitw/pi-caffeinate?name=%40narumitw%2Fpi-caffeinate',
  },
  {
    name: '@tmustier/pi-raw-paste',
    installSource: 'npm:@tmustier/pi-raw-paste',
    descriptionKey: 'ohMyPi.extensions.recommended.rawPaste',
    detailUrl:
      'https://pi.dev/packages/@tmustier/pi-raw-paste?name=%40tmustier%2Fpi-raw-paste',
  },
  {
    name: '@victor-software-house/pi-curated-themes',
    installSource: 'npm:@victor-software-house/pi-curated-themes',
    descriptionKey: 'ohMyPi.extensions.recommended.curatedThemes',
    detailUrl:
      'https://pi.dev/packages/@victor-software-house/pi-curated-themes?name=%40victor-software-house%2Fpi-curated-themes',
  },
  {
    name: '@cortexkit/pi-magic-context',
    installSource: 'npm:@cortexkit/pi-magic-context',
    descriptionKey: 'ohMyPi.extensions.recommended.magicContext',
    detailUrl: 'https://github.com/cortexkit/magic-context',
  },
];

const normalizeSource = (source: string): string => source.trim().toLowerCase();

const getSourceDisplayName = (source: string): string => (
  source.replace(/^(?:npm|file|github|git):/i, '')
);

const isRecommendedInstalled = (
  extensions: OmpExtensionSummary[],
  installSource: string,
): boolean => {
  const normalizedInstallSource = normalizeSource(installSource);
  const normalizedPackageName = normalizedInstallSource.startsWith('npm:')
    ? normalizedInstallSource.slice(4)
    : normalizedInstallSource;

  return extensions.some((extension) => {
    const normalizedSource = normalizeSource(extension.source);
    return normalizedSource === normalizedInstallSource || normalizedSource === normalizedPackageName;
  });
};



const OmpExtensionsSection: React.FC<OmpExtensionsSectionProps> = ({ refreshKey = 0 }) => {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [data, setData] = React.useState<OmpExtensionListResult | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [customSource, setCustomSource] = React.useState('');
  const [installingSources, setInstallingSources] = React.useState<Set<string>>(() => new Set());
  const [uninstallingSource, setUninstallingSource] = React.useState<string | null>(null);
  const [pendingUninstall, setPendingUninstall] = React.useState<OmpExtensionSummary | null>(null);
  const [updating, setUpdating] = React.useState(false);
  const [updatingSource, setUpdatingSource] = React.useState<string | null>(null);
  const [commandResult, setCommandResult] = React.useState<OmpExtensionCommandResult | null>(null);

  const loadExtensions = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await listOmpExtensions();
      setData(result);
    } catch (loadError) {
      const messageText = loadError instanceof Error ? loadError.message : String(loadError);
      setError(messageText);
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    void loadExtensions();
  }, [loadExtensions, refreshKey]);

  const extensions = data?.extensions ?? [];
  const updateAvailableCount = extensions.filter((extension) => extension.updateAvailable).length;

  const handleInstall = async (source: string) => {
    const normalizedSource = source.trim();
    if (!normalizedSource) {
      void message.warning(t('ohMyPi.extensions.sourceRequired'));
      return;
    }

    setInstallingSources((current) => new Set(current).add(normalizedSource));
    try {
      await installOmpExtension({ source: normalizedSource });
      void message.success(t('ohMyPi.extensions.installSuccess'));
      setCustomSource('');
      await loadExtensions();
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
      await uninstallOmpExtension({
        source: extension.source,
        scope: extension.scope,
        kind: extension.kind,
        path: extension.path,
      });
      void message.success(
        extension.kind === 'package'
          ? t('ohMyPi.extensions.uninstallSuccess')
          : t('ohMyPi.extensions.deleteSuccess'),
      );
      setPendingUninstall(null);
      await loadExtensions();
    } catch (uninstallError) {
      void message.error(
        uninstallError instanceof Error ? uninstallError.message : String(uninstallError),
      );
    } finally {
      setUninstallingSource(null);
    }
  };

  const handleUpdateAll = async () => {
    setUpdating(true);
    try {
      const result = await updateOmpExtensions();
      setCommandResult(result);
      await loadExtensions();
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
      const result = await updateOmpExtensions({ source: normalizedSource });
      setCommandResult(result);
      await loadExtensions();
    } catch (updateError) {
      void message.error(
        updateError instanceof Error ? updateError.message : String(updateError),
      );
    } finally {
      setUpdatingSource(null);
    }
  };

  const handleOpenExtensionsFolder = async () => {
    if (!data?.extensionsPath) {
      return;
    }
    try {
      await invoke('open_folder', { path: data.extensionsPath });
    } catch (openError) {
      void message.error(openError instanceof Error ? openError.message : String(openError));
    }
  };

  const handleOpenPackagesFolder = async () => {
    if (!data?.packagesPath) {
      return;
    }
    try {
      await invoke('open_folder', { path: data.packagesPath });
    } catch (openError) {
      void message.error(openError instanceof Error ? openError.message : String(openError));
    }
  };

  const renderRecommendedExtension = (extension: RecommendedOmpExtension) => {
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
              {installed && <Tag color="success">{t('ohMyPi.extensions.installed')}</Tag>}
            </Space>
          </div>
          <Text type="secondary" className={styles.extensionSecondary}>
            {t(extension.descriptionKey)}
          </Text>
        </div>
        <Space size={6} className={styles.itemActions}>
          <Tooltip title={t('ohMyPi.extensions.openPackage')}>
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
            {installed ? t('ohMyPi.extensions.installed') : t('ohMyPi.extensions.install')}
          </Button>
        </Space>
      </div>
    );
  };

  const renderInstalledExtension = (extension: OmpExtensionSummary) => {
    const isPackage = extension.kind === 'package';
    const actionText = isPackage ? t('ohMyPi.extensions.uninstall') : t('ohMyPi.extensions.deleteLocal');
    const versionLabel = extension.updateAvailable
      && extension.currentVersion
      && extension.latestVersion
      ? `${extension.currentVersion} → ${extension.latestVersion}`
      : extension.currentVersion;
    const isUpdatingThis = updatingSource === extension.source;

    return (
      <div key={extension.id} className={styles.extensionItem}>
        <div className={styles.extensionContent}>
          <div className={styles.extensionTitleRow}>
            <Space size={6} wrap>
              <Text strong>{getSourceDisplayName(extension.source)}</Text>
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
                  {t('ohMyPi.extensions.updateAvailable')}
                </Button>
              )}
              {extension.builtIn && <Tag color="blue">{t('ohMyPi.extensions.builtIn')}</Tag>}
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
        items={[
          {
            key: 'extensions',
            label: (
              <Space>
                <AppstoreAddOutlined />
                <Text strong>{t('ohMyPi.extensions.title')}</Text>
              </Space>
            ),
            extra: (
              <Space onClick={(event) => event.stopPropagation()}>
                <Button
                  type="link"
                  size="small"
                  icon={<FolderOpenOutlined />}
                  disabled={!data?.extensionsPath}
                  onClick={handleOpenExtensionsFolder}
                >
                  {t('ohMyPi.extensions.openDirectory')}
                </Button>
                <Button
                  type="link"
                  size="small"
                  icon={<FolderOpenOutlined />}
                  disabled={!data?.packagesPath}
                  onClick={handleOpenPackagesFolder}
                >
                  {t('ohMyPi.extensions.openPackagesDirectory')}
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
                    message={t('ohMyPi.extensions.loadFailed')}
                    description={error}
                  />
                )}
                <div className={styles.metaRow}>
                  <Text type="secondary">{t('ohMyPi.extensions.cliPathLabel')}</Text>
                  <Text code className={styles.pathText}>
                    {data?.cliPath || '-'}
                  </Text>
                  {data?.cliVersion && (
                    <>
                      <Text type="secondary">{t('ohMyPi.extensions.cliVersionLabel')}</Text>
                      <Text code className={styles.pathText}>
                        {data.cliVersion}
                      </Text>
                    </>
                  )}
                  <Text type="secondary">{t('ohMyPi.extensions.pathLabel')}</Text>
                  <Text code className={styles.pathText}>
                    {data?.extensionsPath || '-'}
                  </Text>
                  <Text type="secondary">{t('ohMyPi.extensions.packagesPathLabel')}</Text>
                  <Text code className={styles.pathText}>
                    {data?.packagesPath || '-'}
                  </Text>
                  <Text type="secondary">{t('ohMyPi.extensions.restartHint')}</Text>
                </div>

                <div className={styles.customInstallRow}>
                  <Input
                    value={customSource}
                    onChange={(event) => setCustomSource(event.target.value)}
                    onPressEnter={() => {
                      void handleInstall(customSource);
                    }}
                    placeholder={t('ohMyPi.extensions.sourcePlaceholder')}
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
                    {t('ohMyPi.extensions.install')}
                  </Button>
                </div>

                <Collapse
                  className={styles.innerCollapse}
                  size="small"
                  bordered={false}
                  items={[
                    {
                      key: 'recommended',
                      label: (
                        <Space>
                          <Text strong>{t('ohMyPi.extensions.recommendedTitle')}</Text>
                          <Button
                            type="link"
                            size="small"
                            className={styles.officialPackagesLink}
                            icon={<LinkOutlined />}
                            onClick={(event) => {
                              event.stopPropagation();
                              void openUrl(PI_PACKAGES_URL);
                            }}
                          >
                            {t('ohMyPi.extensions.officialPackages')}
                          </Button>
                          <Text type="secondary">
                            {t('ohMyPi.extensions.recommendedCount', {
                              count: RECOMMENDED_OMP_EXTENSIONS.length,
                            })}
                          </Text>
                        </Space>
                      ),
                      children: (
                        <div className={styles.recommendedList}>
                          {RECOMMENDED_OMP_EXTENSIONS.map(renderRecommendedExtension)}
                        </div>
                      ),
                    },
                  ]}
                />

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
                          <Text strong>{t('ohMyPi.extensions.installedTitle')}</Text>
                          <Text type="secondary">
                            {t('ohMyPi.extensions.count', { count: extensions.length })}
                          </Text>
                          {updateAvailableCount > 0 && (
                            <Text type="warning">
                              {t('ohMyPi.extensions.updateAvailableCount', {
                                count: updateAvailableCount,
                              })}
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
                            ? t('ohMyPi.extensions.updateAllWithCount', {
                                count: updateAvailableCount,
                              })
                            : t('ohMyPi.extensions.updateAll')}
                        </Button>
                      ),
                      children: loading && !data ? (
                        <div className={styles.loadingText}>{t('ohMyPi.extensions.loading')}</div>
                      ) : extensions.length > 0 ? (
                        <div className={styles.installedList}>
                          {extensions.map(renderInstalledExtension)}
                        </div>
                      ) : (
                        <Empty
                          image={Empty.PRESENTED_IMAGE_SIMPLE}
                          description={t('ohMyPi.extensions.emptyInstalled')}
                        />
                      ),
                    },
                  ]}
                />

              </div>
            ),
          },
        ]}
      />

      <Modal
        title={pendingUninstall?.kind === 'package'
          ? t('ohMyPi.extensions.confirmUninstallTitle')
          : t('ohMyPi.extensions.confirmDeleteTitle')}
        open={!!pendingUninstall}
        okText={pendingUninstall?.kind === 'package'
          ? t('ohMyPi.extensions.uninstall')
          : t('ohMyPi.extensions.deleteLocal')}
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
                ? t('ohMyPi.extensions.confirmUninstallContent')
                : t('ohMyPi.extensions.confirmDeleteContent')}
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
        title={t('ohMyPi.extensions.updateResultTitle')}
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
            {`${commandResult.command}\n${commandResult.output || t('ohMyPi.extensions.emptyCommandOutput')}`}
          </pre>
        )}
      </Modal>
    </>
  );
};

export default OmpExtensionsSection;
