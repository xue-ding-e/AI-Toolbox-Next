import React from 'react';
import { Button, Input, InputNumber, Modal, Select, Switch, Tabs, Tooltip } from 'antd';
import { Plus, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  previewGatewayPrivacy,
  updateGatewayPrivacySettings,
  type GatewayPrivacyPreview,
  type GatewayPrivacyRules,
  type GatewayPrivacySettings,
} from '@/services/gatewayPrivacyApi';
import styles from './GatewayPrivacySettings.module.less';

const BUILTINS = [
  { id: 'credentials', label: 'gateway.privacy.builtins.credentials', hint: 'gateway.privacy.builtins.credentialsHint' },
  { id: 'private_keys', label: 'gateway.privacy.builtins.privateKeys', hint: 'gateway.privacy.builtins.privateKeysHint' },
  { id: 'passwords', label: 'gateway.privacy.builtins.passwords', hint: 'gateway.privacy.builtins.passwordsHint' },
  { id: 'connection_strings', label: 'gateway.privacy.builtins.connections', hint: 'gateway.privacy.builtins.connectionsHint' },
  { id: 'email', label: 'gateway.privacy.builtins.email', hint: 'gateway.privacy.builtins.emailHint' },
  { id: 'ip', label: 'gateway.privacy.builtins.ip', hint: 'gateway.privacy.builtins.ipHint' },
] as const;

interface Props {
  rules: GatewayPrivacyRules;
  onClose: () => void;
  onSaved: (settings: GatewayPrivacySettings) => void;
}

