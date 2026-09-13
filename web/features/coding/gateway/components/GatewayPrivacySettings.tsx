import React from 'react';
import { Button, Switch } from 'antd';
import { ChevronRight, SlidersHorizontal } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  getGatewayPrivacySettings,
  updateGatewayPrivacySettings,
  type GatewayPrivacySettings as PrivacySettings,
} from '@/services/gatewayPrivacyApi';
import GatewayPrivacyRulesModal from './GatewayPrivacyRulesModal';
import styles from './GatewayPrivacySettings.module.less';

interface Props { running: boolean }

const GatewayPrivacySettings: React.FC<Props> = ({ running }) => {
  const { t } = useTranslation();
  const [settings, setSettings] = React.useState<PrivacySettings | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [saving, setSaving] = React.useState(false);
  const [editing, setEditing] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const revision = React.useRef(0);
  const savingRef = React.useRef(false);

  const load = React.useCallback(async () => {
    const request = ++revision.current;
    setLoading(true);
    setError(null);
    try {
      const saved = await getGatewayPrivacySettings();
      if (request === revision.current) setSettings(saved);
    } catch (error) {
      if (request === revision.current) setError(String(error));
    } finally {
      if (request === revision.current) setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    void load();
    return () => { revision.current += 1; };
  }, [load]);

  const toggle = async (enabled: boolean) => {
    if (savingRef.current) return;
    savingRef.current = true;
    const request = ++revision.current;
    setSaving(true);
    setError(null);
    try {
      const saved = await updateGatewayPrivacySettings({ enabled });
      if (request === revision.current) setSettings(saved);
    } catch (error) {
      if (request === revision.current) setError(String(error));
    } finally {
      savingRef.current = false;
      if (request === revision.current) setSaving(false);
    }
  };

  const count = settings ? settings.rules.builtins.length + settings.rules.custom.filter((rule) => rule.enabled).length : 0;
  return (
    <div className={styles.settings}>
      <div className={styles.settingHeader}>
        <div className={styles.heading}>
          <span id="gateway-privacy-toggle-label" className={styles.label}>{t('gateway.privacy.enable')}</span>
          <p className={styles.helper}>{t('gateway.privacy.description')}</p>
        </div>
        <div className={styles.actions}>
          <span className={styles.state} role="status">
            {loading ? t('gateway.privacy.loading') : !settings ? t('gateway.privacy.unavailable') : settings.enabled
              ? running ? t('gateway.privacy.active') : t('gateway.privacy.waiting')
              : t('gateway.privacy.disabled')}
          </span>
          <Switch size="small" aria-labelledby="gateway-privacy-toggle-label" checked={settings?.enabled ?? false}
            disabled={loading || !settings || editing} loading={saving} onChange={(enabled) => { void toggle(enabled); }} />
        </div>
      </div>
      <div className={styles.row}>
        <span className={styles.summary}>{settings ? t('gateway.privacy.summary', { count, allowed: settings.rules.allowlist.length }) : '—'}</span>
        <button type="button" className={styles.manageButton} disabled={!settings || loading || saving} onClick={() => setEditing(true)}>
          <SlidersHorizontal size={13} aria-hidden="true" />
          <span>{t('gateway.privacy.manage')}</span>
          <ChevronRight size={12} aria-hidden="true" />
        </button>
      </div>
      {error && <div className={styles.error} role="alert">
        {t('gateway.privacy.operationFailed', { error })}
        {!settings && <Button className={styles.actionButton} onClick={() => { void load(); }}>{t('gateway.privacy.retry')}</Button>}
      </div>}
      {settings && editing && <GatewayPrivacyRulesModal rules={settings.rules} onClose={() => setEditing(false)}
        onSaved={(saved) => { revision.current += 1; setSettings(saved); setError(null); setEditing(false); }} />}
    </div>
  );
};

export default GatewayPrivacySettings;
