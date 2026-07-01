import React from 'react';
import { Modal, Alert, Button, Checkbox, message } from 'antd';
import { useTranslation } from 'react-i18next';
import { parse as parseToml } from 'smol-toml';
import {
  extractCodexCommonConfigFromCurrentFile,
  getCodexCommonConfig,
  saveCodexCommonConfig,
  saveCodexLocalConfig,
} from '@/services/codexApi';
import TomlEditor from '@/components/common/TomlEditor';
import {
  canToggleCodexRemoteCompaction,
  getCodexIgnoredCommonConfigKeys,
  isCodexGoalModeEnabled,
  isCodexRemoteCompactionEnabled,
  setCodexGoalMode,
  setCodexRemoteCompaction,
} from '@/utils/codexConfigUtils';
import styles from './CodexCommonConfigModal.module.less';

interface CodexCommonConfigModalProps {
  open: boolean;
  onCancel: () => void;
  onSuccess: () => void;
  isLocalProvider?: boolean;
  gatewaySaveLocked?: boolean;
}

function isTomlTextValid(value: string): boolean {
  try {
    if (value.trim()) {
      parseToml(value);
    }
    return true;
  } catch {
    return false;
  }
}

const CodexCommonConfigModal: React.FC<CodexCommonConfigModalProps> = ({
  open,
  onCancel,
  onSuccess,
  isLocalProvider = false,
  gatewaySaveLocked = false,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configValue, setConfigValue] = React.useState<string>('');
  const [rootDir, setRootDir] = React.useState<string | null>(null);
  const [isTomlValid, setIsTomlValid] = React.useState(true);

  const loadConfig = React.useCallback(async () => {
    setLoading(true);
    try {
      const config = await getCodexCommonConfig();
      if (config?.config) {
        setConfigValue(config.config);
        setIsTomlValid(isTomlTextValid(config.config));
        setRootDir(config.rootDir ?? null);
      } else {
        setConfigValue('');
        setIsTomlValid(true);
        setRootDir(null);
      }
    } catch (error) {
      console.error('Failed to load common config:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  // Load existing config
  React.useEffect(() => {
    if (open) {
      loadConfig();
    }
  }, [loadConfig, open]);

  const updateConfigValue = React.useCallback((value: string) => {
    setConfigValue(value);
    setIsTomlValid(isTomlTextValid(value));
  }, []);

  const handleSave = async () => {
    if (gatewaySaveLocked) {
      message.warning(t('gateway.proxy.commonConfigSaveLockedTooltip'));
      return;
    }

    // 验证 TOML 格式
    if (!isTomlValid) {
      message.error(t('codex.provider.configTomlInvalid'));
      return;
    }

    setLoading(true);
    try {
      if (isLocalProvider) {
        await saveCodexLocalConfig({ commonConfig: configValue, rootDir });
      } else {
        await saveCodexCommonConfig({ config: configValue, rootDir });
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

  const handleEditorChange = (value: string) => {
    updateConfigValue(value);
  };

  const goalModeEnabled = React.useMemo(
    () => isTomlValid && isCodexGoalModeEnabled(configValue),
    [configValue, isTomlValid],
  );

  const remoteCompactionEnabled = React.useMemo(
    () => isTomlValid && isCodexRemoteCompactionEnabled(configValue),
    [configValue, isTomlValid],
  );

  const remoteCompactionEditable = React.useMemo(
    () => isTomlValid && canToggleCodexRemoteCompaction(configValue),
    [configValue, isTomlValid],
  );

  const ignoredCommonConfigKeys = React.useMemo(
    () => (isTomlValid ? getCodexIgnoredCommonConfigKeys(configValue) : []),
    [configValue, isTomlValid],
  );

  const handleGoalModeToggle = (checked: boolean) => {
    try {
      updateConfigValue(setCodexGoalMode(configValue, checked));
    } catch (error) {
      console.error('Failed to toggle Codex Goal mode:', error);
      message.error(t('codex.provider.configTomlInvalid'));
    }
  };

  const handleRemoteCompactionToggle = (checked: boolean) => {
    updateConfigValue(setCodexRemoteCompaction(configValue, checked));
  };

  const handleExtractFromCurrentConfig = async () => {
    setLoading(true);
    try {
      const extractedConfig = await extractCodexCommonConfigFromCurrentFile();
      const nextConfig = extractedConfig.config || '';
      setConfigValue(nextConfig);
      setRootDir(extractedConfig.rootDir ?? null);
      setIsTomlValid(isTomlTextValid(nextConfig));
      message.success(t('codex.commonConfig.extractSuccess'));
    } catch (error) {
      console.error('Failed to extract common config from current Codex file:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={t('codex.commonConfig.title')}
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
          {t('codex.commonConfig.extractFromCurrent')}
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
            message={t('codex.localConfigHint')}
            type="warning"
            showIcon
          />
        )}
        <div className={styles.editorSection}>
          <div className={styles.quickOptions}>
            <Checkbox
              checked={goalModeEnabled}
              disabled={!isTomlValid}
              onChange={(event) => handleGoalModeToggle(event.target.checked)}
            >
              {t('codex.commonConfig.enableGoalMode')}
            </Checkbox>
            <Checkbox
              checked={remoteCompactionEnabled}
              disabled={!remoteCompactionEditable}
              title={t('codex.commonConfig.remoteCompactionHint')}
              onChange={(event) => handleRemoteCompactionToggle(event.target.checked)}
            >
              {t('codex.commonConfig.enableRemoteCompaction')}
            </Checkbox>
          </div>
          <TomlEditor
            value={configValue}
            onChange={handleEditorChange}
            height={400}
          />
          {ignoredCommonConfigKeys.length > 0 && (
            <Alert
              message={t('codex.commonConfig.protectedWarningTitle')}
              description={t('codex.commonConfig.protectedWarningDescription', {
                keys: ignoredCommonConfigKeys.join(', '),
              })}
              type="warning"
              showIcon
            />
          )}
        </div>

        <Alert
          message={t('codex.commonConfig.description')}
          type="info"
          showIcon
        />
      </div>
    </Modal>
  );
};

export default CodexCommonConfigModal;
