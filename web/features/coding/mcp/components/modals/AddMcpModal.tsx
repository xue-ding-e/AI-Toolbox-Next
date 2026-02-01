import React, { useState, useEffect, useMemo } from 'react';
import { Modal, Form, Input, Select, Button, Space, Checkbox, Dropdown, Tag } from 'antd';
import { PlusOutlined, MinusCircleOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import * as mcpApi from '../../services/mcpApi';
import type { CreateMcpServerInput, UpdateMcpServerInput, McpTool, McpServer, StdioConfig, HttpConfig } from '../../types';
import styles from './AddMcpModal.module.less';

interface AddMcpModalProps {
  open: boolean;
  tools: McpTool[];
  editingServer?: McpServer | null;
  onClose: () => void;
  onSubmit: (input: CreateMcpServerInput) => Promise<void>;
  onUpdate?: (serverId: string, input: UpdateMcpServerInput) => Promise<void>;
}

export const AddMcpModal: React.FC<AddMcpModalProps> = ({
  open,
  tools,
  editingServer,
  onClose,
  onSubmit,
  onUpdate,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const [serverType, setServerType] = useState<'stdio' | 'http' | 'sse'>('stdio');
  const [selectedTools, setSelectedTools] = useState<string[]>([]);
  const [favorites, setFavorites] = useState<mcpApi.FavoriteMcp[]>([]);
  const [favoritesExpanded, setFavoritesExpanded] = useState(false);

  const isEditMode = !!editingServer;

  // Installed tools (memoized to avoid useEffect loops)
  const installedTools = useMemo(() => tools.filter((t) => t.installed), [tools]);
  // Uninstalled tools
  const uninstalledTools = useMemo(() => tools.filter((t) => !t.installed), [tools]);

  // Load favorites when modal opens
  useEffect(() => {
    if (open) {
      loadFavorites();
    } else {
      setFavoritesExpanded(false);
    }
  }, [open]);

  const loadFavorites = async () => {
    try {
      // Initialize default favorites if empty
      await mcpApi.initMcpDefaultFavorites();
      // Then load the list
      const list = await mcpApi.listMcpFavorites();
      setFavorites(list);
    } catch (error) {
      console.error('Failed to load favorites:', error);
    }
  };

  // Initialize form when editing or adding
  useEffect(() => {
    if (open && editingServer) {
      const config = editingServer.server_config;
      setServerType(editingServer.server_type);
      setSelectedTools(editingServer.enabled_tools);

      if (editingServer.server_type === 'stdio') {
        const stdioConfig = config as StdioConfig;
        // Convert env object to key-value array
        const envList = stdioConfig.env
          ? Object.entries(stdioConfig.env).map(([key, value]) => ({ key, value }))
          : [];
        form.setFieldsValue({
          name: editingServer.name,
          server_type: editingServer.server_type,
          command: stdioConfig.command,
          args: stdioConfig.args || [],
          env: envList,
          description: editingServer.description,
        });
      } else {
        const httpConfig = config as HttpConfig;
        // Extract Bearer Token from headers if present
        let bearerToken = '';
        const headersList: { key: string; value: string }[] = [];
        if (httpConfig.headers) {
          Object.entries(httpConfig.headers).forEach(([key, value]) => {
            if (key.toLowerCase() === 'authorization' && typeof value === 'string' && value.startsWith('Bearer ')) {
              bearerToken = value.substring(7); // Remove "Bearer " prefix
            } else {
              headersList.push({ key, value: String(value) });
            }
          });
        }
        form.setFieldsValue({
          name: editingServer.name,
          server_type: editingServer.server_type,
          url: httpConfig.url,
          bearerToken,
          headers: headersList,
          description: editingServer.description,
        });
      }
    } else if (open) {
      // Reset for add mode
      form.resetFields();
      setServerType('stdio');
      // Load preferred tools, fallback to all installed tools
      mcpApi.getMcpPreferredTools().then((preferred) => {
        if (preferred.length > 0) {
          setSelectedTools(preferred);
        } else {
          setSelectedTools(installedTools.map((t) => t.key));
        }
      }).catch(() => {
        setSelectedTools(installedTools.map((t) => t.key));
      });
    }
  }, [open, editingServer, form, installedTools]);

  const handleToolToggle = (toolKey: string) => {
    setSelectedTools((prev) =>
      prev.includes(toolKey)
        ? prev.filter((k) => k !== toolKey)
        : [...prev, toolKey]
    );
  };

  // Handle selecting a favorite MCP
  const handleSelectFavorite = (fav: mcpApi.FavoriteMcp) => {
    setServerType(fav.server_type);
    if (fav.server_type === 'stdio') {
      const config = fav.server_config as { command?: string; args?: string[]; env?: Record<string, string> };
      const envList = config.env
        ? Object.entries(config.env).map(([key, value]) => ({ key, value }))
        : [];
      form.setFieldsValue({
        name: fav.name,
        server_type: fav.server_type,
        command: config.command,
        args: config.args || [],
        env: envList,
        description: fav.description,
      });
    } else {
      const config = fav.server_config as { url?: string; headers?: Record<string, string> };
      const headersList = config.headers
        ? Object.entries(config.headers).filter(([key]) => key.toLowerCase() !== 'authorization').map(([key, value]) => ({ key, value }))
        : [];
      const bearerToken = config.headers?.['Authorization']?.replace('Bearer ', '') || '';
      form.setFieldsValue({
        name: fav.name,
        server_type: fav.server_type,
        url: config.url,
        bearerToken,
        headers: headersList,
        description: fav.description,
      });
    }
    setFavoritesExpanded(false);
  };

  // Handle removing a favorite MCP
  const handleRemoveFavorite = (fav: mcpApi.FavoriteMcp) => {
    Modal.confirm({
      title: t('mcp.favorites.removeTitle'),
      content: t('mcp.favorites.removeConfirm', { name: fav.name }),
      okText: t('common.confirm'),
      cancelText: t('common.cancel'),
      onOk: async () => {
        await mcpApi.deleteMcpFavorite(fav.id);
        setFavorites((prev) => prev.filter((f) => f.id !== fav.id));
      },
    });
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();

      setLoading(true);

      let serverConfig: StdioConfig | HttpConfig;
      if (serverType === 'stdio') {
        // Convert env key-value array to object
        const envObj: Record<string, string> = {};
        if (values.env && Array.isArray(values.env)) {
          values.env.forEach((item: { key?: string; value?: string }) => {
            if (item.key && item.key.trim()) {
              envObj[item.key.trim()] = item.value || '';
            }
          });
        }
        serverConfig = {
          command: values.command,
          args: values.args?.filter((a: string) => a) || [],
          env: Object.keys(envObj).length > 0 ? envObj : undefined,
        };
      } else {
        // Convert headers key-value array to object and merge Bearer Token
        const headersObj: Record<string, string> = {};
        if (values.headers && Array.isArray(values.headers)) {
          values.headers.forEach((item: { key?: string; value?: string }) => {
            if (item.key && item.key.trim()) {
              headersObj[item.key.trim()] = item.value || '';
            }
          });
        }
        // Add Bearer Token to headers if provided
        if (values.bearerToken && values.bearerToken.trim()) {
          headersObj['Authorization'] = `Bearer ${values.bearerToken.trim()}`;
        }
        serverConfig = {
          url: values.url,
          headers: Object.keys(headersObj).length > 0 ? headersObj : undefined,
        };
      }

      if (isEditMode && onUpdate && editingServer) {
        await onUpdate(editingServer.id, {
          name: values.name,
          server_type: serverType,
          server_config: serverConfig,
          enabled_tools: selectedTools,
          description: values.description,
        });
        // Update favorite (upsert by name)
        await mcpApi.upsertMcpFavorite({
          name: values.name,
          server_type: serverType,
          server_config: serverConfig as unknown as Record<string, unknown>,
          description: values.description,
          tags: values.tags?.filter((t: string) => t) || [],
        });
      } else {
        await onSubmit({
          name: values.name,
          server_type: serverType,
          server_config: serverConfig,
          enabled_tools: selectedTools,
          description: values.description,
          tags: values.tags?.filter((t: string) => t) || [],
        });
        // Add to favorites (upsert by name)
        await mcpApi.upsertMcpFavorite({
          name: values.name,
          server_type: serverType,
          server_config: serverConfig as unknown as Record<string, unknown>,
          description: values.description,
          tags: values.tags?.filter((t: string) => t) || [],
        });
      }

      form.resetFields();
      setSelectedTools([]);
      onClose();
    } catch (error) {
      console.error('Form validation failed:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    form.resetFields();
    setSelectedTools([]);
    onClose();
  };

  return (
    <Modal
      title={isEditMode ? t('mcp.editServer') : t('mcp.addServer')}
      open={open}
      onCancel={handleCancel}
      footer={[
        <Button key="cancel" onClick={handleCancel}>
          {t('common.cancel')}
        </Button>,
        <Button key="submit" type="primary" loading={loading} onClick={handleSubmit}>
          {t('common.save')}
        </Button>,
      ]}
      width={700}
      destroyOnClose
    >
      <Form
        form={form}
        layout="horizontal"
        labelCol={{ span: 6 }}
        wrapperCol={{ span: 18 }}
        initialValues={{ server_type: 'stdio' }}
      >
        <Form.Item
          label={t('mcp.name')}
          required
        >
          <div className={styles.nameRow}>
            <Form.Item
              name="name"
              noStyle
              rules={[{ required: true, message: t('mcp.nameRequired') }]}
            >
              <Input placeholder={t('mcp.namePlaceholder')} />
            </Form.Item>
            {favorites.length > 0 && (
              <a
                className={styles.favoritesToggle}
                onClick={() => setFavoritesExpanded(!favoritesExpanded)}
              >
                {t('mcp.favorites.label')}
                {favoritesExpanded ? ' ▴' : ' ▾'}
              </a>
            )}
          </div>
        </Form.Item>

        {favoritesExpanded && (
          <Form.Item wrapperCol={{ offset: 6, span: 18 }} style={{ marginTop: -8 }}>
            <div className={styles.favoritesTagsList}>
              {favorites.map((fav) => (
                <Tag
                  key={fav.id}
                  closable
                  className={styles.favoriteTag}
                  onClick={() => handleSelectFavorite(fav)}
                  onClose={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    handleRemoveFavorite(fav);
                  }}
                >
                  {fav.name}
                </Tag>
              ))}
            </div>
          </Form.Item>
        )}

        <Form.Item label={t('mcp.type')} name="server_type">
          <Select
            value={serverType}
            onChange={(v) => setServerType(v)}
            options={[
              { label: 'Stdio', value: 'stdio' },
              { label: 'HTTP', value: 'http' },
              { label: 'SSE', value: 'sse' },
            ]}
          />
        </Form.Item>

        {serverType === 'stdio' ? (
          <>
            <Form.Item
              label={t('mcp.command')}
              name="command"
              rules={[{ required: true, message: t('mcp.commandRequired') }]}
            >
              <Input placeholder="npx -y @modelcontextprotocol/server-xxx" />
            </Form.Item>

            <Form.Item label={t('mcp.args')}>
              <Form.List name="args">
                {(fields, { add, remove }) => (
                  <>
                    {fields.map((field, index) => (
                      <Space key={field.key} className={styles.argRow}>
                        <Form.Item {...field} noStyle>
                          <Input placeholder={`${t('mcp.arg')} ${index + 1}`} />
                        </Form.Item>
                        <MinusCircleOutlined onClick={() => remove(field.name)} />
                      </Space>
                    ))}
                    <Button type="dashed" onClick={() => add()} block icon={<PlusOutlined />}>
                      {t('mcp.addArg')}
                    </Button>
                  </>
                )}
              </Form.List>
            </Form.Item>

            <Form.Item label={t('mcp.env')}>
              <Form.List name="env">
                {(fields, { add, remove }) => (
                  <>
                    {fields.map((field) => (
                      <div key={field.key} className={styles.kvRow}>
                        <Form.Item
                          {...field}
                          name={[field.name, 'key']}
                          noStyle
                        >
                          <Input placeholder={t('mcp.envKey')} className={styles.kvKey} />
                        </Form.Item>
                        <Form.Item
                          {...field}
                          name={[field.name, 'value']}
                          noStyle
                        >
                          <Input placeholder={t('mcp.envValue')} className={styles.kvValue} />
                        </Form.Item>
                        <MinusCircleOutlined onClick={() => remove(field.name)} />
                      </div>
                    ))}
                    <Button type="dashed" onClick={() => add()} block icon={<PlusOutlined />}>
                      {t('mcp.addEnv')}
                    </Button>
                  </>
                )}
              </Form.List>
            </Form.Item>
          </>
        ) : (
          <>
            <Form.Item
              label={t('mcp.url')}
              name="url"
              rules={[{ required: true, message: t('mcp.urlRequired') }]}
            >
              <Input placeholder="https://example.com/mcp" />
            </Form.Item>

            <Form.Item label={t('mcp.bearerToken')} name="bearerToken">
              <Input.Password placeholder={t('mcp.bearerTokenPlaceholder')} />
            </Form.Item>

            <Form.Item label={t('mcp.headers')}>
              <Form.List name="headers">
                {(fields, { add, remove }) => (
                  <>
                    {fields.map((field) => (
                      <div key={field.key} className={styles.kvRow}>
                        <Form.Item
                          {...field}
                          name={[field.name, 'key']}
                          noStyle
                        >
                          <Input placeholder={t('mcp.headerKey')} className={styles.kvKey} />
                        </Form.Item>
                        <Form.Item
                          {...field}
                          name={[field.name, 'value']}
                          noStyle
                        >
                          <Input placeholder={t('mcp.headerValue')} className={styles.kvValue} />
                        </Form.Item>
                        <MinusCircleOutlined onClick={() => remove(field.name)} />
                      </div>
                    ))}
                    <Button type="dashed" onClick={() => add()} block icon={<PlusOutlined />}>
                      {t('mcp.addHeader')}
                    </Button>
                  </>
                )}
              </Form.List>
            </Form.Item>
          </>
        )}

        <Form.Item label={t('mcp.description')} name="description">
          <Input.TextArea rows={2} placeholder={t('mcp.descriptionPlaceholder')} />
        </Form.Item>
      </Form>

      <div className={styles.toolsSection}>
        <div className={styles.toolsLabel}>{t('mcp.enabledTools')}</div>
        <div className={styles.toolsHint}>{t('mcp.enabledToolsHint')}</div>
        <div className={styles.toolsGrid}>
          {installedTools.length > 0 ? (
            installedTools.map((tool) => (
              <Checkbox
                key={tool.key}
                checked={selectedTools.includes(tool.key)}
                onChange={() => handleToolToggle(tool.key)}
              >
                {tool.display_name}
              </Checkbox>
            ))
          ) : (
            <span className={styles.noTools}>{t('mcp.noToolsInstalled')}</span>
          )}
          {uninstalledTools.length > 0 && (
            <Dropdown
              trigger={['click']}
              menu={{
                items: uninstalledTools.map((tool) => ({
                  key: tool.key,
                  label: (
                    <span>
                      {tool.display_name}
                      <span className={styles.notInstalledTag}>{t('mcp.notInstalled')}</span>
                    </span>
                  ),
                  onClick: () => handleToolToggle(tool.key),
                })),
              }}
            >
              <Button type="dashed" size="small" icon={<PlusOutlined />} />
            </Dropdown>
          )}
        </div>
      </div>
    </Modal>
  );
};

export default AddMcpModal;
