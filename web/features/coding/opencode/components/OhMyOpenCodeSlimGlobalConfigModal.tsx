import React, { useState, useEffect, useRef } from 'react';
import { Modal, Button, message } from 'antd';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import type { 
  OhMyOpenCodeSlimGlobalConfig, 
  OhMyOpenCodeSlimGlobalConfigInput 
} from '@/types/ohMyOpenCodeSlim';

interface OhMyOpenCodeSlimGlobalConfigModalProps {
  open: boolean;
  initialConfig?: OhMyOpenCodeSlimGlobalConfig;
  onCancel: () => void;
  onSuccess: (values: OhMyOpenCodeSlimGlobalConfigInput) => void;
}

// Helper to remove empty values for display
const cleanObject = (obj: Record<string, unknown>): Record<string, unknown> => {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj)) {
    if (value === null || value === undefined) continue;
    if (Array.isArray(value) && value.length === 0) continue;
    if (typeof value === 'object' && value !== null && Object.keys(value).length === 0) continue;
    if (typeof value === 'string' && value === '') continue;
    result[key] = value;
  }
  return result;
};

const isEmptyObject = (value: Record<string, unknown>): boolean => Object.keys(value).length === 0;

const PLACEHOLDER_JSON = `{
  "tmux": {
    "enabled": true,
    "layout": "main-vertical",
    "main_pane_size": 60
  }
}`;

const OhMyOpenCodeSlimGlobalConfigModal: React.FC<OhMyOpenCodeSlimGlobalConfigModalProps> = ({
  open,
  initialConfig,
  onCancel,
  onSuccess
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const [configValue, setConfigValue] = useState<Record<string, unknown> | undefined>(undefined);
  const [isValid, setIsValid] = useState(true);
  
  const configRef = useRef<Record<string, unknown> | undefined>(undefined);

  useEffect(() => {
    if (open) {
      if (initialConfig) {
        // Merge known fields + otherFields into a single flat object
        // Convert camelCase to snake_case for the editor as preferred
        const merged: Record<string, unknown> = {
          sisyphus_agent: initialConfig.sisyphusAgent,
          disabled_agents: initialConfig.disabledAgents,
          disabled_mcps: initialConfig.disabledMcps,
          disabled_hooks: initialConfig.disabledHooks,
          lsp: initialConfig.lsp,
          experimental: initialConfig.experimental,
          ...(initialConfig.otherFields || {})
        };

        const cleaned = cleanObject(merged);
        if (isEmptyObject(cleaned)) {
          setConfigValue(undefined);
          configRef.current = {};
        } else {
          setConfigValue(cleaned);
          configRef.current = cleaned;
        }
      } else {
        // Initialize empty if no config provided
        setConfigValue(undefined);
        configRef.current = {};
      }
      setIsValid(true);
    } else if (!open) {
      setConfigValue(undefined);
      configRef.current = undefined;
    }
  }, [open, initialConfig]);

  const handleSave = () => {
    if (!isValid) {
      message.error(t('opencode.ohMyOpenCode.invalidJson'));
      return;
    }

    setLoading(true);
    try {
      const json = configRef.current || {};
      const raw = json as Record<string, unknown>;
      
      // Helper to extract object fields
      const getObject = (snakeKey: string, camelKey: string): Record<string, unknown> | undefined => {
        const val = raw[snakeKey] ?? raw[camelKey];
        if (val && typeof val === 'object' && !Array.isArray(val)) {
          return val as Record<string, unknown>;
        }
        return undefined;
      };

      // Helper to extract string array fields
      const getStringArray = (snakeKey: string, camelKey: string): string[] | undefined => {
        const val = raw[snakeKey] ?? raw[camelKey];
        if (Array.isArray(val)) {
          return val.filter((item): item is string => typeof item === 'string');
        }
        return undefined;
      };

      // Destructure known keys (support both snake_case and camelCase)
      const sisyphusAgent = getObject('sisyphus_agent', 'sisyphusAgent');
      const disabledAgents = getStringArray('disabled_agents', 'disabledAgents');
      const disabledMcps = getStringArray('disabled_mcps', 'disabledMcps');
      const disabledHooks = getStringArray('disabled_hooks', 'disabledHooks');
      const lsp = getObject('lsp', 'lsp');
      const experimental = getObject('experimental', 'experimental');

      const knownKeys = new Set([
        'sisyphus_agent', 'sisyphusAgent',
        'disabled_agents', 'disabledAgents',
        'disabled_mcps', 'disabledMcps',
        'disabled_hooks', 'disabledHooks',
        'lsp',
        'experimental'
      ]);

      const others: Record<string, unknown> = {};
      Object.keys(raw).forEach(key => {
        if (!knownKeys.has(key)) {
          others[key] = raw[key];
        }
      });

      // Construct input object
      const input: OhMyOpenCodeSlimGlobalConfigInput = {
        sisyphusAgent: sisyphusAgent ?? null,
        disabledAgents: disabledAgents ?? [],
        disabledMcps: disabledMcps ?? [],
        disabledHooks: disabledHooks ?? [],
        lsp: lsp ?? null,
        experimental: experimental ?? null,
        // All remaining fields go to otherFields, null if empty
        otherFields: Object.keys(others).length > 0 ? others : null
      };

      onSuccess(input);
    } catch (error) {
      console.error('Failed to prepare config for save:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={t('opencode.ohMyOpenCode.globalConfigTitle')}
      open={open}
      onCancel={onCancel}
      width={800}
      footer={[
        <Button key="cancel" onClick={onCancel}>
          {t('common.cancel')}
        </Button>,
        <Button key="save" type="primary" onClick={handleSave} loading={loading}>
          {t('common.save')}
        </Button>
      ]}
      styles={{ body: { paddingBottom: 0 } }}
    >
      <div style={{ marginTop: 24 }}>
        <JsonEditor
          value={configValue}
          onChange={(value, valid) => {
            setIsValid(valid);
            if (value === null) {
              configRef.current = {};
              setConfigValue(undefined);
            } else if (valid && typeof value === 'object' && !Array.isArray(value)) {
              configRef.current = value as Record<string, unknown>;
              setConfigValue(value as Record<string, unknown>);
            }
          }}
          onBlur={(value, valid) => {
            if (!valid) {
              return;
            }
            if (value === null) {
              configRef.current = {};
              setConfigValue(undefined);
              return;
            }
            if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
              const objectValue = value as Record<string, unknown>;
              configRef.current = objectValue;
              if (isEmptyObject(objectValue)) {
                setConfigValue(undefined);
              }
            }
          }}
          height={520}
          resizable={false}
          mode="text"
          placeholder={PLACEHOLDER_JSON}
        />
      </div>
    </Modal>
  );
};

export default OhMyOpenCodeSlimGlobalConfigModal;
