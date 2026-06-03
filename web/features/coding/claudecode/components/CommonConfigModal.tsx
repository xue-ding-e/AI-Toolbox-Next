import React from 'react';
import { Modal, Alert, Button, Checkbox, message } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  extractClaudeCommonConfigFromCurrentFile,
  getClaudeCommonConfig,
  saveClaudeCommonConfig,
  saveClaudeLocalConfig,
} from '@/services/claudeCodeApi';
import JsonEditor from '@/components/common/JsonEditor';
import styles from './CommonConfigModal.module.less';

interface CommonConfigModalProps {
  open: boolean;
  onCancel: () => void;
  onSuccess: () => void;
  isLocalProvider?: boolean;
  gatewaySaveLocked?: boolean;
}

const CommonConfigModal: React.FC<CommonConfigModalProps> = ({
  open,
  onCancel,
  onSuccess,
  isLocalProvider = false,
  gatewaySaveLocked = false,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configValue, setConfigValue] = React.useState<unknown>({});
  const [rootDir, setRootDir] = React.useState<string | null>(null);
  const [isEditorValid, setIsEditorValid] = React.useState(true);

  // Use ref for validation state to avoid re-renders during editing
  const isValidRef = React.useRef(true);

  // 加载现有配置
  React.useEffect(() => {
    if (open) {
      loadConfig();
    }
  }, [open]);

  const loadConfig = async () => {
    setLoading(true);
    try {
      const config = await getClaudeCommonConfig();
      if (config?.config) {
        try {
          const configObj = JSON.parse(config.config);
          setConfigValue(configObj);
          setRootDir(config.rootDir ?? null);
          setIsEditorValid(true);
          isValidRef.current = true;
        } catch (error) {
          console.error('Failed to parse config JSON:', error);
          setConfigValue(config.config);
          setRootDir(config.rootDir ?? null);
          setIsEditorValid(false);
          isValidRef.current = false;
        }
      } else {
        // 空配置时设置为空对象，让 JSON 编辑器显示对象语义
        setConfigValue({});
        setRootDir(null);
        setIsEditorValid(true);
        isValidRef.current = true;
      }
    } catch (error) {
      console.error('Failed to load common config:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleExtractFromCurrentConfig = async () => {
    setLoading(true);
    try {
      const extractedConfig = await extractClaudeCommonConfigFromCurrentFile();
      const extractedValue = extractedConfig.config
        ? JSON.parse(extractedConfig.config) as unknown
        : {};
      if (!isPlainObject(extractedValue)) {
        throw new Error(t('claudecode.commonConfig.invalidJsonObject'));
      }
      setConfigValue(extractedValue);
      setRootDir(extractedConfig.rootDir ?? null);
      setIsEditorValid(true);
      isValidRef.current = true;
      message.success(t('claudecode.commonConfig.extractSuccess'));
    } catch (error) {
      console.error('Failed to extract common config from current file:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (gatewaySaveLocked) {
      message.warning(t('gateway.proxy.commonConfigSaveLockedTooltip'));
      return;
    }

    if (!isValidRef.current) {
      message.error(t('claudecode.commonConfig.invalidJson'));
      return;
    }

    const normalizedConfigValue = normalizeCommonConfigValue(configValue);
    if (!isPlainObject(normalizedConfigValue)) {
      message.error(t('claudecode.commonConfig.invalidJsonObject'));
      return;
    }

    setLoading(true);
    try {
      const configString = JSON.stringify(normalizedConfigValue, null, 2);
      if (isLocalProvider) {
        await saveClaudeLocalConfig({ commonConfig: configString, rootDir });
      } else {
        await saveClaudeCommonConfig({ config: configString, rootDir });
      }
      message.success(t('common.success'));
      onSuccess();
      onCancel();
    } catch (error) {
      console.error('Failed to save common config:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleEditorChange = (value: unknown, valid: boolean) => {
    setConfigValue(value);
    setIsEditorValid(valid);
    isValidRef.current = valid;
  };

  const quickOptionConfig = React.useMemo(
    () => isEditorValid ? parseCommonConfigObject(configValue) : null,
    [configValue, isEditorValid],
  );

  const toggleStates = React.useMemo(
    () => deriveCommonConfigToggleStates(quickOptionConfig),
    [quickOptionConfig],
  );

  const handleQuickOptionChange = (
    option: CommonConfigQuickOption,
    checked: boolean,
  ) => {
    if (!quickOptionConfig) {
      return;
    }

    const nextConfig = applyCommonConfigQuickOption(quickOptionConfig, option, checked);
    setConfigValue(nextConfig);
    setIsEditorValid(true);
    isValidRef.current = true;
  };

  const quickOptionsDisabled = !quickOptionConfig;

  return (
    <Modal
      title={t('claudecode.commonConfig.title')}
      open={open}
      onCancel={onCancel}
      onOk={handleSave}
      confirmLoading={loading}
      width={800}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
      footer={[
        <Button
          key="extract"
          onClick={handleExtractFromCurrentConfig}
          loading={loading}
        >
          {t('claudecode.commonConfig.extractFromCurrent')}
        </Button>,
        <Button key="cancel" onClick={onCancel} disabled={loading}>
          {t('common.cancel')}
        </Button>,
        <Button key="save" type="primary" onClick={handleSave} loading={loading}>
          {t('common.save')}
        </Button>,
      ]}
    >
      <div className={styles.content}>
        {isLocalProvider && (
          <Alert
            message={t('claudecode.localConfigHint')}
            type="warning"
            showIcon
          />
        )}
        <div className={styles.editorSection}>
          <div className={styles.quickOptions}>
            <Checkbox
              checked={toggleStates.hideAttribution}
              disabled={quickOptionsDisabled}
              onChange={(event) => handleQuickOptionChange('hideAttribution', event.target.checked)}
            >
              {t('claudecode.commonConfig.hideAttribution')}
            </Checkbox>
            <Checkbox
              checked={toggleStates.teammates}
              disabled={quickOptionsDisabled}
              onChange={(event) => handleQuickOptionChange('teammates', event.target.checked)}
            >
              {t('claudecode.commonConfig.enableTeammates')}
            </Checkbox>
            <Checkbox
              checked={toggleStates.enableToolSearch}
              disabled={quickOptionsDisabled}
              onChange={(event) => handleQuickOptionChange('enableToolSearch', event.target.checked)}
            >
              {t('claudecode.commonConfig.enableToolSearch')}
            </Checkbox>
            <Checkbox
              checked={toggleStates.effortMax}
              disabled={quickOptionsDisabled}
              onChange={(event) => handleQuickOptionChange('effortMax', event.target.checked)}
            >
              {t('claudecode.commonConfig.effortMax')}
            </Checkbox>
            <Checkbox
              checked={toggleStates.disableAutoUpgrade}
              disabled={quickOptionsDisabled}
              onChange={(event) => handleQuickOptionChange('disableAutoUpgrade', event.target.checked)}
            >
              {t('claudecode.commonConfig.disableAutoUpgrade')}
            </Checkbox>
          </div>
          <JsonEditor
            value={configValue}
            onChange={handleEditorChange}
            mode="text"
            height={400}
            minHeight={200}
            maxHeight={600}
            resizable
            placeholder={`{
    "skipWebFetchPreflight": true
}`}
          />
        </div>

        <Alert
          message={t('claudecode.commonConfig.combinedHint')}
          type="info"
          showIcon
        />
      </div>
    </Modal>
  );
};

export default CommonConfigModal;

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function normalizeCommonConfigValue(value: unknown): unknown {
  if (typeof value === 'string' && value.trim() === '') {
    return {};
  }

  if (typeof value === 'string') {
    try {
      return JSON.parse(value) as unknown;
    } catch {
      return value;
    }
  }

  return value;
}

type CommonConfigQuickOption =
  | 'hideAttribution'
  | 'teammates'
  | 'enableToolSearch'
  | 'effortMax'
  | 'disableAutoUpgrade';

interface CommonConfigToggleStates {
  hideAttribution: boolean;
  teammates: boolean;
  enableToolSearch: boolean;
  effortMax: boolean;
  disableAutoUpgrade: boolean;
}

const EMPTY_TOGGLE_STATES: CommonConfigToggleStates = {
  hideAttribution: false,
  teammates: false,
  enableToolSearch: false,
  effortMax: false,
  disableAutoUpgrade: false,
};

function parseCommonConfigObject(value: unknown): Record<string, unknown> | null {
  const normalizedValue = normalizeCommonConfigValue(value);
  return isPlainObject(normalizedValue) ? normalizedValue : null;
}

function deriveCommonConfigToggleStates(
  config: Record<string, unknown> | null,
): CommonConfigToggleStates {
  if (!config) {
    return EMPTY_TOGGLE_STATES;
  }

  const attribution = isPlainObject(config.attribution) ? config.attribution : {};
  const env = isPlainObject(config.env) ? config.env : {};

  return {
    hideAttribution: attribution.commit === '' && attribution.pr === '',
    teammates: env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS === '1' ||
      env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS === 1,
    enableToolSearch: env.ENABLE_TOOL_SEARCH === 'true' || env.ENABLE_TOOL_SEARCH === '1',
    effortMax: env.CLAUDE_CODE_EFFORT_LEVEL === 'max',
    disableAutoUpgrade: env.DISABLE_AUTOUPDATER === '1' || env.DISABLE_AUTOUPDATER === 1,
  };
}

function applyCommonConfigQuickOption(
  config: Record<string, unknown>,
  option: CommonConfigQuickOption,
  checked: boolean,
): Record<string, unknown> {
  const nextConfig = cloneCommonConfig(config);

  if (option === 'hideAttribution') {
    const attribution = isPlainObject(nextConfig.attribution)
      ? { ...nextConfig.attribution }
      : {};

    if (checked) {
      attribution.commit = '';
      attribution.pr = '';
      nextConfig.attribution = attribution;
    } else {
      delete attribution.commit;
      delete attribution.pr;
      if (Object.keys(attribution).length > 0) {
        nextConfig.attribution = attribution;
      } else {
        delete nextConfig.attribution;
      }
    }
    return nextConfig;
  }

  const env = isPlainObject(nextConfig.env) ? { ...nextConfig.env } : {};
  const envField = getCommonConfigEnvField(option);

  if (checked) {
    env[envField] = getCommonConfigEnvValue(option);
  } else {
    delete env[envField];
  }

  if (Object.keys(env).length > 0) {
    nextConfig.env = env;
  } else {
    delete nextConfig.env;
  }

  return nextConfig;
}

function getCommonConfigEnvField(option: Exclude<CommonConfigQuickOption, 'hideAttribution'>): string {
  switch (option) {
    case 'teammates':
      return 'CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS';
    case 'enableToolSearch':
      return 'ENABLE_TOOL_SEARCH';
    case 'effortMax':
      return 'CLAUDE_CODE_EFFORT_LEVEL';
    case 'disableAutoUpgrade':
      return 'DISABLE_AUTOUPDATER';
  }
}

function getCommonConfigEnvValue(option: Exclude<CommonConfigQuickOption, 'hideAttribution'>): string {
  switch (option) {
    case 'teammates':
      return '1';
    case 'enableToolSearch':
      return 'true';
    case 'effortMax':
      return 'max';
    case 'disableAutoUpgrade':
      return '1';
  }
}

function cloneCommonConfig(config: Record<string, unknown>): Record<string, unknown> {
  return JSON.parse(JSON.stringify(config)) as Record<string, unknown>;
}
