import React, { useState } from 'react';
import { Modal, Button, Tag, Checkbox, message } from 'antd';
import { useTranslation } from 'react-i18next';
import { useMcpTools } from '../../hooks/useMcpTools';
import { useMcpStore } from '../../stores/mcpStore';
import * as mcpApi from '../../services/mcpApi';
import type { CreateMcpServerInput, McpServer, StdioConfig, HttpConfig } from '../../types';
import JsonEditor from '@/components/common/JsonEditor';
import styles from './ImportMcpModal.module.less';

interface ParsedServer {
  name: string;
  server_type: 'stdio' | 'http' | 'sse';
  server_config: StdioConfig | HttpConfig;
  isDuplicate: boolean;
  existingId?: string;
}

interface ImportJsonModalProps {
  open: boolean;
  servers: McpServer[];
  onClose: () => void;
  onSuccess: () => void;
  onSyncAll?: () => Promise<unknown>;
}

export const ImportJsonModal: React.FC<ImportJsonModalProps> = ({
  open,
  servers,
  onClose,
  onSuccess,
  onSyncAll,
}) => {
  const { t } = useTranslation();
  const { installedMcpTools } = useMcpTools();
  const { fetchServers } = useMcpStore();
  const [jsonValue, setJsonValue] = useState<unknown>(null);
  const [jsonValid, setJsonValid] = useState(false);
  const [parsedServers, setParsedServers] = useState<ParsedServer[]>([]);
  const [selectedServers, setSelectedServers] = useState<Set<string>>(new Set());
  const [selectedTools, setSelectedTools] = useState<string[]>([]);
  const [step, setStep] = useState<'input' | 'confirm'>('input');
  const [loading, setLoading] = useState(false);
  const [parseError, setParseError] = useState<string | null>(null);

  // Initialize selected tools with all installed tools
  React.useEffect(() => {
    if (open) {
      setSelectedTools(installedMcpTools.map((t) => t.key));
    }
  }, [open, installedMcpTools]);

  const resetState = () => {
    setJsonValue(null);
    setJsonValid(false);
    setParsedServers([]);
    setSelectedServers(new Set());
    setStep('input');
    setParseError(null);
  };

  const handleClose = () => {
    resetState();
    onClose();
  };

  // Detect server type from config
  const detectServerType = (config: Record<string, unknown>): 'stdio' | 'http' | 'sse' => {
    if (config.type === 'stdio' || config.type === 'local') return 'stdio';
    if (config.type === 'http') return 'http';
    if (config.type === 'sse' || config.type === 'remote') return 'sse';
    if (config.command) return 'stdio';
    if (config.url) return 'http';
    return 'stdio';
  };

  // Parse server config to unified format
  const parseServerConfig = (config: Record<string, unknown>): StdioConfig | HttpConfig => {
    const serverType = detectServerType(config);

    if (serverType === 'stdio') {
      let command = '';
      let args: string[] = [];

      if (Array.isArray(config.command)) {
        command = String(config.command[0] || '');
        args = config.command.slice(1).map(String);
      } else {
        command = String(config.command || '');
        args = Array.isArray(config.args) ? config.args.map(String) : [];
      }

      const env = (config.env || config.environment) as Record<string, string> | undefined;

      return {
        command,
        args,
        env: env && Object.keys(env).length > 0 ? env : undefined,
      };
    } else {
      return {
        url: String(config.url || ''),
        headers: config.headers as Record<string, string> | undefined,
      };
    }
  };

  const handleParse = () => {
    if (!jsonValid || !jsonValue) {
      setParseError(t('mcp.importJson.invalidJson'));
      return;
    }

    try {
      const parsed = jsonValue as Record<string, unknown>;
      const result: ParsedServer[] = [];

      // Check if it's wrapped in mcpServers
      const serversObj = (parsed.mcpServers || parsed) as Record<string, unknown>;

      // Validate it's an object
      if (typeof serversObj !== 'object' || Array.isArray(serversObj)) {
        throw new Error(t('mcp.importJson.invalidFormat'));
      }

      // Parse each server
      for (const [name, config] of Object.entries(serversObj)) {
        if (typeof config !== 'object' || config === null) continue;

        const serverConfig = config as Record<string, unknown>;
        const serverType = detectServerType(serverConfig);

        // Check if duplicate
        const existing = servers.find((s) => s.name === name);

        result.push({
          name,
          server_type: serverType,
          server_config: parseServerConfig(serverConfig),
          isDuplicate: !!existing,
          existingId: existing?.id,
        });
      }

      if (result.length === 0) {
        throw new Error(t('mcp.importJson.noServersFound'));
      }

      setParsedServers(result);
      setSelectedServers(new Set(result.map((s) => s.name)));
      setParseError(null);
      setStep('confirm');
    } catch (error) {
      setParseError(String(error));
    }
  };

  const handleToggleServer = (name: string) => {
    setSelectedServers((prev) => {
      const next = new Set(prev);
      if (next.has(name)) {
        next.delete(name);
      } else {
        next.add(name);
      }
      return next;
    });
  };

  const handleSelectAllServers = () => {
    if (selectedServers.size === parsedServers.length) {
      setSelectedServers(new Set());
    } else {
      setSelectedServers(new Set(parsedServers.map((s) => s.name)));
    }
  };

  const handleToggleTool = (toolKey: string) => {
    setSelectedTools((prev) =>
      prev.includes(toolKey)
        ? prev.filter((k) => k !== toolKey)
        : [...prev, toolKey]
    );
  };

  const handleImport = async () => {
    if (selectedServers.size === 0) return;

    // Check if any selected servers are duplicates
    const duplicates = parsedServers.filter(
      (s) => selectedServers.has(s.name) && s.isDuplicate
    );

    const doImport = async () => {
      setLoading(true);
      let successCount = 0;
      let overwriteCount = 0;
      const errors: string[] = [];

      try {
        for (const server of parsedServers) {
          if (!selectedServers.has(server.name)) continue;

          try {
            if (server.isDuplicate && server.existingId) {
              // Overwrite existing server
              await mcpApi.updateMcpServer(server.existingId, {
                server_type: server.server_type,
                server_config: server.server_config,
                enabled_tools: selectedTools,
              });
              overwriteCount++;
            } else {
              // Create new server
              const input: CreateMcpServerInput = {
                name: server.name,
                server_type: server.server_type,
                server_config: server.server_config,
                enabled_tools: selectedTools,
              };
              await mcpApi.createMcpServer(input);
            }
            successCount++;
          } catch (error) {
            errors.push(`${server.name}: ${String(error)}`);
          }
        }

        if (successCount > 0) {
          message.success(t('mcp.importSuccess', { count: successCount }));
          await fetchServers();
          // Auto sync all tools if any servers were overwritten
          if (overwriteCount > 0 && onSyncAll) {
            await onSyncAll();
          }
          onSuccess();
          handleClose();
        } else if (errors.length > 0) {
          message.error(errors.join('; '));
        }
      } catch (error) {
        message.error(t('mcp.importFailed') + ': ' + String(error));
      } finally {
        setLoading(false);
      }
    };

    if (duplicates.length > 0) {
      Modal.confirm({
        title: t('mcp.duplicateName.title'),
        content: t('mcp.duplicateName.batchContent', {
          names: duplicates.map((s) => s.name).join(', '),
        }),
        okText: t('mcp.duplicateName.overwrite'),
        cancelText: t('common.cancel'),
        onOk: doImport,
      });
    } else {
      await doImport();
    }
  };

  const renderInputStep = () => (
    <>
      <p className={styles.hint}>{t('mcp.importJson.hint')}</p>
      <JsonEditor
        value={jsonValue}
        onChange={(val, isValid) => {
          setJsonValue(val);
          setJsonValid(isValid);
          if (parseError) setParseError(null);
        }}
        height={350}
        placeholder={t('mcp.importJson.placeholder')}
      />
      {parseError && (
        <div style={{ color: 'var(--color-error)', marginTop: 8, fontSize: 13 }}>
          {parseError}
        </div>
      )}
      <div className={styles.footer}>
        <Button onClick={handleClose}>{t('common.cancel')}</Button>
        <Button
          type="primary"
          onClick={handleParse}
          disabled={!jsonValid}
        >
          {t('mcp.importJson.parse')}
        </Button>
      </div>
    </>
  );

  const renderConfirmStep = () => (
    <>
      <p className={styles.hint}>{t('mcp.importJson.confirmHint')}</p>

      <div className={styles.selectAll}>
        <Checkbox
          checked={selectedServers.size === parsedServers.length}
          indeterminate={selectedServers.size > 0 && selectedServers.size < parsedServers.length}
          onChange={handleSelectAllServers}
        >
          {t('mcp.selectAll')}
        </Checkbox>
        <span className={styles.count}>
          {t('mcp.selectedCount', {
            selected: selectedServers.size,
            total: parsedServers.length,
          })}
        </span>
      </div>

      <div className={styles.list} style={{ maxHeight: 250 }}>
        {parsedServers.map((server) => (
          <div
            key={server.name}
            className={`${styles.toolItem} ${selectedServers.has(server.name) ? styles.selected : ''}`}
            onClick={() => handleToggleServer(server.name)}
          >
            <Checkbox checked={selectedServers.has(server.name)} />
            <div className={styles.toolInfo}>
              <div className={styles.toolHeader}>
                <span className={styles.toolName}>{server.name}</span>
                <Tag style={{ margin: 0 }}>{server.server_type}</Tag>
                {server.isDuplicate && (
                  <Tag color="orange" style={{ margin: 0 }}>{t('mcp.importJson.duplicate')}</Tag>
                )}
              </div>
              <div className={styles.toolPath}>
                {server.server_type === 'stdio'
                  ? (server.server_config as StdioConfig).command
                  : (server.server_config as HttpConfig).url}
              </div>
            </div>
          </div>
        ))}
      </div>

      {installedMcpTools.length > 0 && (
        <div style={{ marginTop: 16 }}>
          <div style={{ fontSize: 13, marginBottom: 8, color: 'var(--color-text-secondary)' }}>
            {t('mcp.enabledTools')}:
          </div>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
            {installedMcpTools.map((tool) => (
              <Checkbox
                key={tool.key}
                checked={selectedTools.includes(tool.key)}
                onChange={() => handleToggleTool(tool.key)}
              >
                {tool.display_name}
              </Checkbox>
            ))}
          </div>
        </div>
      )}

      <div className={styles.footer}>
        <Button onClick={() => setStep('input')}>{t('common.back')}</Button>
        <Button
          type="primary"
          onClick={handleImport}
          disabled={selectedServers.size === 0}
          loading={loading}
        >
          {t('mcp.importJson.import')} ({selectedServers.size})
        </Button>
      </div>
    </>
  );

  return (
    <Modal
      title={t('mcp.importJson.title')}
      open={open}
      onCancel={handleClose}
      footer={null}
      width={700}
      destroyOnClose
    >
      {step === 'input' ? renderInputStep() : renderConfirmStep()}
    </Modal>
  );
};
