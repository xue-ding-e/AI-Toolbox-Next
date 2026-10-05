import React from 'react';
import {
  Alert,
  Button,
  Collapse,
  Descriptions,
  Empty,
  Space,
  Spin,
  Table,
  Tag,
  Typography,
} from 'antd';
import {
  ApiOutlined,
  AppstoreOutlined,
  CloudServerOutlined,
  FolderOpenOutlined,
  InfoCircleOutlined,
  SettingOutlined,
  ToolOutlined,
  ThunderboltOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import SectionSidebarLayout from '@/components/layout/SectionSidebarLayout/SectionSidebarLayout';
import CliManualPathSetting from '@/components/common/CliManualPathSetting';
import { useSettingsStore } from '@/stores';
import { SessionManagerPanel } from '@/features/coding/shared/sessionManager';
import { readOmoNativeRuntimeConfig, listOmoNativeSkills } from '@/services/omoNativeApi';
import type { OmoNativeRuntimeConfig, OmoNativeSkill } from '@/types/omoNative';
import OmoNativeSettings from '../components/OmoNativeSettings';

const { Text, Link } = Typography;

const OMO_NATIVE_DOCS_URL =
  'https://github.com/code-yeongyu/oh-my-openagent/blob/dev/docs/reference/omo-json.md';

const SIDEBAR_ICON_BY_SECTION_ID: Record<string, React.ReactNode> = {
  'omo-native-model-settings': <ThunderboltOutlined />,
  'omo-native-agents': <AppstoreOutlined />,
  'omo-native-providers': <ApiOutlined />,
  'omo-native-mcp-skills': <ToolOutlined />,
  'omo-native-session-manager': <FolderOpenOutlined />,
  'omo-native-more-options': <SettingOutlined />,
};

const OmoNativePage: React.FC = () => {
  const { t } = useTranslation();
  const cliVersion = useSettingsStore((state) => state.cliManualPaths['omo']);

  const [runtimeConfig, setRuntimeConfig] = React.useState<OmoNativeRuntimeConfig | null>(null);
  const [skills, setSkills] = React.useState<OmoNativeSkill[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [loadError, setLoadError] = React.useState<string | null>(null);

  const loadConfig = React.useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const [config, skillList] = await Promise.all([
        readOmoNativeRuntimeConfig(),
        listOmoNativeSkills().catch(() => [] as OmoNativeSkill[]),
      ]);
      setRuntimeConfig(config);
      setSkills(skillList);
    } catch (error) {
      console.error('Failed to load OmO Native runtime config:', error);
      setLoadError((error as Error).message || String(error));
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  const sidebarSections = React.useMemo(
    () => [
      { id: 'omo-native-model-settings', title: t('omoNative.sections.modelSettings'), order: 1 },
      { id: 'omo-native-agents', title: t('omoNative.title'), order: 2 },
      { id: 'omo-native-providers', title: t('omoNative.sections.providers'), order: 3 },
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
      markerAttr="data-omo-native-sidebar-section"
      getIcon={(id: string) => SIDEBAR_ICON_BY_SECTION_ID[id] ?? null}
      sections={sidebarSections}
    >
      <div style={{ padding: 16 }}>
        {loadError && (
          <Alert
            type="error"
            showIcon
            style={{ marginBottom: 16 }}
            message={t('omoNative.loadError')}
            description={loadError}
          />
        )}

        {/* 生效视图 */}
        <div
          id="omo-native-model-settings"
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('omoNative.sections.modelSettings')}
          data-sidebar-order={1}
          style={{ marginBottom: 24 }}
        >
          <Collapse
            defaultActiveKey={['effective']}
            items={[
              {
                key: 'effective',
                label: (
                  <Space>
                    <Text strong>
                      <ThunderboltOutlined style={{ marginRight: 8 }} />
                      {t('omoNative.sections.modelSettings')}
                    </Text>
                    <Link
                      type="secondary"
                      style={{ fontSize: 12 }}
                      onClick={(event) => {
                        event.stopPropagation();
                        void openUrl(OMO_NATIVE_DOCS_URL);
                      }}
                    >
                      {t('omoNative.docs')}
                    </Link>
                  </Space>
                ),
                children: (
                  <div>
                    <Alert
                      type="info"
                      showIcon
                      icon={<InfoCircleOutlined />}
                      style={{ marginBottom: 12 }}
                      message={t('omoNative.effectiveHint')}
                    />
                    <Descriptions size="small" column={1} style={{ marginBottom: 12 }}>
                      <Descriptions.Item label={t('omoNative.configPath')}>
                        <Text code>{runtimeConfig?.configPath ?? '-'}</Text>
                      </Descriptions.Item>
                      <Descriptions.Item label={t('omoNative.rootPath')}>
                        <Text code>{cliVersion ?? '-'}</Text>
                      </Descriptions.Item>
                    </Descriptions>

                    {effectiveEntries.length === 0 ? (
                      <Empty
                        description={t('omoNative.emptyEffective')}
                        style={{ margin: '16px 0' }}
                      />
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
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('omoNative.title')}
          data-sidebar-order={2}
          style={{ marginBottom: 24 }}
        >
          <OmoNativeSettings onConfigUpdated={loadConfig} />
        </div>

        {/* Providers */}
        <div
          id="omo-native-providers"
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('omoNative.sections.providers')}
          data-sidebar-order={3}
          style={{ marginBottom: 24 }}
        >
          <Collapse
            items={[
              {
                key: 'providers',
                label: (
                  <Space>
                    <Text strong>
                      <ApiOutlined style={{ marginRight: 8 }} />
                      {t('omoNative.sections.providers')}
                    </Text>
                  </Space>
                ),
                children: (
                  <Alert
                    type="info"
                    showIcon
                    icon={<CloudServerOutlined />}
                    message={t('omoNative.providersPlaceholder')}
                    description={
                      <div>
                        <div>{t('omoNative.providersPlaceholderDesc')}</div>
                        <div style={{ marginTop: 8 }}>
                          <Text code>~/.omo/agent/models.json</Text>
                          <br />
                          <Text code>~/.omo/agent/auth.json</Text>
                        </div>
                        {runtimeConfig?.modelsContent && (
                          <pre
                            style={{
                              marginTop: 12,
                              maxHeight: 240,
                              overflow: 'auto',
                              fontSize: 12,
                            }}
                          >
                            {runtimeConfig.modelsContent}
                          </pre>
                        )}
                      </div>
                    }
                  />
                ),
              },
            ]}
          />
        </div>

        {/* MCP 与 Skills */}
        <div
          id="omo-native-mcp-skills"
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('omoNative.sections.mcpSkills')}
          data-sidebar-order={4}
          style={{ marginBottom: 24 }}
        >
          <Collapse
            items={[
              {
                key: 'mcp-skills',
                label: (
                  <Space>
                    <Text strong>
                      <ToolOutlined style={{ marginRight: 8 }} />
                      {t('omoNative.sections.mcpSkills')}
                    </Text>
                    <Tag>{t('omoNative.skillCount', { count: skills.length })}</Tag>
                  </Space>
                ),
                children:
                  skills.length === 0 ? (
                    <Empty description={t('omoNative.emptySkills')} style={{ margin: '16px 0' }} />
                  ) : (
                    <Table
                      size="small"
                      pagination={false}
                      rowKey="name"
                      dataSource={skills}
                      columns={[
                        { title: t('omoNative.skillName'), dataIndex: 'name', width: 200 },
                        {
                          title: t('omoNative.skillDescription'),
                          dataIndex: 'description',
                          render: (value: string | undefined) => value ?? '-',
                        },
                      ]}
                    />
                  ),
              },
            ]}
          />
        </div>

        {/* 会话管理 */}
        <div
          id="omo-native-session-manager"
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('sessionManager.title')}
          data-sidebar-order={5}
          style={{ marginBottom: 24 }}
        >
          <Collapse
            items={[
              {
                key: 'session-manager',
                label: (
                  <Space>
                    <Text strong>
                      <FolderOpenOutlined style={{ marginRight: 8 }} />
                      {t('sessionManager.title')}
                    </Text>
                  </Space>
                ),
                children: <SessionManagerPanel tool="omo_native" />,
              },
            ]}
          />
        </div>

        {/* 更多选项 */}
        <div
          id="omo-native-more-options"
          data-omo-native-sidebar-section="true"
          data-sidebar-title={t('omoNative.sections.moreOptions')}
          data-sidebar-order={6}
        >
          <Collapse
            items={[
              {
                key: 'more-options',
                label: (
                  <Space>
                    <Text strong>
                      <SettingOutlined style={{ marginRight: 8 }} />
                      {t('omoNative.sections.moreOptions')}
                    </Text>
                  </Space>
                ),
                children: (
                  <Space direction="vertical" style={{ width: '100%' }}>
                    <CliManualPathSetting
                      commandName="omo"
                      labelKey="subModules.omoNative"
                      toolNameKey="omoNative.fullName"
                    />
                    <Button
                      type="link"
                      style={{ paddingLeft: 0 }}
                      onClick={() => void loadConfig()}
                    >
                      {t('common.refresh')}
                    </Button>
                  </Space>
                ),
              },
            ]}
          />
        </div>
      </div>
    </SectionSidebarLayout>
  );
};

export default OmoNativePage;
