import React, { useMemo } from 'react';
import { Modal, Checkbox, Button, Empty, message, Spin, Tag, Dropdown } from 'antd';
import { PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useMcpStore } from '../../stores/mcpStore';
import { useMcpTools } from '../../hooks/useMcpTools';
import * as mcpApi from '../../services/mcpApi';
import styles from './ImportMcpModal.module.less';
import addMcpStyles from './AddMcpModal.module.less';

interface ImportMcpModalProps {
  open: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

export const ImportMcpModal: React.FC<ImportMcpModalProps> = ({
  open,
  onClose,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const { fetchServers, scanResult, loadScanResult } = useMcpStore();
  const { tools } = useMcpTools();
  const [selected, setSelected] = React.useState<Set<string>>(new Set());
  const [selectedTools, setSelectedTools] = React.useState<string[]>([]);
  const [loading, setLoading] = React.useState(false);
  const [scanning, setScanning] = React.useState(false);
  const [preferredTools, setPreferredTools] = React.useState<string[] | null>(null);

  // Group discovered servers by tool_key
  const serversByTool = React.useMemo(() => {
    const map = new Map<string, string[]>();
    if (scanResult?.servers) {
      for (const server of scanResult.servers) {
        const list = map.get(server.tool_key) || [];
        list.push(server.name);
        map.set(server.tool_key, list);
      }
    }
    return map;
  }, [scanResult]);

  // Only show tools that have discovered servers
  const toolsWithServers = React.useMemo(() => {
    return tools.filter((tool) => {
      const servers = serversByTool.get(tool.key);
      return servers && servers.length > 0 && tool.installed;
    });
  }, [tools, serversByTool]);

  // Split tools based on preferred tools setting + selected tools
  const visibleTools = useMemo(() => {
    if (preferredTools && preferredTools.length > 0) {
      // If preferred tools are set, show those + any selected tools
      return tools.filter((t) => preferredTools.includes(t.key) || selectedTools.includes(t.key));
    }
    // Otherwise show installed tools + any selected tools
    return tools.filter((t) => t.installed || selectedTools.includes(t.key));
  }, [tools, preferredTools, selectedTools]);

  // Hidden tools: everything not in visible list, sorted by installed first
  const hiddenTools = useMemo(() => {
    const hidden = preferredTools && preferredTools.length > 0
      ? tools.filter((t) => !preferredTools.includes(t.key) && !selectedTools.includes(t.key))
      : tools.filter((t) => !t.installed && !selectedTools.includes(t.key));
    // Sort: installed first
    return [...hidden].sort((a, b) => {
      if (a.installed === b.installed) return 0;
      return a.installed ? -1 : 1;
    });
  }, [tools, preferredTools, selectedTools]);

  // Track if we've initialized selection for this open session
  const initializedRef = React.useRef(false);
  const toolsInitializedRef = React.useRef(false);

  // Load preferred tools on mount
  React.useEffect(() => {
    const loadPreferredTools = async () => {
      try {
        const preferred = await mcpApi.getMcpPreferredTools();
        setPreferredTools(preferred);
      } catch (error) {
        console.error('Failed to load preferred tools:', error);
      }
    };
    loadPreferredTools();
  }, []);

  // Reset initialized state when modal closes
  React.useEffect(() => {
    if (!open) {
      initializedRef.current = false;
      toolsInitializedRef.current = false;
    }
  }, [open]);

  // Trigger scan when modal opens
  React.useEffect(() => {
    if (open) {
      setScanning(true);
      loadScanResult().finally(() => setScanning(false));
    }
  }, [open, loadScanResult]);

  React.useEffect(() => {
    if (!initializedRef.current && toolsWithServers.length > 0) {
      // Don't pre-select any tools by default
      setSelected(new Set());
      initializedRef.current = true;
    }
  }, [toolsWithServers]);

  // Initialize selected tools based on preferredTools (same logic as AddMcpModal)
  React.useEffect(() => {
    if (open && !toolsInitializedRef.current && preferredTools !== null) {
      if (preferredTools.length > 0) {
        setSelectedTools(preferredTools);
      } else {
        // preferredTools loaded but empty, use installed tools
        const installed = tools.filter((t) => t.installed).map((t) => t.key);
        setSelectedTools(installed);
      }
      toolsInitializedRef.current = true;
    }
  }, [open, tools, preferredTools]);

  const handleToggle = (toolKey: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(toolKey)) {
        next.delete(toolKey);
      } else {
        next.add(toolKey);
      }
      return next;
    });
  };

  const handleToolToggle = (toolKey: string) => {
    setSelectedTools((prev) =>
      prev.includes(toolKey)
        ? prev.filter((k) => k !== toolKey)
        : [...prev, toolKey]
    );
  };

  const handleSelectAll = () => {
    const allKeys = toolsWithServers.map((t) => t.key);
    if (selected.size === allKeys.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(allKeys));
    }
  };

  const handleImport = async () => {
    if (selected.size === 0) return;

    setLoading(true);
    let totalImported = 0;
    let totalSkipped = 0;
    const allDuplicated: string[] = [];
    const errors: string[] = [];

    try {
      for (const toolKey of selected) {
        try {
          const result = await mcpApi.importMcpFromTool(toolKey, selectedTools);
          totalImported += result.servers_imported;
          totalSkipped += result.servers_skipped;
          if (result.servers_duplicated?.length > 0) {
            allDuplicated.push(...result.servers_duplicated);
          }
          if (result.errors.length > 0) {
            errors.push(...result.errors);
          }
        } catch (error) {
          errors.push(`${toolKey}: ${String(error)}`);
        }
      }

      if (totalImported > 0) {
        message.success(t('mcp.importSuccess', { count: totalImported }));
      } else if (totalSkipped > 0) {
        message.info(t('mcp.importSkipped', { count: totalSkipped }));
      } else {
        message.info(t('mcp.importNoServers'));
      }

      // Show warning for duplicated servers with different configs
      if (allDuplicated.length > 0) {
        message.warning(
          t('mcp.importDuplicated', { names: allDuplicated.join(', ') }),
          5
        );
      }

      if (errors.length > 0) {
        console.error('Import errors:', errors);
      }

      await fetchServers();
      onSuccess();
    } catch (error) {
      message.error(t('mcp.importFailed') + ': ' + String(error));
    } finally {
      setLoading(false);
    }
  };

  const totalServersFound = scanResult?.total_servers_found || 0;

  return (
    <Modal
      title={t('mcp.importTitle')}
      open={open}
      onCancel={onClose}
      footer={null}
      width={600}
    >
      <Spin spinning={loading || scanning}>
        <p className={styles.hint}>{t('mcp.importSummary')}</p>

        {scanning ? (
          <div className={styles.scanningHint}>
            {t('mcp.scanning')}
          </div>
        ) : (
          <>
            <div className={styles.stats}>
              <span>{t('mcp.serversFound', { count: totalServersFound })}</span>
            </div>

            {toolsWithServers.length === 0 ? (
              <Empty description={t('mcp.noToolsToImport')} />
            ) : (
              <>
                <div className={styles.selectAll}>
                  <Checkbox
                    checked={selected.size === toolsWithServers.length}
                    indeterminate={selected.size > 0 && selected.size < toolsWithServers.length}
                    onChange={handleSelectAll}
                  >
                    {t('mcp.selectAll')}
                  </Checkbox>
                  <span className={styles.count}>
                    {t('mcp.selectedCount', {
                      selected: selected.size,
                      total: toolsWithServers.length,
                    })}
                  </span>
                </div>

                <div className={styles.list}>
                  {toolsWithServers.map((tool) => {
                    const servers = serversByTool.get(tool.key) || [];
                    return (
                      <div
                        key={tool.key}
                        className={`${styles.toolItem} ${selected.has(tool.key) ? styles.selected : ''}`}
                        onClick={() => handleToggle(tool.key)}
                      >
                        <Checkbox checked={selected.has(tool.key)} />
                        <div className={styles.toolInfo}>
                          <div className={styles.toolHeader}>
                            <span className={styles.toolName}>{tool.display_name}</span>
                            <span className={styles.toolPath}>{tool.mcp_config_path}</span>
                          </div>
                          <div className={styles.serverList}>
                            {servers.map((name) => (
                              <Tag key={name} className={styles.serverTag}>{name}</Tag>
                            ))}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>

                <div className={addMcpStyles.toolsSection}>
                  <div className={addMcpStyles.toolsLabel}>{t('mcp.enabledTools')}</div>
                  <div className={addMcpStyles.toolsHint}>{t('mcp.enabledToolsHint')}</div>
                  <div className={addMcpStyles.toolsGrid}>
                    {visibleTools.length > 0 ? (
                      visibleTools.map((tool) => (
                        <Checkbox
                          key={tool.key}
                          checked={selectedTools.includes(tool.key)}
                          onChange={() => handleToolToggle(tool.key)}
                        >
                          {tool.display_name}
                        </Checkbox>
                      ))
                    ) : (
                      <span className={addMcpStyles.noTools}>{t('mcp.noToolsInstalled')}</span>
                    )}
                    {hiddenTools.length > 0 && (
                      <Dropdown
                        trigger={['click']}
                        menu={{
                          items: hiddenTools.map((tool) => ({
                            key: tool.key,
                            disabled: !tool.installed,
                            label: (
                              <Checkbox
                                checked={selectedTools.includes(tool.key)}
                                disabled={!tool.installed}
                                onClick={(e) => e.stopPropagation()}
                              >
                                {tool.display_name}
                                {!tool.installed && (
                                  <span className={addMcpStyles.notInstalledTag}> {t('mcp.notInstalled')}</span>
                                )}
                              </Checkbox>
                            ),
                            onClick: () => {
                              if (tool.installed) {
                                handleToolToggle(tool.key);
                              }
                            },
                          })),
                        }}
                      >
                        <Button type="dashed" size="small" icon={<PlusOutlined />} />
                      </Dropdown>
                    )}
                  </div>
                </div>
              </>
            )}
          </>
        )}

        <div className={styles.footer}>
          <Button onClick={onClose}>{t('common.close')}</Button>
          <Button
            type="primary"
            onClick={handleImport}
            disabled={selected.size === 0 || scanning}
            loading={loading}
          >
            {t('mcp.importAndSync')}
          </Button>
        </div>
      </Spin>
    </Modal>
  );
};
