import React from 'react';
import { App, Button, Input, Space, Typography } from 'antd';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';

import { detectManualCliPath, probeManualCliVersion } from '@/services/settingsApi';
import { useSettingsStore } from '@/stores/settingsStore';

import styles from './CliManualPathSetting.module.less';

const { Text } = Typography;

interface CliManualPathSettingProps {
  /** CLI command name (e.g. `opencode`, `claude`, `grok`, `pi`, `omp`, `hermes`, `dsh`, `openclaw`). */
  commandName: string;
  /** i18n label for the tool name (e.g. `subModules.opencode`). Used to build the full title. */
  labelKey: string;
  /** Optional i18n key for a longer/full product name shown in the title (e.g. "Oh My Pi"). */
  toolNameKey?: string;
}

/**
 * "More Options" row for a tab's local CLI.
 *
 * - By default it always shows a read-only display state: if a path is saved it
 *   shows the path + probed version, otherwise it shows a "not set" placeholder.
 * - Clicking "edit" switches to an editable state with an input row, a browse
 *   button, a link-style confirmation button, and an "auto detect" link under
 *   the input that prefills the detected path.
 */
const CliManualPathSetting: React.FC<CliManualPathSettingProps> = ({
  commandName,
  labelKey,
  toolNameKey,
}) => {
  const { message } = App.useApp();
  const { t } = useTranslation();
  const savedPath = useSettingsStore((s) => s.cliManualPaths[commandName]);
  const setManualCliPath = useSettingsStore((s) => s.setManualCliPath);

  const [path, setPath] = React.useState(savedPath ?? '');
  const [editing, setEditing] = React.useState(false);
  const [saving, setSaving] = React.useState(false);
  const [detecting, setDetecting] = React.useState(false);
  const [checkingVersion, setCheckingVersion] = React.useState(false);
  const [version, setVersion] = React.useState<string | null>(null);
  const [versionError, setVersionError] = React.useState<string | null>(null);

  // Re-probe the version whenever the saved path changes (component remounts
  // when SidebarSettingsModal opens because `destroyOnHidden` is used).
  React.useEffect(() => {
    const saved = savedPath ?? '';
    setPath(saved);
    if (!saved.trim()) {
      setVersion(null);
      setVersionError(null);
      return;
    }
    let cancelled = false;
    setCheckingVersion(true);
    setVersionError(null);
    probeManualCliVersion(saved)
      .then((probed) => {
        if (!cancelled) setVersion(probed);
      })
      .catch((error) => {
        if (!cancelled) {
          setVersion(null);
          setVersionError(String(error));
        }
      })
      .finally(() => {
        if (!cancelled) setCheckingVersion(false);
      });
    return () => {
      cancelled = true;
    };
  }, [savedPath]);

  const handleSelectFile = async () => {
    try {
      const selected = await open({
        title: t('common.moreOptionsSelectCliFile'),
        multiple: false,
        directory: false,
      });
      if (selected && typeof selected === 'string') {
        setPath(selected);
      }
    } catch (error) {
      console.error('Failed to select CLI file:', error);
      message.error(t('common.error'));
    }
  };

  const handleDetect = async () => {
    setDetecting(true);
    try {
      const detected = await detectManualCliPath(commandName);
      setPath(detected);
      message.success(t('common.moreOptionsCliDetectSuccess'));
    } catch (error) {
      console.error('Failed to auto-detect CLI path:', error);
      message.error(t('common.moreOptionsCliDetectFailed'));
    } finally {
      setDetecting(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      const probedVersion = await setManualCliPath(commandName, path);
      setVersion(probedVersion || null);
      setVersionError(null);
      setEditing(false);
      if (probedVersion) {
        message.success(t('common.moreOptionsCliPathSaved'));
      } else {
        message.success(t('common.moreOptionsCliPathCleared'));
      }
    } catch (error) {
      setVersion(null);
      setVersionError(String(error));
      // The error message guides the user back to "More Options".
      message.error(t('common.moreOptionsCliPathInvalid'));
    } finally {
      setSaving(false);
    }
  };

  const hasPath = Boolean((savedPath ?? '').trim());

  return (
    <div className={styles.cliManualPath}>
      <div className={styles.titleRow}>
      <span className={styles.title}>
        {t('common.moreOptionsCliPathTitle', { tool: toolNameKey ? t(toolNameKey) : t(labelKey) })}
      </span>
      {hasPath &&
        (checkingVersion ? (
          <Text type="secondary" className={styles.titleVersion}>
            {t('common.moreOptionsCliProbing')}
          </Text>
        ) : version ? (
          <Text type="secondary" className={styles.titleVersion}>
            {t('common.moreOptionsCliVersion')}: {version}
          </Text>
        ) : (
          <Text type="danger" className={styles.titleVersion}>
            {t('common.moreOptionsCliVersionFailed')}
          </Text>
        ))}
    </div>

      {editing ? (
        <>
          <div className={styles.inputRow}>
            <Space.Compact style={{ width: '100%', flex: 1 }}>
              <Input
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder={t('common.moreOptionsCliPathPlaceholder')}
                allowClear
                onPressEnter={handleSave}
              />
              <Button onClick={handleSelectFile}>{t('common.browse')}</Button>
            </Space.Compact>
            <Button type="link" size="small" loading={saving} onClick={handleSave}>
              {t('common.save')}
            </Button>
          </div>
          <div className={styles.actions}>
            <Button type="link" size="small" loading={detecting} onClick={handleDetect}>
              {t('common.moreOptionsCliDetect')}
            </Button>
          </div>
        </>
      ) : (
        <div className={styles.displayRow}>
          <div className={styles.displayText}>
            {hasPath ? (
              <Text className={styles.pathText}>{savedPath}</Text>
            ) : (
              <Text type="secondary" className={styles.notSetText}>
                {t('common.moreOptionsCliNotSet')}
              </Text>
            )}
            {hasPath && versionError && !checkingVersion && (
              <Text type="danger" className={styles.errorText}>
                {versionError}
              </Text>
            )}
          </div>
          <Button type="link" size="small" onClick={() => setEditing(true)}>
            {t('common.edit')}
          </Button>
        </div>
      )}

      {editing && versionError && (
        <div className={styles.meta}>
          <Text type="danger" className={styles.errorText}>
            {versionError}
          </Text>
        </div>
      )}
    </div>
  );
};

export default CliManualPathSetting;
