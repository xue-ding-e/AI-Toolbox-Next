import React from 'react';
import { Modal, Button, Checkbox, message, Form, Input, Space, Tooltip, Switch, Radio } from 'antd';
import { ClearOutlined, DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { McpServer, McpTool, StdioConfig, HttpConfig } from '../../types';
import * as mcpApi from '../../services/mcpApi';
import { useMcpStore } from '../../stores/mcpStore';
import { refreshTrayMenu } from '@/services/appApi';
import styles from './McpSettingsModal.module.less';

interface McpSettingsModalProps {
  open: boolean;
  onClose: () => void;
}

interface CustomMcpTool {
  key: string;
  display_name: string;
  mcp_config_path: string | null;
  mcp_config_format: string | null;
  mcp_field: string | null;
}

export const McpSettingsModal: React.FC<McpSettingsModalProps> = ({
  open: isOpen,
  onClose,
}) => {
  const { t } = useTranslation();
  const { fetchTools, servers, fetchServers } = useMcpStore();
  const [form] = Form.useForm();
  const [loading, setLoading] = React.useState(false);
  const [allTools, setAllTools] = React.useState<McpTool[]>([]);
  const [preferredTools, setPreferredTools] = React.useState<string[]>([]);
  const [customTools, setCustomTools] = React.useState<CustomMcpTool[]>([]);
  const [showAddCustomModal, setShowAddCustomModal] = React.useState(false);
  const [addingTool, setAddingTool] = React.useState(false);
  const [showInTray, setShowInTray] = React.useState(false);
  const [showClearAllModal, setShowClearAllModal] = React.useState(false);
  const [clearAllConfirmText, setClearAllConfirmText] = React.useState('');
  const [clearingAll, setClearingAll] = React.useState(false);

  // Load settings on mount
  React.useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      const [tools, trayEnabled, savedPreferredTools] = await Promise.all([
        mcpApi.getMcpTools(),
        mcpApi.getMcpShowInTray(),
        mcpApi.getMcpPreferredTools(),
      ]);

      // Sort: installed tools first
      const sorted = [...tools].sort((a, b) => {
        if (a.installed === b.installed) return 0;
        return a.installed ? -1 : 1;
      });
      setAllTools(sorted);
      setShowInTray(trayEnabled);

      // Extract custom tools
      const custom = tools.filter((t) => t.is_custom && t.supports_mcp);
      setCustomTools(
        custom.map((t) => ({
          key: t.key,
          display_name: t.display_name,
          mcp_config_path: t.mcp_config_path,
          mcp_config_format: t.mcp_config_format,
          mcp_field: t.mcp_field,
        }))
      );

      // Use saved preferred tools if available, otherwise default to installed tools
      if (savedPreferredTools.length > 0) {
        setPreferredTools(savedPreferredTools);
      } else {
        setPreferredTools(tools.filter((t) => t.installed && t.supports_mcp).map((t) => t.key));
      }
    } catch (error) {
      console.error('Failed to load settings:', error);
    }
  };

  const handleToolToggle = (toolKey: string, checked: boolean) => {
    setPreferredTools((prev) =>
      checked ? [...prev, toolKey] : prev.filter((k) => k !== toolKey)
    );
  };

  const handleShowInTrayChange = async (checked: boolean) => {
    setShowInTray(checked);
    try {
      await mcpApi.setMcpShowInTray(checked);
      await refreshTrayMenu();
    } catch (error) {
      message.error(String(error));
      setShowInTray(!checked); // Revert on error
    }
  };

  // Sort tools: installed built-in > custom tools > not installed built-in
  const sortedTools = React.useMemo(() => {
    const customKeys = new Set(customTools.map((c) => c.key));
    const installedBuiltin = allTools.filter((t) => t.installed && !customKeys.has(t.key) && t.supports_mcp);
    const customToolItems = allTools.filter((t) => customKeys.has(t.key) && t.supports_mcp);
    const notInstalledBuiltin = allTools.filter((t) => !t.installed && !customKeys.has(t.key) && t.supports_mcp);
    return [...installedBuiltin, ...customToolItems, ...notInstalledBuiltin];
  }, [allTools, customTools]);

  const handleSave = async () => {
    setLoading(true);
    try {
      // Save preferred tools
      await mcpApi.setMcpPreferredTools(preferredTools);
      await fetchTools(); // Refresh global store
      message.success(t('common.success'));
      onClose();
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  };

  const handleAddCustomTool = async (values: {
    key: string;
    displayName: string;
    mcpConfigPath: string;
    mcpConfigFormat: 'json' | 'toml';
    mcpField: string;
  }) => {
    setAddingTool(true);
    try {
      await mcpApi.addMcpCustomTool({
        key: values.key,
        displayName: values.displayName,
        mcpConfigPath: values.mcpConfigPath,
        mcpConfigFormat: values.mcpConfigFormat,
        mcpField: values.mcpField,
      });
      message.success(t('common.success'));
      form.resetFields();
      setShowAddCustomModal(false);
      await loadData();
      await fetchTools();
    } catch (error) {
      message.error(String(error));
    } finally {
      setAddingTool(false);
    }
  };

  const handleRemoveCustomTool = async (key: string) => {
    try {
      await mcpApi.removeMcpCustomTool(key);
      message.success(t('common.success'));
      await loadData();
      await fetchTools();
    } catch (error) {
      message.error(String(error));
    }
  };

  const getDuplicateServers = (list: McpServer[]): McpServer[] => {
    const groups = new Map<string, McpServer[]>();
    for (const server of list) {
      let key: string;
      if (server.server_type === 'stdio') {
        const config = server.server_config as StdioConfig;
        key = `stdio:${config.command}:${JSON.stringify([...(config.args || [])].sort())}`;
      } else {
        const config = server.server_config as HttpConfig;
        key = `${server.server_type}:${config.url}`;
      }
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(server);
    }

    const duplicates: McpServer[] = [];
    for (const group of groups.values()) {
      if (group.length > 1) {
        group.sort((a, b) => a.created_at - b.created_at);
        duplicates.push(...group.slice(1));
      }
    }
    return duplicates;
  };

  const duplicateServers = React.useMemo(() => getDuplicateServers(servers), [servers]);

  const expectedConfirmText = t('mcp.clearAll.confirmText');

  const handleClearAllServers = async () => {
    if (clearAllConfirmText !== expectedConfirmText) {
      message.error(t('mcp.clearAll.confirmMismatch'));
      return;
    }
    setClearingAll(true);
    try {
      for (const server of servers) {
        await mcpApi.deleteMcpServer(server.id);
      }
      await fetchServers();
      message.success(t('mcp.clearAll.success'));
      setShowClearAllModal(false);
      setClearAllConfirmText('');
    } catch (error) {
      message.error(String(error));
    } finally {
      setClearingAll(false);
    }
  };

  const handleClearDuplicates = () => {
    if (duplicateServers.length === 0) {
      message.info(t('mcp.clearDuplicates.noDuplicates'));
      return;
    }
    Modal.confirm({
      title: t('mcp.clearDuplicates.modalTitle'),
      content: t('mcp.clearDuplicates.modalMessage', { count: duplicateServers.length }),
      okText: t('mcp.clearDuplicates.confirm'),
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          for (const server of duplicateServers) {
            await mcpApi.deleteMcpServer(server.id);
          }
          await fetchServers();
          message.success(t('mcp.clearDuplicates.success', { count: duplicateServers.length }));
        } catch (error) {
          message.error(String(error));
        }
      },
    });
  };

  return (
    <Modal
      title={t('mcp.settings')}
      open={isOpen}
      onCancel={onClose}
      footer={null}
      width={700}
    >
      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('mcp.showInTray')}</label>
        </div>
        <div className={styles.inputArea}>
          <Switch checked={showInTray} onChange={handleShowInTrayChange} />
          <p className={styles.hint}>{t('mcp.showInTrayHint')}</p>
        </div>
      </div>

      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('mcp.preferredTools')}</label>
        </div>
        <div className={styles.inputArea}>
          <div className={styles.toolList}>
            {sortedTools.map((tool) => {
              const isCustomTool = customTools.some((c) => c.key === tool.key);
              const isDisabled = !tool.installed && !isCustomTool;
              return (
                <div key={tool.key} className={styles.toolItem}>
                  <Tooltip title={tool.mcp_config_path || ''}>
                    <Checkbox
                      checked={preferredTools.includes(tool.key)}
                      onChange={(e) => handleToolToggle(tool.key, e.target.checked)}
                      disabled={isDisabled}
                    >
                      {tool.display_name}
                    </Checkbox>
                  </Tooltip>
                  {isCustomTool && (
                    <Button
                      type="text"
                      size="small"
                      icon={<DeleteOutlined />}
                      danger
                      onClick={() => handleRemoveCustomTool(tool.key)}
                    />
                  )}
                </div>
              );
            })}
            <Button
              type="dashed"
              size="small"
              icon={<PlusOutlined />}
              onClick={() => setShowAddCustomModal(true)}
            >
              {t('mcp.customToolSettings.add')}
            </Button>
          </div>
          <p className={styles.hint}>{t('mcp.preferredToolsHint')}</p>
        </div>
      </div>

      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('mcp.clearAll.title')}</label>
        </div>
        <div className={styles.inputArea}>
          <Space>
            <Button
              danger
              icon={<ClearOutlined />}
              onClick={() => setShowClearAllModal(true)}
              disabled={servers.length === 0}
            >
              {t('mcp.clearAll.button')}
            </Button>
            <Button
              danger
              icon={<DeleteOutlined />}
              onClick={handleClearDuplicates}
              disabled={duplicateServers.length === 0}
            >
              {t('mcp.clearDuplicates.button')}
            </Button>
          </Space>
          <p className={styles.hint}>{t('mcp.clearAll.hint')}</p>
        </div>
      </div>

      <div className={styles.footer}>
        <Button onClick={onClose}>{t('common.cancel')}</Button>
        <Button type="primary" onClick={handleSave} loading={loading}>
          {t('common.save')}
        </Button>
      </div>

      {showAddCustomModal && (
        <Modal
          title={t('mcp.customToolSettings.addTitle')}
          open={showAddCustomModal}
          onCancel={() => setShowAddCustomModal(false)}
          footer={null}
        >
        <Form form={form} layout="vertical" onFinish={handleAddCustomTool}>
          <Form.Item
            name="key"
            label={t('mcp.customToolSettings.key')}
            rules={[
              { required: true, message: t('mcp.customToolSettings.keyRequired') },
              { pattern: /^[a-z][a-z0-9_]*$/, message: t('mcp.customToolSettings.keyHint') },
            ]}
          >
            <Input placeholder="my_tool" />
          </Form.Item>
          <Form.Item
            name="displayName"
            label={t('mcp.customToolSettings.displayName')}
            rules={[{ required: true, message: t('mcp.customToolSettings.displayNameRequired') }]}
          >
            <Input placeholder="My Tool" />
          </Form.Item>
          <Form.Item
            name="mcpConfigPath"
            label={t('mcp.customToolSettings.configPath')}
            rules={[{ required: true, message: t('mcp.customToolSettings.configPathRequired') }]}
            extra={t('mcp.customToolSettings.configPathHint')}
          >
            <Input placeholder="~/.mytool/mcp.json" />
          </Form.Item>
          <Form.Item
            name="mcpConfigFormat"
            label={t('mcp.customToolSettings.configFormat')}
            rules={[{ required: true }]}
            initialValue="json"
          >
            <Radio.Group
              options={[
                { label: 'JSON', value: 'json' },
                { label: 'TOML', value: 'toml' },
              ]}
            />
          </Form.Item>
          <Form.Item
            name="mcpField"
            label={t('mcp.customToolSettings.configField')}
            rules={[{ required: true, message: t('mcp.customToolSettings.configFieldRequired') }]}
          >
            <Input placeholder="mcpServers" />
          </Form.Item>
          <div style={{ textAlign: 'right' }}>
            <Space>
              <Button onClick={() => setShowAddCustomModal(false)}>{t('common.cancel')}</Button>
              <Button type="primary" htmlType="submit" loading={addingTool}>
                {t('common.add')}
              </Button>
            </Space>
          </div>
        </Form>
      </Modal>
      )}

      {showClearAllModal && (
        <Modal
          title={t('mcp.clearAll.modalTitle')}
          open={showClearAllModal}
          onCancel={() => {
            setShowClearAllModal(false);
            setClearAllConfirmText('');
          }}
          footer={null}
          width={450}
        >
          <div style={{ marginBottom: 16 }}>
            <p>{t('mcp.clearAll.modalMessage', { count: servers.length })}</p>
            <p style={{ color: '#ff4d4f', fontWeight: 500 }}>
              {t('mcp.clearAll.modalWarning')}
            </p>
          </div>
          <div style={{ marginBottom: 16 }}>
            <p style={{ marginBottom: 8 }}>
              {t('mcp.clearAll.inputPrompt', { text: expectedConfirmText })}
            </p>
            <Input
              value={clearAllConfirmText}
              onChange={(e) => setClearAllConfirmText(e.target.value)}
              placeholder={expectedConfirmText}
            />
          </div>
          <div style={{ textAlign: 'right' }}>
            <Space>
              <Button onClick={() => {
                setShowClearAllModal(false);
                setClearAllConfirmText('');
              }}>
                {t('common.cancel')}
              </Button>
              <Button
                type="primary"
                danger
                onClick={handleClearAllServers}
                loading={clearingAll}
                disabled={clearAllConfirmText !== expectedConfirmText}
              >
                {t('mcp.clearAll.confirm')}
              </Button>
            </Space>
          </div>
        </Modal>
      )}
    </Modal>
  );
};
