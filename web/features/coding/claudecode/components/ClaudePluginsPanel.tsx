import React from 'react';
import {
  Alert,
  Button,
  Collapse,
  Empty,
  Input,
  Modal,
  Popconfirm,
  Spin,
  Tabs,
  Tag,
  Typography,
  message,
} from 'antd';
import {
  CloudDownloadOutlined,
  CodeSandboxOutlined,
  DeleteOutlined,
  EyeOutlined,
  LinkOutlined,
  PlusOutlined,
  ReloadOutlined,
  SearchOutlined,
  SettingOutlined,
  StopOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import JsonPreviewModal from '@/components/common/JsonPreviewModal';
import {
  addClaudeMarketplace,
  disableClaudePluginUserScope,
  enableClaudePluginUserScope,
  getClaudePluginRuntimeStatus,
  installClaudePluginUserScope,
  listClaudeInstalledPlugins,
  listClaudeKnownMarketplaces,
  listClaudeMarketplacePlugins,
  removeClaudeMarketplace,
  uninstallClaudePluginUserScope,
  updateClaudeMarketplace,
  updateClaudePluginUserScope,
} from '@/services/claudeCodeApi';
import type {
  ClaudeInstalledPlugin,
  ClaudeKnownMarketplace,
  ClaudeMarketplacePlugin,
  ClaudePluginRuntimeStatus,
} from '@/types/claudecode';
import styles from './ClaudePluginsPanel.module.less';

const { Text, Link } = Typography;

interface ClaudePluginsPanelProps {
  refreshToken?: number;
}

function formatScopeList(scopes: string[]): string {
  if (scopes.length === 0) {
    return '-';
  }

  return scopes.join(', ');
}

function matchesMarketplacePlugin(
  plugin: ClaudeMarketplacePlugin,
  normalizedKeyword: string,
): boolean {
  if (!normalizedKeyword) {
    return true;
  }

  const searchableText = [
    plugin.pluginId,
    plugin.name,
    plugin.marketplaceName,
    plugin.description,
    plugin.category,
    plugin.homepage,
    ...plugin.tags,
  ]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();

  return searchableText.includes(normalizedKeyword);
}

const ClaudePluginsPanel: React.FC<ClaudePluginsPanelProps> = ({ refreshToken = 0 }) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [submitting, setSubmitting] = React.useState(false);
  const [activeTabKey, setActiveTabKey] = React.useState('installed');
  const [runtimeCollapsed, setRuntimeCollapsed] = React.useState(true);
  const [addMarketplaceModalOpen, setAddMarketplaceModalOpen] = React.useState(false);
  const [runtimeStatus, setRuntimeStatus] = React.useState<ClaudePluginRuntimeStatus | null>(null);
  const [installedPlugins, setInstalledPlugins] = React.useState<ClaudeInstalledPlugin[]>([]);
  const [knownMarketplaces, setKnownMarketplaces] = React.useState<ClaudeKnownMarketplace[]>([]);
  const [marketplacePlugins, setMarketplacePlugins] = React.useState<ClaudeMarketplacePlugin[]>([]);
  const [marketplaceSourceInput, setMarketplaceSourceInput] = React.useState('');
  const [discoverSearchKeyword, setDiscoverSearchKeyword] = React.useState('');
  const [previewTitle, setPreviewTitle] = React.useState('');
  const [previewData, setPreviewData] = React.useState<unknown>(null);
  const [previewOpen, setPreviewOpen] = React.useState(false);

  const deferredDiscoverSearchKeyword = React.useDeferredValue(
    discoverSearchKeyword.trim().toLowerCase(),
  );

  const loadData = React.useCallback(async (silent = false) => {
    setLoading(true);
    try {
      const [runtime, installed, marketplaces, discoverPlugins] = await Promise.all([
        getClaudePluginRuntimeStatus(),
        listClaudeInstalledPlugins(),
        listClaudeKnownMarketplaces(),
        listClaudeMarketplacePlugins(),
      ]);
      setRuntimeStatus(runtime);
      setInstalledPlugins(installed);
      setKnownMarketplaces(marketplaces);
      setMarketplacePlugins(discoverPlugins);
    } catch (error) {
      console.error('Failed to load Claude plugins panel data:', error);
      if (!silent) {
        message.error(t('common.error'));
      }
    } finally {
      setLoading(false);
    }
  }, [t]);

  React.useEffect(() => {
    loadData(true);
  }, [loadData, refreshToken]);

  const runAction = React.useCallback(async (
    action: () => Promise<void>,
    successMessage: string,
  ): Promise<boolean> => {
    setSubmitting(true);
    try {
      await action();
      message.success(successMessage);
      await loadData(true);
      return true;
    } catch (error) {
      console.error('Claude plugin action failed:', error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
      return false;
    } finally {
      setSubmitting(false);
    }
  }, [loadData, t]);

  const handleAddMarketplace = async () => {
    const normalizedSource = marketplaceSourceInput.trim();
    if (!normalizedSource) {
      message.warning(t('claudecode.plugins.marketplaces.sourceRequired'));
      return;
    }

    const success = await runAction(
      () => addClaudeMarketplace({ source: normalizedSource }),
      t('claudecode.plugins.marketplaces.addSuccess'),
    );

    if (success) {
      setMarketplaceSourceInput('');
      setAddMarketplaceModalOpen(false);
    }
  };

  const handlePreviewMarketplaceSource = (marketplace: ClaudeKnownMarketplace) => {
    setPreviewTitle(`${marketplace.name} Source`);
    setPreviewData(marketplace.source);
    setPreviewOpen(true);
  };

  const handlePreviewPluginSource = (plugin: ClaudeMarketplacePlugin) => {
    setPreviewTitle(`${plugin.pluginId} Source`);
    setPreviewData(plugin.source);
    setPreviewOpen(true);
  };

  const filteredMarketplacePlugins = React.useMemo(
    () => marketplacePlugins.filter((plugin) => matchesMarketplacePlugin(plugin, deferredDiscoverSearchKeyword)),
    [marketplacePlugins, deferredDiscoverSearchKeyword],
  );

  const installedItems = installedPlugins.length === 0 ? (
    <div className={styles.emptyWrap}>
      <Empty description={t('claudecode.plugins.installed.empty')} />
    </div>
  ) : (
    <div className={styles.list}>
      {installedPlugins.map((plugin) => {
        const userScopeActionDisabled = !plugin.userScopeInstalled;

        return (
          <div key={plugin.pluginId} className={styles.pluginCard}>
            <div className={styles.pluginHeader}>
              <div className={styles.pluginTitleWrap}>
                <div className={styles.pluginTitleRow}>
                  <Text className={styles.pluginTitle}>{plugin.name}</Text>
                  <Tag color={plugin.userScopeInstalled ? (plugin.userScopeEnabled ? 'green' : 'default') : 'gold'}>
                    {plugin.userScopeInstalled
                      ? (plugin.userScopeEnabled
                        ? t('claudecode.plugins.installed.enabled')
                        : t('claudecode.plugins.installed.disabled'))
                      : t('claudecode.plugins.installed.nonUserScope')}
                  </Tag>
                  <Tag>{plugin.marketplaceName}</Tag>
                  {plugin.version ? <Tag>{plugin.version}</Tag> : null}
                </div>
                <Text code className={styles.pluginId}>{plugin.pluginId}</Text>
                {plugin.description ? (
                  <div className={styles.pluginDescription}>{plugin.description}</div>
                ) : null}
              </div>

              <div className={styles.pluginActions}>
                <Button
                  type="text"
                  size="small"
                  icon={plugin.userScopeEnabled ? <StopOutlined /> : <SettingOutlined />}
                  loading={submitting}
                  disabled={userScopeActionDisabled}
                  onClick={() => runAction(
                    () => (
                      plugin.userScopeEnabled
                        ? disableClaudePluginUserScope({ pluginId: plugin.pluginId })
                        : enableClaudePluginUserScope({ pluginId: plugin.pluginId })
                    ),
                    plugin.userScopeEnabled
                      ? t('claudecode.plugins.installed.disableSuccess')
                      : t('claudecode.plugins.installed.enableSuccess'),
                  )}
                >
                  {plugin.userScopeEnabled
                    ? t('claudecode.plugins.installed.disable')
                    : t('claudecode.plugins.installed.enable')}
                </Button>
                <Button
                  type="text"
                  size="small"
                  icon={<ReloadOutlined />}
                  loading={submitting}
                  disabled={userScopeActionDisabled}
                  onClick={() => runAction(
                    () => updateClaudePluginUserScope({ pluginId: plugin.pluginId }),
                    t('claudecode.plugins.installed.updateSuccess'),
                  )}
                >
                  {t('claudecode.plugins.installed.update')}
                </Button>
                <Popconfirm
                  title={t('claudecode.plugins.installed.uninstallConfirm', { name: plugin.name })}
                  onConfirm={() => runAction(
                    () => uninstallClaudePluginUserScope({ pluginId: plugin.pluginId }),
                    t('claudecode.plugins.installed.uninstallSuccess'),
                  )}
                  okText={t('common.confirm')}
                  cancelText={t('common.cancel')}
                >
                  <Button
                    type="text"
                    size="small"
                    danger
                    icon={<DeleteOutlined />}
                    loading={submitting}
                    disabled={userScopeActionDisabled}
                  >
                    {t('claudecode.plugins.installed.uninstall')}
                  </Button>
                </Popconfirm>
              </div>
            </div>

            <div className={styles.pluginMeta}>
              <div className={styles.pluginMetaItem}>
                <Text className={styles.pluginMetaLabel}>
                  {t('claudecode.plugins.installed.installScopes')}:
                </Text>{' '}
                <Text>{formatScopeList(plugin.installScopes)}</Text>
              </div>
              {!plugin.userScopeInstalled ? (
                <div className={styles.pluginMetaItem}>
                  <Text className={styles.pluginMetaLabel}>
                    {t('claudecode.plugins.installed.userScopeHint')}
                  </Text>
                </div>
              ) : null}
              {plugin.installPath ? (
                <div className={styles.pluginMetaItem}>
                  <Text className={styles.pluginMetaLabel}>
                    {t('claudecode.plugins.installed.installPath')}:
                  </Text>{' '}
                  <Text code>{plugin.installPath}</Text>
                </div>
              ) : null}
            </div>

            <div className={styles.tagList}>
              {plugin.hasSkills ? <Tag color="blue">skills</Tag> : null}
              {plugin.hasAgents ? <Tag color="cyan">agents</Tag> : null}
              {plugin.hasHooks ? <Tag color="gold">hooks</Tag> : null}
              {plugin.hasMcpServers ? <Tag color="purple">MCP</Tag> : null}
              {plugin.hasLspServers ? <Tag color="geekblue">LSP</Tag> : null}
              {plugin.homepage ? (
                <Link onClick={() => openUrl(plugin.homepage!)}>
                  <LinkOutlined /> {t('claudecode.plugins.common.homepage')}
                </Link>
              ) : null}
            </div>
          </div>
        );
      })}
    </div>
  );

  const marketplaceItems = (
    <>
      {knownMarketplaces.length === 0 ? (
        <div className={styles.emptyWrap}>
          <Empty description={t('claudecode.plugins.marketplaces.empty')} />
        </div>
      ) : (
        <div className={styles.list}>
          {knownMarketplaces.map((marketplace) => (
            <div key={marketplace.name} className={styles.pluginCard}>
              <div className={styles.pluginHeader}>
                <div className={styles.pluginTitleWrap}>
                  <div className={styles.pluginTitleRow}>
                    <Text className={styles.pluginTitle}>{marketplace.name}</Text>
                    <Tag color={marketplace.autoUpdateEnabled ? 'green' : 'default'}>
                      {marketplace.autoUpdateEnabled
                        ? t('claudecode.plugins.marketplaces.autoUpdateOn')
                        : t('claudecode.plugins.marketplaces.autoUpdateOff')}
                    </Tag>
                    <Tag>{t('claudecode.plugins.marketplaces.pluginCount', { count: marketplace.pluginCount })}</Tag>
                  </div>
                  {marketplace.description ? (
                    <div className={styles.pluginDescription}>{marketplace.description}</div>
                  ) : null}
                </div>

                <div className={styles.pluginActions}>
                  <Button
                    type="text"
                    size="small"
                    icon={<EyeOutlined />}
                    onClick={() => handlePreviewMarketplaceSource(marketplace)}
                  >
                    {t('common.preview')}
                  </Button>
                  <Button
                    type="text"
                    size="small"
                    icon={<ReloadOutlined />}
                    loading={submitting}
                    onClick={() => runAction(
                      () => updateClaudeMarketplace({ marketplaceName: marketplace.name }),
                      t('claudecode.plugins.marketplaces.updateSuccess'),
                    )}
                  >
                    {t('claudecode.plugins.marketplaces.update')}
                  </Button>
                  <Popconfirm
                    title={t('claudecode.plugins.marketplaces.removeConfirm', { name: marketplace.name })}
                    onConfirm={() => runAction(
                      () => removeClaudeMarketplace({ marketplaceName: marketplace.name }),
                      t('claudecode.plugins.marketplaces.removeSuccess'),
                    )}
                    okText={t('common.confirm')}
                    cancelText={t('common.cancel')}
                  >
                    <Button type="text" size="small" danger icon={<DeleteOutlined />} loading={submitting}>
                      {t('claudecode.plugins.marketplaces.remove')}
                    </Button>
                  </Popconfirm>
                </div>
              </div>

              <div className={styles.pluginMeta}>
                {marketplace.installLocation ? (
                  <div className={styles.pluginMetaItem}>
                    <Text className={styles.pluginMetaLabel}>
                      {t('claudecode.plugins.marketplaces.installLocation')}:
                    </Text>{' '}
                    <Text code>{marketplace.installLocation}</Text>
                  </div>
                ) : null}
                {marketplace.lastUpdated ? (
                  <div className={styles.pluginMetaItem}>
                    <Text className={styles.pluginMetaLabel}>
                      {t('claudecode.plugins.marketplaces.lastUpdated')}:
                    </Text>{' '}
                    <Text>{marketplace.lastUpdated}</Text>
                  </div>
                ) : null}
              </div>
            </div>
          ))}
        </div>
      )}

      {marketplacePlugins.length > 0 ? (
        <div className={styles.discoverSection}>
          <Alert
            type="info"
            showIcon
            message={t('claudecode.plugins.marketplaces.discoverHint')}
          />
          <div className={styles.discoverToolbar}>
            <Input
              allowClear
              value={discoverSearchKeyword}
              onChange={(event) => setDiscoverSearchKeyword(event.target.value)}
              placeholder={t('claudecode.plugins.marketplaces.searchPlaceholder')}
              prefix={<SearchOutlined />}
            />
          </div>

          {filteredMarketplacePlugins.length === 0 ? (
            <div className={styles.emptyWrap}>
              <Empty description={t('claudecode.plugins.marketplaces.searchEmpty')} />
            </div>
          ) : (
            <div className={styles.list}>
              {filteredMarketplacePlugins.map((plugin) => {
                const installedInAnyScope = installedPlugins.some((item) => item.pluginId === plugin.pluginId);
                const userScopeInstalled = installedPlugins.some(
                  (item) => item.pluginId === plugin.pluginId && item.userScopeInstalled,
                );

                return (
                  <div key={plugin.pluginId} className={styles.pluginCard}>
                    <div className={styles.pluginHeader}>
                      <div className={styles.pluginTitleWrap}>
                        <div className={styles.pluginTitleRow}>
                          <Text className={styles.pluginTitle}>{plugin.name}</Text>
                          <Tag>{plugin.marketplaceName}</Tag>
                          {plugin.version ? <Tag>{plugin.version}</Tag> : null}
                          {installedInAnyScope ? (
                            <Tag color={userScopeInstalled ? 'green' : 'gold'}>
                              {userScopeInstalled
                                ? t('claudecode.plugins.marketplaces.installed')
                                : t('claudecode.plugins.marketplaces.installedOtherScope')}
                            </Tag>
                          ) : null}
                        </div>
                        <Text code className={styles.pluginId}>{plugin.pluginId}</Text>
                        {plugin.description ? (
                          <div className={styles.pluginDescription}>{plugin.description}</div>
                        ) : null}
                      </div>

                      <div className={styles.pluginActions}>
                        <Button
                          type="text"
                          size="small"
                          icon={<EyeOutlined />}
                          onClick={() => handlePreviewPluginSource(plugin)}
                        >
                          {t('common.preview')}
                        </Button>
                        <Button
                          type="text"
                          size="small"
                          icon={<CloudDownloadOutlined />}
                          loading={submitting}
                          disabled={userScopeInstalled}
                          onClick={() => runAction(
                            () => installClaudePluginUserScope({ pluginId: plugin.pluginId }),
                            t('claudecode.plugins.marketplaces.installSuccess'),
                          )}
                        >
                          {userScopeInstalled
                            ? t('claudecode.plugins.marketplaces.alreadyInstalled')
                            : t('claudecode.plugins.marketplaces.install')}
                        </Button>
                      </div>
                    </div>

                    <div className={styles.tagList}>
                      {plugin.category ? <Tag color="blue">{plugin.category}</Tag> : null}
                      {plugin.tags.map((tag) => (
                        <Tag key={`${plugin.pluginId}-${tag}`}>{tag}</Tag>
                      ))}
                      {plugin.homepage ? (
                        <Link onClick={() => openUrl(plugin.homepage!)}>
                          <LinkOutlined /> {t('claudecode.plugins.common.homepage')}
                        </Link>
                      ) : null}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      ) : null}
    </>
  );

  return (
    <>
      <Spin spinning={loading}>
        <div className={styles.panel}>
          <div className={styles.hintBlock}>
            <div>{t('claudecode.plugins.sectionHint')}</div>
          </div>

          {runtimeStatus ? (
            <Collapse
              bordered={false}
              className={styles.runtimeCollapse}
              activeKey={runtimeCollapsed ? [] : ['runtime']}
              onChange={(keys) => setRuntimeCollapsed(!keys.includes('runtime'))}
              items={[
                {
                  key: 'runtime',
                  label: (
                    <div className={styles.runtimeCollapseHeader}>
                      <div>
                        <div className={styles.runtimeTitle}>{t('claudecode.plugins.runtime.title')}</div>
                        <span className={styles.runtimeHint}>
                          {t('claudecode.plugins.runtime.description')}
                        </span>
                      </div>
                      <div className={styles.runtimeTags}>
                        <Tag color={runtimeStatus.mode === 'wslDirect' ? 'cyan' : 'blue'}>
                          {runtimeStatus.mode === 'wslDirect'
                            ? t('claudecode.plugins.runtime.wslDirect', {
                                distro: runtimeStatus.distro || '-',
                              })
                            : t('claudecode.plugins.runtime.local')}
                        </Tag>
                        <Tag>
                          {t(`claudecode.rootPathSource.modal.source${runtimeStatus.source.charAt(0).toUpperCase()}${runtimeStatus.source.slice(1)}`)}
                        </Tag>
                      </div>
                    </div>
                  ),
                  children: (
                    <div className={styles.runtimeGrid}>
                      <div className={styles.runtimeItem}>
                        <span className={styles.runtimeLabel}>{t('claudecode.plugins.runtime.rootDir')}</span>
                        <Text code className={styles.runtimeValue}>{runtimeStatus.rootDir}</Text>
                      </div>
                      <div className={styles.runtimeItem}>
                        <span className={styles.runtimeLabel}>{t('claudecode.plugins.runtime.pluginsDir')}</span>
                        <Text code className={styles.runtimeValue}>{runtimeStatus.pluginsDir}</Text>
                      </div>
                      <div className={styles.runtimeItem}>
                        <span className={styles.runtimeLabel}>{t('claudecode.plugins.runtime.settingsPath')}</span>
                        <Text code className={styles.runtimeValue}>{runtimeStatus.settingsPath}</Text>
                      </div>
                      {runtimeStatus.linuxRootDir ? (
                        <div className={styles.runtimeItem}>
                          <span className={styles.runtimeLabel}>{t('claudecode.plugins.runtime.linuxRootDir')}</span>
                          <Text code className={styles.runtimeValue}>{runtimeStatus.linuxRootDir}</Text>
                        </div>
                      ) : null}
                    </div>
                  ),
                },
              ]}
            />
          ) : null}

          <section className={styles.tabsCard}>
            <Tabs
              activeKey={activeTabKey}
              destroyInactiveTabPane={false}
              onChange={setActiveTabKey}
              tabBarExtraContent={{
                right: (
                  <div className={styles.tabExtra}>
                    <Button
                      type="text"
                      size="small"
                      icon={<ReloadOutlined />}
                      onClick={() => loadData()}
                    >
                      {t('common.refresh')}
                    </Button>
                    {activeTabKey === 'marketplaces' ? (
                      <Button
                        type="text"
                        size="small"
                        icon={<PlusOutlined />}
                        onClick={() => setAddMarketplaceModalOpen(true)}
                      >
                        {t('claudecode.plugins.marketplaces.add')}
                      </Button>
                    ) : null}
                    <Button
                      type="text"
                      size="small"
                      icon={<CodeSandboxOutlined />}
                      onClick={() => openUrl('https://code.claude.com/docs/en/discover-plugins')}
                    >
                      {t('claudecode.plugins.viewDocs')}
                    </Button>
                  </div>
                ),
              }}
              items={[
                {
                  key: 'installed',
                  label: `${t('claudecode.plugins.installed.title')} (${installedPlugins.length})`,
                  children: installedItems,
                },
                {
                  key: 'marketplaces',
                  label: `${t('claudecode.plugins.marketplaces.title')} (${knownMarketplaces.length})`,
                  children: marketplaceItems,
                },
              ]}
            />
          </section>
        </div>
      </Spin>

      <Modal
        open={addMarketplaceModalOpen}
        title={t('claudecode.plugins.marketplaces.addModalTitle')}
        okText={t('claudecode.plugins.marketplaces.add')}
        cancelText={t('common.cancel')}
        confirmLoading={submitting}
        destroyOnClose
        onOk={handleAddMarketplace}
        onCancel={() => {
          if (!submitting) {
            setAddMarketplaceModalOpen(false);
          }
        }}
      >
        <div className={styles.modalFieldRow}>
          <div className={styles.modalFieldLabel}>
            {t('claudecode.plugins.marketplaces.sourceLabel')}
          </div>
          <div className={styles.modalFieldControl}>
            <Input
              autoFocus
              value={marketplaceSourceInput}
              onChange={(event) => setMarketplaceSourceInput(event.target.value)}
              placeholder={t('claudecode.plugins.marketplaces.sourcePlaceholder')}
              onPressEnter={() => {
                void handleAddMarketplace();
              }}
            />
          </div>
        </div>
      </Modal>

      <JsonPreviewModal
        open={previewOpen}
        onClose={() => setPreviewOpen(false)}
        title={previewTitle}
        data={previewData}
      />
    </>
  );
};

export default ClaudePluginsPanel;
