import React from 'react';
import { Modal, Switch, message } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  getClaudePluginStatus,
  applyClaudePluginConfig,
  getClaudeOnboardingStatus,
  applyClaudeOnboardingSkip,
  clearClaudeOnboardingSkip,
} from '@/services/claudeCodeApi';
import styles from './ClaudeCodeSettingsModal.module.less';

interface ClaudeCodeSettingsModalProps {
  open: boolean;
  onClose: () => void;
}

export const ClaudeCodeSettingsModal: React.FC<ClaudeCodeSettingsModalProps> = ({
  open,
  onClose,
}) => {
  const { t } = useTranslation();
  const [vscodeEnabled, setVscodeEnabled] = React.useState(false);
  const [skipOnboarding, setSkipOnboarding] = React.useState(false);
  const [vscodeLoading, setVscodeLoading] = React.useState(false);
  const [onboardingLoading, setOnboardingLoading] = React.useState(false);

  // Load settings on mount
  React.useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const [pluginStatus, onboardingStatus] = await Promise.all([
        getClaudePluginStatus(),
        getClaudeOnboardingStatus(),
      ]);
      setVscodeEnabled(pluginStatus.enabled);
      setSkipOnboarding(onboardingStatus);
    } catch (error) {
      console.error('Failed to load settings:', error);
    }
  };

  const handleVscodeToggle = async (checked: boolean) => {
    setVscodeLoading(true);
    try {
      await applyClaudePluginConfig(checked);
      setVscodeEnabled(checked);
      message.success(
        checked
          ? t('claudecode.plugin.enabled')
          : t('claudecode.plugin.disabled')
      );
    } catch (error) {
      console.error('Failed to toggle VSCode integration:', error);
      message.error(t('common.error'));
    } finally {
      setVscodeLoading(false);
    }
  };

  const handleOnboardingToggle = async (checked: boolean) => {
    setOnboardingLoading(true);
    try {
      if (checked) {
        await applyClaudeOnboardingSkip();
      } else {
        await clearClaudeOnboardingSkip();
      }
      setSkipOnboarding(checked);
      message.success(t('common.success'));
    } catch (error) {
      console.error('Failed to toggle onboarding skip:', error);
      message.error(t('common.error'));
    } finally {
      setOnboardingLoading(false);
    }
  };

  return (
    <Modal
      title={t('claudecode.settings.title')}
      open={open}
      onCancel={onClose}
      footer={null}
      width={550}
    >
      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('claudecode.settings.vscode')}</label>
        </div>
        <div className={styles.inputArea}>
          <Switch
            checked={vscodeEnabled}
            loading={vscodeLoading}
            onChange={handleVscodeToggle}
          />
          <p className={styles.hint}>{t('claudecode.settings.vscodeHint')}</p>
        </div>
      </div>

      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('claudecode.settings.skipOnboarding')}</label>
        </div>
        <div className={styles.inputArea}>
          <Switch
            checked={skipOnboarding}
            loading={onboardingLoading}
            onChange={handleOnboardingToggle}
          />
          <p className={styles.hint}>{t('claudecode.settings.skipOnboardingHint')}</p>
        </div>
      </div>
    </Modal>
  );
};

export default ClaudeCodeSettingsModal;
