import React from 'react';
import { Alert, Button, Collapse, Descriptions, Empty, Space, Spin, Table, Tag, Typography } from 'antd';
import {
  ApiOutlined,
  AppstoreOutlined,
  EditOutlined,
  EllipsisOutlined,
  EyeOutlined,
  FolderOpenOutlined,
  InfoCircleOutlined,
  LinkOutlined,
  ReloadOutlined,
  SettingOutlined,
  ThunderboltOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import SectionSidebarLayout, {
  type SidebarSectionMarker,
} from '@/components/layout/SectionSidebarLayout/SectionSidebarLayout';
import SidebarSettingsModal from '@/components/common/SidebarSettingsModal';
import CliManualPathSetting from '@/components/common/CliManualPathSetting';
import FileConfigPreviewModal from '@/components/common/FileConfigPreviewModal';
import RootDirectoryModal from '@/features/coding/shared/RootDirectoryModal';
import useRootDirectoryConfig from '@/features/coding/shared/useRootDirectoryConfig';
import { SessionManagerPanel } from '@/features/coding/shared/sessionManager';
import { useSettingsStore } from '@/stores';
import { refreshTrayMenu } from '@/services/appApi';
import {
  getOmoNativeRootPathInfo,
  getOmoNativeSettingsConfig,
  readOmoNativeRuntimeConfig,
  saveOmoNativeSettingsConfig,
} from '@/services/omoNativeApi';
import type { OmoNativePathInfo, OmoNativeRuntimeConfig } from '@/types/omoNative';
import OmoNativeSettings from '../components/OmoNativeSettings';
import OmoNativeProvidersSection from '../components/OmoNativeProvidersSection';
import styles from './OmoNativePage.module.less';

const { Text, Link, Title } = Typography;

const OMO_NATIVE_DOCS_URL =
  'https://github.com/code-yeongyu/oh-my-openagent/blob/dev/docs/reference/omo-json.md';

const SIDEBAR_ICON_BY_SECTION_ID: Record<string, React.ReactNode> = {
  'omo-native-model-settings': <ThunderboltOutlined />,
  'omo-native-agents': <AppstoreOutlined />,
  'omo-native-providers': <ApiOutlined />,
  'omo-native-mcp-skills': <ApiOutlined />,
  'omo-native-session-manager': <FolderOpenOutlined />,
  'omo-native-more-options': <SettingOutlined />,
};

const OmoNativePage: React.FC = () => {
  const { t } = useTranslation();

  const [runtimeConfig, setRuntimeConfig] = React.useState<OmoNativeRuntimeConfig | null>(null);
  const [pathInfo, setPathInfo] = React.useState<OmoNativePathInfo | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [loadError, setLoadError] = React.useState<string | null>(null);
  const [previewModalOpen, setPreviewModalOpen] = React.useState(false);
  const [settingsModalOpen, setSettingsModalOpen] = React.useState(false);
  const { sidebarHiddenByPage, setSidebarHidden } = useSettingsStore();
  const sidebarHidden = sidebarHiddenByPage.omo_native;

  const loadConfig = React.useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
    setLoadError(null);
    try {
      const [config, rootPathInfo] = await Promise.all([
        readOmoNativeRuntimeConfig(),
        getOmoNativeRootPathInfo(),
      ]);
      setRuntimeConfig(config);
      setPathInfo(rootPathInfo);
    } catch (error) {
      console.error('Failed to load OmO Native runtime config:', error);
      setLoadError((error as Error).message || String(error));
    } finally {
      if (!silent) setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  const {
    rootDirectoryModalOpen,
    setRootDirectoryModalOpen,
    getRootDirectoryModalProps,
    handleSaveRootDirectory,
    handleResetRootDirectory,
  } = useRootDirectoryConfig({
    t,
    translationKeyPrefix: 'omoNative',
    defaultConfig: '{}',
    loadConfig,
    getCommonConfig: getOmoNativeSettingsConfig,
    saveCommonConfig: async (input) => {
      await saveOmoNativeSettingsConfig(input);
      await refreshTrayMenu();
    },
  });

  const sidebarSections = React.useMemo<SidebarSectionMarker[]>(
    () => [
      { id: 'omo-native-model-settings', title: t('omoNative.sections.modelSettings'), order: 1 },
      { id: 'omo-native-agents', title: t('omoNative.title'), order: 2 },
      { id: 'omo-native-providers', title: t('omoNative.providers.title'), order: 3 },
      { id: 'omo-native-mcp-skills', title: t('omoNative.sections.mcpSkills'), order: 4 },
      { id: 'omo-native-session-manager', title: t('sessionManager.title'), order: 5 },
      { id: 'omo-native-more-options', title: t('omoNative.sections.moreOptions'), order: 6 },
    ],
    [t],
  );

  const effectiveEntries = React.useMemo(() => {
    if (!runtimeConfig?.effective) return [];
    return Object.entries(runtimeConfig.effective).sort(([a], [b]) => a.localeCompare(b));
  }, [runtimeConfig]);

  const handleOpenRootFolder = async () => {
    if (!pathInfo?.path) return;
    try {
      const { openPath } = await import('@tauri-apps/plugin-opener');
      await openPath(pathInfo.path);
    } catch (error) {
      console.error('Failed to open root folder:', error);
    }
  };

  if (loading && !runtimeConfig) {
    return (
      <div style={{ padding: 48, textAlign: 'center' }}>
        <Spin />
      </div>
    );
  }

  return (
    <SectionSidebarLayout
      sidebarTitle={t('omoNative.title')}
      sidebarHidden={sidebarHidden}
      markerAttr="data-omo-native-sidebar-section"
      getIcon={(id: string) => SIDEBAR_ICON_BY_SECTION_ID[id] ?? null}
      sections={sidebarSections}
    >
      <div className={styles.pageContent}>
        {/* 页头：与 OMP / OpenCode 同构 */}
        <div className={styles.pageHeader}>
          <div>
            <div className={styles.titleRow}>
              <Title level={4} className={styles.pageTitle}>
                {t('omoNative.title')}
              </Title>
              <Link
                type="secondary"
                className={styles.headerLink}
                onClick={(event) => {
                  event.stopPropagation();
                  void openUrl(OMO_NATIVE_DOCS_URL);
                }}
              >
                <LinkOutlined /> {t('omoNative.docs')}
              </Link>
              <Link
                type="secondary"
                className={styles.headerLink}
                onClick={(event) => {
                  event.stopPropagation();
                  setPreviewModalOpen(true);
                }}
              >
                <EyeOutlined /> {t('common.previewConfig')}
              </Link>
            </div>
            <Space className={styles.pathToolbar} wrap>
              <Text type="secondary" className={styles.pathLabel}>
                {t('omoNative.configPath')}:
              </Text>
              <Text code className={styles.pathText}>
                {runtimeConfig?.configPath ?? '-'}
              </Text>
              <Button
                type="text"
                size="small"
                icon={<EditOutlined />}
                onClick={() => setRootDirectoryModalOpen(true)}
                className={styles.textAction}
              >
                {t('omoNative.rootPathSource.customize')}
              </Button>
              <Button
                type="text"
                size="small"
                icon={<FolderOpenOutlined />}
                onClick={handleOpenRootFolder}
                className={styles.textAction}
              >
                {t('omoNative.openFolder')}
              </Button>
              <Button
                type="text"
                size="small"
                icon={<ReloadOutlined />}
                onClick={() => void loadConfig()}
                className={styles.textAction}
              >
                {t('omoNative.refreshConfig')}
              </Button>
            </Space>
          </div>
          <Button
            type="text"
            icon={<EllipsisOutlined />}
            onClick={() => setSettingsModalOpen(true)}
          >
            {t('common.moreOptions')}
          </Button>
        </div>
        <div className={styles.pageHint}>{t('omoNative.pageHint')}</div>

        {loadError && (
          <Alert type="error" showIcon message={t('omoNative.loadError')} description={loadError} />
        )}

        {/* 生效配置 */}
        <div
          id="omo-native-model-settings"
          className={styles.omoSection}
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('omoNative.sections.modelSettings')}
        >
          <Collapse
            className={styles.collapseCard}
            defaultActiveKey={['effective']}
            items={[
              {
                key: 'effective',
                label: (
                  <Space>
                    <ThunderboltOutlined />
                    <Text strong>{t('omoNative.sections.modelSettings')}</Text>
                  </Space>
                ),
                children: (
                  <div>
                    <div className={styles.sectionHint}>{t('omoNative.effectiveHint')}</div>
                    <Descriptions size="small" column={1} style={{ marginBottom: 12 }}>
                      <Descriptions.Item label={t('omoNative.configPath')}>
                        <Text code>{runtimeConfig?.configPath ?? '-'}</Text>
                      </Descriptions.Item>
                      <Descriptions.Item label={t('omoNative.rootPath')}>
                        <Text code>{pathInfo?.path ?? '-'}</Text>
                      </Descriptions.Item>
                    </Descriptions>

                    {effectiveEntries.length === 0 ? (
                      <Empty description={t('omoNative.emptyEffective')} />
                    ) : (
                      <Table
                        size="small"
                        pagination={false}
                        rowKey="key"
                        dataSource={effectiveEntries.map(([key, value]) => ({ key, value }))}
                        columns={[
                          {
                            title: t('omoNative.keyColumn'),
                            dataIndex: 'key',
                            width: 200,
                            render: (key: string) => <Text code>{key}</Text>,
                          },
                          {
                            title: t('omoNative.sourceColumn'),
                            dataIndex: 'key',
                            width: 120,
                            render: (key: string) => {
                              const inNative = runtimeConfig?.nativeBlock?.[key] !== undefined;
                              return inNative ? (
                                <Tag color="processing">[native]</Tag>
                              ) : (
                                <Tag>{t('omoNative.sharedBase')}</Tag>
                              );
                            },
                          },
                          {
                            title: t('omoNative.valueColumn'),
                            dataIndex: 'value',
                            render: (value: unknown) => (
                              <Text code style={{ fontSize: 12 }}>
                                {JSON.stringify(value)}
                              </Text>
                            ),
                          },
                        ]}
                      />
                    )}
                  </div>
                ),
              },
            ]}
          />
        </div>

        {/* Agent/Category 方案 */}
        <div
          id="omo-native-agents"
          className={styles.omoSection}
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('omoNative.title')}
        >
          <OmoNativeSettings onConfigUpdated={() => void loadConfig(true)} />
        </div>

        {/* Providers */}
        <div
          id="omo-native-providers"
          className={styles.omoSection}
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('omoNative.providers.title')}
        >
          <OmoNativeProvidersSection />
        </div>

        {/* MCP 与 Skills：与其他 tab 一致，走顶部的独立页面 */}
        <div
          id="omo-native-mcp-skills"
          className={styles.omoSection}
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('omoNative.sections.mcpSkills')}
        >
          <Collapse
            className={styles.collapseCard}
            items={[
              {
                key: 'mcp-skills',
                label: (
                  <Space>
                    <ApiOutlined />
                    <Text strong>{t('omoNative.sections.mcpSkills')}</Text>
                  </Space>
                ),
                children: (
                  <Alert
                    type="info"
                    showIcon
                    icon={<InfoCircleOutlined />}
                    message={t('omoNative.mcpSkillsHint')}
                    description={
                      <div>
                        <div>{t('omoNative.mcpSkillsHintDesc')}</div>
                        <Space style={{ marginTop: 8 }} wrap>
                          <Text code>~/.omo/agent/mcp.json</Text>
                          <Text code>~/.omo/agent/skills/</Text>
                        </Space>
                      </div>
                    }
                  />
                ),
              },
            ]}
          />
        </div>

        {/* 会话管理 */}
        <div
          id="omo-native-session-manager"
          className={styles.omoSection}
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('sessionManager.title')}
        >
          <SessionManagerPanel tool="omo_native" />
        </div>

        {/* 更多选项 */}
        <div
          id="omo-native-more-options"
          className={styles.omoSection}
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('omoNative.sections.moreOptions')}
        >
          <Collapse
            className={styles.collapseCard}
            items={[
              {
                key: 'more-options',
                label: (
                  <Space>
                    <SettingOutlined />
                    <Text strong>{t('omoNative.sections.moreOptions')}</Text>
                  </Space>
                ),
                children: (
                  <Space direction="vertical" style={{ width: '100%' }}>
                    <CliManualPathSetting
                      commandName="omo"
                      labelKey="subModules.omoNative"
                      toolNameKey="omoNative.fullName"
                    />
                    <Button type="link" style={{ paddingLeft: 0 }} onClick={() => void loadConfig()}>
                      {t('common.refresh')}
                    </Button>
                  </Space>
                ),
              },
            ]}
          />
        </div>
      </div>

      <RootDirectoryModal
        open={rootDirectoryModalOpen}
        {...getRootDirectoryModalProps(pathInfo)}
        onCancel={() => setRootDirectoryModalOpen(false)}
        onSubmit={handleSaveRootDirectory}
        onReset={handleResetRootDirectory}
      />

      <FileConfigPreviewModal
        open={previewModalOpen}
        onClose={() => setPreviewModalOpen(false)}
        title={t('omoNative.preview.title')}
        files={[
          {
            key: 'config',
            label: runtimeConfig?.configPath?.split(/[\\/]/).pop() || 'omo.jsonc',
            content: runtimeConfig?.nativeBlock
              ? JSON.stringify(runtimeConfig.nativeBlock, null, 2)
              : undefined,
            language: 'json',
          },
          {
            key: 'settings',
            label: 'settings.json',
            content: runtimeConfig?.settingsContent,
            language: 'json',
          },
          {
            key: 'models',
            label: 'models.json',
            content: runtimeConfig?.modelsContent,
            language: 'json',
          },
          {
            key: 'mcp',
            label: 'mcp.json',
            content: runtimeConfig?.mcpContent,
            language: 'json',
          },
        ]}
      />

      <SidebarSettingsModal
        open={settingsModalOpen}
        onClose={() => setSettingsModalOpen(false)}
        sidebarVisible={!sidebarHidden}
        onSidebarVisibleChange={async (visible) => {
          await setSidebarHidden('omo_native', !visible);
        }}
      >
        <CliManualPathSetting
          commandName="omo"
          labelKey="subModules.omoNative"
          toolNameKey="omoNative.fullName"
        />
      </SidebarSettingsModal>
    </SectionSidebarLayout>
  );
};

export default OmoNativePage;