const GatewayPrivacyRulesModal: React.FC<Props> = ({ rules, onClose, onSaved }) => {
  const { t } = useTranslation();
  const [draft, setDraft] = React.useState(() => structuredClone(rules));
  const [allowlistText, setAllowlistText] = React.useState(() => rules.allowlist.join('\n'));
  const [text, setText] = React.useState('');
  const [preview, setPreview] = React.useState<GatewayPrivacyPreview | null>(null);
  const [saving, setSaving] = React.useState(false);
  const [testing, setTesting] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const revision = React.useRef(0);
  const busy = React.useRef(false);
  React.useEffect(() => () => { revision.current += 1; }, []);

  const change = (next: GatewayPrivacyRules) => {
    revision.current += 1;
    setDraft(next);
    setPreview(null);
    setTesting(false);
    setError(null);
  };

  const save = async () => {
    if (busy.current) return;
    busy.current = true;
    revision.current += 1;
    setSaving(true);
    setTesting(false);
    setError(null);
    try { onSaved(await updateGatewayPrivacySettings({ rules: draft })); }
    catch (error) { setError(String(error)); }
    finally { busy.current = false; setSaving(false); }
  };

  const test = async () => {
    const request = ++revision.current;
    setTesting(true);
    setPreview(null);
    setError(null);
    try {
      const result = await previewGatewayPrivacy(draft, text);
      if (request === revision.current) setPreview(result);
    } catch (error) { if (request === revision.current) setError(String(error)); }
    finally { if (request === revision.current) setTesting(false); }
  };

  return <Modal open title={t('gateway.privacy.manage')} width="min(900px, 100%)" destroyOnHidden
    onCancel={onClose} onOk={() => { void save(); }} confirmLoading={saving}
    okText={t('gateway.privacy.save')} cancelText={t('gateway.privacy.cancel')}
    closable={!saving} maskClosable={!saving} keyboard={!saving} cancelButtonProps={{ disabled: saving }}>
    <p className={styles.helper}>{t('gateway.privacy.scopeHint')}</p>
    <Tabs items={[
      { key: 'builtins', label: t('gateway.privacy.tabs.builtins'), children: <div className={styles.list}>
        {BUILTINS.map((rule) => <div key={rule.id} className={styles.row}>
          <div className={styles.grow}><span id={`privacy-builtin-${rule.id}`}>{t(rule.label)}</span><p className={styles.helper}>{t(rule.hint)}</p></div>
          <Switch size="small" aria-labelledby={`privacy-builtin-${rule.id}`} disabled={saving} checked={draft.builtins.includes(rule.id)}
            onChange={(enabled) => change({ ...draft, builtins: enabled ? [...draft.builtins, rule.id] : draft.builtins.filter((id) => id !== rule.id) })} />
        </div>)}
      </div> },
      { key: 'custom', label: t('gateway.privacy.tabs.custom'), children: <div className={styles.list}>
        <p className={styles.helper}>{t('gateway.privacy.regexHint')}</p>
        {draft.custom.length === 0 && <p className={styles.empty}>{t('gateway.privacy.noRules')}</p>}
        {draft.custom.map((rule, index) => {
          const update = (patch: Partial<typeof rule>) => change({ ...draft, custom: draft.custom.map((item) => item.id === rule.id ? { ...item, ...patch } : item) });
          return <div className={styles.rule} key={rule.id}>
            <div className={styles.ruleToolbar}>
              <Switch size="small" aria-label={t('gateway.privacy.ruleEnabled', { index: index + 1 })} checked={rule.enabled} disabled={saving} onChange={(enabled) => update({ enabled })} />
              <Input aria-label={t('gateway.privacy.ruleName')} placeholder={t('gateway.privacy.ruleName')} value={rule.name} disabled={saving} maxLength={200} onChange={(event) => update({ name: event.target.value })} />
              <div className={styles.ruleOptions}>
                <Select aria-label={t('gateway.privacy.ruleKind')} value={rule.kind} disabled={saving} onChange={(kind) => update({ kind })}
                  options={[{ value: 'literal', label: t('gateway.privacy.literal') }, { value: 'regex', label: t('gateway.privacy.regex') }]} />
                <Tooltip title={t('gateway.privacy.priorityHint')}><InputNumber aria-label={t('gateway.privacy.priority')} value={rule.priority} disabled={saving} min={-1000} max={1000} precision={0} onChange={(priority) => update({ priority: priority ?? 0 })} /></Tooltip>
              </div>
              <Tooltip title={t('gateway.privacy.remove')}><Button type="text" aria-label={t('gateway.privacy.remove')} disabled={saving} icon={<Trash2 size={14} />} onClick={() => change({ ...draft, custom: draft.custom.filter((item) => item.id !== rule.id) })} /></Tooltip>
            </div>
            <label className={styles.field}><span>{t('gateway.privacy.pattern')}</span>
              <Input.TextArea rows={2} value={rule.pattern} disabled={saving} maxLength={4096} onChange={(event) => update({ pattern: event.target.value })} />
            </label>
          </div>;
        })}
        <Button className={styles.actionButton} disabled={saving || draft.custom.length >= 100} icon={<Plus size={14} />} onClick={() => change({ ...draft, custom: [...draft.custom, {
          id: crypto.randomUUID(), name: '', enabled: true, kind: 'literal', pattern: '', priority: 200,
        }] })}>{t('gateway.privacy.addRule')}</Button>
      </div> },
      { key: 'allowlist', label: t('gateway.privacy.tabs.allowlist'), children: <div className={styles.list}>
        <p className={styles.helper}>{t('gateway.privacy.allowlistHint')}</p>
        <Input.TextArea aria-label={t('gateway.privacy.tabs.allowlist')} rows={10} disabled={saving} value={allowlistText}
          onChange={(event) => { setAllowlistText(event.target.value); change({ ...draft, allowlist: event.target.value.split(/\r?\n/).filter((value) => value.length > 0) }); }} />
      </div> },
      { key: 'test', label: t('gateway.privacy.tabs.test'), children: <div className={styles.list}>
        <p className={styles.helper}>{t('gateway.privacy.testHint')}</p>
        <Input.TextArea aria-label={t('gateway.privacy.testInput')} rows={5} value={text} disabled={saving} maxLength={65536}
          onChange={(event) => { revision.current += 1; setText(event.target.value); setPreview(null); setTesting(false); setError(null); }} />
        <div><Button className={styles.actionButton} loading={testing} disabled={saving || !text} onClick={() => { void test(); }}>{t('gateway.privacy.testRun')}</Button></div>
        {preview && <>
          <span role="status">{t('gateway.privacy.testMatches', { count: preview.detail.matched_values })}</span>
          <div className={styles.previewGrid}>
            <label><span>{t('gateway.privacy.redacted')}</span><Input.TextArea readOnly rows={7} value={preview.redacted} /></label>
            <label><span>{t('gateway.privacy.restored')}</span><Input.TextArea readOnly rows={7} value={preview.restored} /></label>
          </div>
        </>}
      </div> },
    ]} />
    {error && <p className={styles.error} role="alert">{t('gateway.privacy.validationFailed', { error })}</p>}
    <p className={`${styles.helper} ${styles.footerHint}`}>{t('gateway.privacy.historyHint')}</p>
  </Modal>;
};

export default GatewayPrivacyRulesModal;
