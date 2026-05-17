import React from 'react';
import {
  Activity,
  AlertCircle,
  CheckCircle2,
  CircleHelp,
  Copy,
  FileText,
  Gauge,
  Loader2,
  Network,
  Power,
  RefreshCw,
  Save,
  Shield,
  Square,
  Terminal,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  checkProxyGatewayHealth,
  checkProxyGatewayPortAvailable,
  getProxyGatewayCliStatuses,
  getProxyGatewaySettings,
  getProxyGatewayStatus,
  preflightStopProxyGateway,
  startProxyGateway,
  stopProxyGateway,
  updateProxyGatewaySettings,
  type GatewayCliTakeoverStatus,
  type GatewayCliKey,
  type ProxyGatewayHealthCheckResult,
  type ProxyGatewaySettings,
  type ProxyGatewayStatus,
} from '@/services';
import styles from './GatewaySettingsPanel.module.less';

type BusyAction = 'load' | 'save' | 'start' | 'stop' | 'health' | 'port' | 'copy';
type NoticeKind = 'success' | 'error' | 'info';
type SupportedGatewayCliKey = Extract<GatewayCliKey, 'claude' | 'codex' | 'gemini'>;

interface NoticeState {
  kind: NoticeKind;
  text: string;
}

interface CliOption {
  key: SupportedGatewayCliKey;
  labelKey: string;
}

const CLI_OPTIONS: CliOption[] = [
  {
    key: 'claude',
    labelKey: 'settings.gateway.cli.claude',
  },
  {
    key: 'codex',
    labelKey: 'settings.gateway.cli.codex',
  },
  {
    key: 'gemini',
    labelKey: 'settings.gateway.cli.gemini',
  },
];

const joinClassNames = (...classNames: Array<string | false | null | undefined>) =>
  classNames.filter(Boolean).join(' ');

const cloneGatewaySettings = (settings: ProxyGatewaySettings): ProxyGatewaySettings => ({
  ...settings,
  enabled_cli_keys: [...settings.enabled_cli_keys],
});

const toInteger = (value: string, fallback: number, minimum = 0) => {
  const nextValue = Number(value);
  if (!Number.isFinite(nextValue)) {
    return fallback;
  }
  return Math.max(minimum, Math.trunc(nextValue));
};

const formatGatewayError = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

const deriveRequestLogLevel = (settings: ProxyGatewaySettings) => {
  if (!settings.request_log_enabled) {
    return 'off';
  }
  if (settings.store_request_body && settings.store_headers && settings.store_response_body) {
    return 'full';
  }
  if (settings.store_request_body || settings.store_response_body) {
    return 'body';
  }
  if (settings.store_headers) {
    return 'headers';
  }
  return 'summary';
};

const isCliTakeoverActive = (status: GatewayCliTakeoverStatus | undefined) =>
  Boolean(status?.can_restore_direct);

interface GatewayButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  icon?: React.ReactNode;
  variant?: 'default' | 'primary' | 'danger' | 'ghost';
  busy?: boolean;
}

const GatewayButton: React.FC<GatewayButtonProps> = ({
  icon,
  variant = 'default',
  busy,
  children,
  className,
  disabled,
  ...buttonProps
}) => (
  <button
    {...buttonProps}
    type={buttonProps.type ?? 'button'}
    disabled={disabled || busy}
    className={joinClassNames(styles.button, styles[`button_${variant}`], className)}
  >
    {busy ? <Loader2 size={14} className={styles.spin} aria-hidden="true" /> : icon}
    {children}
  </button>
);

interface SwitchControlProps {
  checked: boolean;
  disabled?: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}

const SwitchControl: React.FC<SwitchControlProps> = ({ checked, disabled, label, onChange }) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    disabled={disabled}
    className={joinClassNames(styles.switchControl, checked && styles.switchControlChecked)}
    onClick={() => onChange(!checked)}
  >
    <span className={styles.switchTrack} aria-hidden="true">
      <span className={styles.switchThumb} />
    </span>
    <span className={styles.switchLabel}>{label}</span>
  </button>
);

interface FieldRowProps {
  label: string;
  description?: string;
  help?: string;
  children: React.ReactNode;
}

const FieldRow: React.FC<FieldRowProps> = ({ label, description, help, children }) => (
  <div className={styles.fieldRow}>
    <div className={styles.fieldMeta}>
      <span className={styles.fieldLabelRow}>
        <span className={styles.fieldLabel}>{label}</span>
        {help ? (
          <span className={styles.fieldHelpButton} tabIndex={0} aria-label={help}>
            <CircleHelp size={12} aria-hidden="true" />
            <span className={styles.fieldHelpBubble} role="tooltip">
              {help}
            </span>
          </span>
        ) : null}
      </span>
      {description ? <span className={styles.fieldDescription}>{description}</span> : null}
    </div>
    <div className={styles.fieldControl}>{children}</div>
  </div>
);

interface SectionProps {
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
}

const Section: React.FC<SectionProps> = ({ icon, title, children }) => (
  <section className={styles.section}>
    <div className={styles.sectionHeader}>
      <span className={styles.sectionIcon}>{icon}</span>
      <h3>{title}</h3>
    </div>
    {children}
  </section>
);

interface GatewaySettingsPanelProps {
  showTitleBlock?: boolean;
}

const GatewaySettingsPanel: React.FC<GatewaySettingsPanelProps> = ({ showTitleBlock = true }) => {
  const { t } = useTranslation();
  const [savedSettings, setSavedSettings] = React.useState<ProxyGatewaySettings | null>(null);
  const [draftSettings, setDraftSettings] = React.useState<ProxyGatewaySettings | null>(null);
  const [status, setStatus] = React.useState<ProxyGatewayStatus | null>(null);
  const [cliStatuses, setCliStatuses] = React.useState<GatewayCliTakeoverStatus[]>([]);
  const [health, setHealth] = React.useState<ProxyGatewayHealthCheckResult | null>(null);
  const [busyAction, setBusyAction] = React.useState<BusyAction | null>('load');
  const [notice, setNotice] = React.useState<NoticeState | null>(null);

  const updateDraftSetting = React.useCallback(
    <K extends keyof ProxyGatewaySettings>(key: K, value: ProxyGatewaySettings[K]) => {
      setDraftSettings((previousSettings) =>
        previousSettings ? { ...previousSettings, [key]: value } : previousSettings,
      );
    },
    [],
  );

  const refreshGatewayState = React.useCallback(async () => {
    const [nextSettings, nextStatus, nextCliStatuses] = await Promise.all([
      getProxyGatewaySettings(),
      getProxyGatewayStatus(),
      getProxyGatewayCliStatuses(),
    ]);
    setSavedSettings(nextSettings);
    setDraftSettings(cloneGatewaySettings(nextSettings));
    setStatus(nextStatus);
    setCliStatuses(nextCliStatuses);
  }, []);

  React.useEffect(() => {
    let disposed = false;

    const loadGateway = async () => {
      setBusyAction('load');
      try {
        const [nextSettings, nextStatus, nextCliStatuses] = await Promise.all([
          getProxyGatewaySettings(),
          getProxyGatewayStatus(),
          getProxyGatewayCliStatuses(),
        ]);
        if (disposed) {
          return;
        }
        setSavedSettings(nextSettings);
        setDraftSettings(cloneGatewaySettings(nextSettings));
        setStatus(nextStatus);
        setCliStatuses(nextCliStatuses);
      } catch (error) {
        if (!disposed) {
          setNotice({
            kind: 'error',
            text: t('settings.gateway.notice.loadFailed', { error: formatGatewayError(error) }),
          });
        }
      } finally {
        if (!disposed) {
          setBusyAction(null);
        }
      }
    };

    void loadGateway();

    return () => {
      disposed = true;
    };
  }, [t]);

  const hasUnsavedChanges = React.useMemo(() => {
    if (!draftSettings || !savedSettings) {
      return false;
    }
    return JSON.stringify(draftSettings) !== JSON.stringify(savedSettings);
  }, [draftSettings, savedSettings]);

  const gatewayOrigin = React.useMemo(() => {
    if (status?.base_url) {
      return status.base_url;
    }
    if (draftSettings) {
      return `http://${draftSettings.listen_host}:${draftSettings.listen_port}`;
    }
    return null;
  }, [draftSettings, status]);

  const statusKind = status?.running
    ? 'running'
    : status?.last_error
      ? 'error'
      : 'stopped';

  const cliStatusByKey = React.useMemo(() => {
    const entries = cliStatuses.map((cliStatus) => [cliStatus.cli_key, cliStatus] as const);
    return Object.fromEntries(entries) as Partial<Record<SupportedGatewayCliKey, GatewayCliTakeoverStatus>>;
  }, [cliStatuses]);

  const handleSave = async () => {
    if (!draftSettings) {
      return;
    }
    setBusyAction('save');
    try {
      const nextSettings = await updateProxyGatewaySettings({
        ...draftSettings,
        enabled_on_startup: status?.running ? true : draftSettings.enabled_on_startup,
      });
      setSavedSettings(nextSettings);
      setDraftSettings(cloneGatewaySettings(nextSettings));
      setNotice({ kind: 'success', text: t('settings.gateway.notice.saved') });
    } catch (error) {
      setNotice({
        kind: 'error',
        text: t('settings.gateway.notice.saveFailed', { error: formatGatewayError(error) }),
      });
    } finally {
      setBusyAction(null);
    }
  };

  const handleStart = async () => {
    if (!draftSettings) {
      return;
    }
    setBusyAction('start');
    try {
      const nextSettings = await updateProxyGatewaySettings({
        ...draftSettings,
        enabled_on_startup: false,
      });
      const nextStatus = await startProxyGateway(nextSettings);
      const persistedSettings = await getProxyGatewaySettings();
      setSavedSettings(persistedSettings);
      setDraftSettings(cloneGatewaySettings(persistedSettings));
      setStatus(nextStatus);
      setCliStatuses(await getProxyGatewayCliStatuses());
      setNotice({ kind: 'success', text: t('settings.gateway.notice.started') });
    } catch (error) {
      setNotice({
        kind: 'error',
        text: t('settings.gateway.notice.startFailed', { error: formatGatewayError(error) }),
      });
      try {
        setStatus(await getProxyGatewayStatus());
      } catch {
        // Best effort refresh only.
      }
    } finally {
      setBusyAction(null);
    }
  };

  const handleStop = async () => {
    setBusyAction('stop');
    try {
      const preflight = await preflightStopProxyGateway();
      if (!preflight.allowed) {
        setCliStatuses((previousStatuses) => {
          const nextByKey = new Map(previousStatuses.map((cliStatus) => [cliStatus.cli_key, cliStatus]));
          for (const blockingStatus of preflight.blocking_cli_takeovers) {
            nextByKey.set(blockingStatus.cli_key, blockingStatus);
          }
          return Array.from(nextByKey.values());
        });
        const blockingNames = preflight.blocking_cli_takeovers
          .map((cliStatus) => t(`settings.gateway.cli.${cliStatus.cli_key}`))
          .join(', ');
        setNotice({
          kind: 'error',
          text: t('settings.gateway.notice.stopBlockedByCli', { cli: blockingNames || '-' }),
        });
        return;
      }
      const nextStatus = await stopProxyGateway();
      setStatus(nextStatus);
      setHealth(null);
      const nextSettings = await getProxyGatewaySettings();
      setSavedSettings(nextSettings);
      setDraftSettings(cloneGatewaySettings(nextSettings));
      setNotice({ kind: 'success', text: t('settings.gateway.notice.stopped') });
      setCliStatuses(await getProxyGatewayCliStatuses());
    } catch (error) {
      setNotice({
        kind: 'error',
        text: t('settings.gateway.notice.stopFailed', { error: formatGatewayError(error) }),
      });
    } finally {
      setBusyAction(null);
    }
  };

  const handleHealthCheck = async () => {
    setBusyAction('health');
    try {
      const nextHealth = await checkProxyGatewayHealth();
      setHealth(nextHealth);
      setNotice({
        kind: nextHealth.ok ? 'success' : 'error',
        text: nextHealth.ok
          ? t('settings.gateway.notice.healthOk', { statusCode: nextHealth.status_code ?? '-' })
          : t('settings.gateway.notice.healthFailed', { error: nextHealth.error ?? '-' }),
      });
    } catch (error) {
      setNotice({
        kind: 'error',
        text: t('settings.gateway.notice.healthFailed', { error: formatGatewayError(error) }),
      });
    } finally {
      setBusyAction(null);
    }
  };

  const handleCheckPort = async () => {
    if (!draftSettings) {
      return;
    }
    setBusyAction('port');
    try {
      const result = await checkProxyGatewayPortAvailable({
        listen_host: draftSettings.listen_host,
        listen_port: draftSettings.listen_port,
      });
      setNotice({
        kind: result.available ? 'success' : 'error',
        text: result.available
          ? t('settings.gateway.notice.portAvailable', { port: result.listen_port })
          : t('settings.gateway.notice.portOccupied', { port: result.listen_port }),
      });
    } catch (error) {
      setNotice({
        kind: 'error',
        text: t('settings.gateway.notice.portCheckFailed', { error: formatGatewayError(error) }),
      });
    } finally {
      setBusyAction(null);
    }
  };

  const handleCopyGatewayOrigin = async () => {
    if (!gatewayOrigin) {
      return;
    }
    setBusyAction('copy');
    try {
      await navigator.clipboard.writeText(gatewayOrigin);
      setNotice({ kind: 'success', text: t('settings.gateway.notice.copied') });
    } catch (error) {
      setNotice({
        kind: 'error',
        text: t('settings.gateway.notice.copyFailed', { error: formatGatewayError(error) }),
      });
    } finally {
      setBusyAction(null);
    }
  };

  const handleLogPartToggle = (
    key: 'store_request_body' | 'store_headers' | 'store_response_body',
    checked: boolean,
  ) => {
    if (!draftSettings) {
      return;
    }
    const nextSettings = { ...draftSettings, [key]: checked };
    nextSettings.request_log_level = deriveRequestLogLevel(nextSettings);
    setDraftSettings(nextSettings);
  };

  const handleRequestLogEnabledToggle = (checked: boolean) => {
    if (!draftSettings) {
      return;
    }
    const nextSettings = { ...draftSettings, request_log_enabled: checked };
    nextSettings.request_log_level = deriveRequestLogLevel(nextSettings);
    setDraftSettings(nextSettings);
  };

  if (!draftSettings) {
    return (
      <div className={styles.loadingState}>
        <Loader2 size={18} className={styles.spin} aria-hidden="true" />
        <span>{t('settings.gateway.loading')}</span>
      </div>
    );
  }

  return (
    <div className={styles.panel}>
      <div className={joinClassNames(styles.topBar, !showTitleBlock && styles.topBarActionsOnly)}>
        {showTitleBlock ? (
          <div className={styles.titleBlock}>
            <span className={styles.titleIcon}>
              <Network size={18} aria-hidden="true" />
            </span>
            <div>
              <h2>{t('settings.gateway.title')}</h2>
              <p>{t('settings.gateway.subtitle')}</p>
            </div>
          </div>
        ) : null}

        <div className={styles.actionBar}>
          <GatewayButton
            icon={<Save size={14} aria-hidden="true" />}
            busy={busyAction === 'save'}
            disabled={!hasUnsavedChanges}
            onClick={() => void handleSave()}
          >
            {t('common.save')}
          </GatewayButton>
          {status?.running ? (
            <GatewayButton
              icon={<Square size={13} aria-hidden="true" />}
              busy={busyAction === 'stop'}
              onClick={() => void handleStop()}
            >
              {t('settings.gateway.actions.stop')}
            </GatewayButton>
          ) : (
            <GatewayButton
              variant="primary"
              icon={<Power size={14} aria-hidden="true" />}
              busy={busyAction === 'start'}
              onClick={() => void handleStart()}
            >
              {t('settings.gateway.actions.start')}
            </GatewayButton>
          )}
          <GatewayButton
            variant="ghost"
            icon={<Activity size={14} aria-hidden="true" />}
            busy={busyAction === 'health'}
            onClick={() => void handleHealthCheck()}
          >
            {t('settings.gateway.actions.health')}
          </GatewayButton>
          <GatewayButton
            variant="ghost"
            icon={<RefreshCw size={14} aria-hidden="true" />}
            busy={busyAction === 'load'}
            onClick={() => {
              setBusyAction('load');
              refreshGatewayState()
                .then(() => setNotice({ kind: 'success', text: t('settings.gateway.notice.refreshed') }))
                .catch((error) =>
                  setNotice({
                    kind: 'error',
                    text: t('settings.gateway.notice.loadFailed', { error: formatGatewayError(error) }),
                  }),
                )
                .finally(() => setBusyAction(null));
            }}
          >
            {t('common.refresh')}
          </GatewayButton>
        </div>
      </div>

      <div className={styles.statusGrid}>
        <div className={styles.statusItem}>
          <span className={styles.statusLabel}>{t('settings.gateway.status.state')}</span>
          <span className={joinClassNames(styles.statusBadge, styles[`statusBadge_${statusKind}`])}>
            {statusKind === 'running' ? (
              <CheckCircle2 size={14} aria-hidden="true" />
            ) : statusKind === 'error' ? (
              <AlertCircle size={14} aria-hidden="true" />
            ) : (
              <Square size={13} aria-hidden="true" />
            )}
            {t(`settings.gateway.status.${statusKind}`)}
          </span>
        </div>
        <div className={styles.statusItemWide}>
          <span className={styles.statusLabel}>{t('settings.gateway.status.address')}</span>
          <div className={styles.addressRow}>
            <code>{gatewayOrigin ?? '-'}</code>
            <button
              type="button"
              className={styles.iconButton}
              disabled={!gatewayOrigin || busyAction === 'copy'}
              aria-label={t('settings.gateway.actions.copyAddress')}
              title={t('settings.gateway.actions.copyAddress')}
              onClick={() => void handleCopyGatewayOrigin()}
            >
              {busyAction === 'copy' ? (
                <Loader2 size={14} className={styles.spin} aria-hidden="true" />
              ) : (
                <Copy size={14} aria-hidden="true" />
              )}
            </button>
          </div>
        </div>
        <div className={styles.statusItem}>
          <span className={styles.statusLabel}>{t('settings.gateway.status.health')}</span>
          <span className={joinClassNames(styles.healthText, health?.ok === false && styles.healthTextError)}>
            {health ? (
              health.ok
                ? t('settings.gateway.status.healthOk', { statusCode: health.status_code ?? '-' })
                : t('settings.gateway.status.healthFailed')
            ) : (
              t('settings.gateway.status.healthUnknown')
            )}
          </span>
        </div>
      </div>

      {status?.last_error ? (
        <div className={styles.inlineAlert} role="alert">
          <AlertCircle size={14} aria-hidden="true" />
          <span>{status.last_error}</span>
        </div>
      ) : null}

      {notice ? (
        <div className={joinClassNames(styles.notice, styles[`notice_${notice.kind}`])} role="status" aria-live="polite">
          {notice.text}
        </div>
      ) : null}

      <div className={styles.contentGrid}>
        <Section icon={<Network size={15} aria-hidden="true" />} title={t('settings.gateway.sections.listen')}>
          <div className={styles.fieldStack}>
            <FieldRow label={t('settings.gateway.fields.host')} description={t('settings.gateway.hints.host')}>
              <input
                className={styles.textInput}
                value={draftSettings.listen_host}
                onChange={(event) => updateDraftSetting('listen_host', event.currentTarget.value)}
              />
            </FieldRow>
            <FieldRow label={t('settings.gateway.fields.port')} description={t('settings.gateway.hints.port')}>
              <div className={styles.inlineControlGroup}>
                <input
                  className={styles.numberInput}
                  type="number"
                  min={1024}
                  value={draftSettings.listen_port}
                  onChange={(event) =>
                    updateDraftSetting(
                      'listen_port',
                      toInteger(event.currentTarget.value, draftSettings.listen_port, 0),
                    )
                  }
                />
                <GatewayButton
                  variant="ghost"
                  busy={busyAction === 'port'}
                  onClick={() => void handleCheckPort()}
                >
                  {t('settings.gateway.actions.checkPort')}
                </GatewayButton>
              </div>
            </FieldRow>
            <FieldRow label={t('settings.gateway.fields.autoSelectPort')}>
              <SwitchControl
                checked={draftSettings.port_auto_select}
                label={draftSettings.port_auto_select ? t('common.enabled') : t('common.disabled')}
                onChange={(checked) => updateDraftSetting('port_auto_select', checked)}
              />
            </FieldRow>
          </div>
        </Section>

        <Section icon={<Terminal size={15} aria-hidden="true" />} title={t('settings.gateway.sections.cli')}>
          <div className={styles.cliList}>
            {CLI_OPTIONS.map((option) => {
              const cliStatus = cliStatusByKey[option.key];
              const active = isCliTakeoverActive(cliStatus);
              const dot = cliStatus?.dot ?? 'gray';
              return (
                <div
                  key={option.key}
                  className={joinClassNames(styles.cliRow, active && styles.cliRowActive)}
                  title={cliStatus?.message ?? t('settings.gateway.cliStatus.direct')}
                >
                  <span
                    className={joinClassNames(styles.cliStatusDot, styles[`cliStatusDot_${dot}`])}
                    aria-hidden="true"
                  />
                  <span className={styles.cliName}>{t(option.labelKey)}</span>
                  <span className={styles.cliStateText}>
                    {t(`settings.gateway.cliStatus.${cliStatus?.state ?? 'direct'}`)}
                  </span>
                </div>
              );
            })}
          </div>
        </Section>

        <Section icon={<FileText size={15} aria-hidden="true" />} title={t('settings.gateway.sections.logs')}>
          <div className={styles.fieldStack}>
            <FieldRow label={t('settings.gateway.fields.requestLog')}>
              <SwitchControl
                checked={draftSettings.request_log_enabled}
                label={draftSettings.request_log_enabled ? t('common.enabled') : t('common.disabled')}
                onChange={handleRequestLogEnabledToggle}
              />
            </FieldRow>
            <FieldRow label={t('settings.gateway.fields.metrics')}>
              <SwitchControl
                checked={draftSettings.metrics_enabled}
                label={draftSettings.metrics_enabled ? t('common.enabled') : t('common.disabled')}
                onChange={(checked) => updateDraftSetting('metrics_enabled', checked)}
              />
            </FieldRow>
            <div className={styles.logParts} aria-label={t('settings.gateway.fields.detailStorage')}>
              <label className={styles.checkItem}>
                <input
                  type="checkbox"
                  checked={draftSettings.store_headers}
                  disabled={!draftSettings.request_log_enabled}
                  onChange={(event) => handleLogPartToggle('store_headers', event.currentTarget.checked)}
                />
                <span>{t('settings.gateway.logParts.headers')}</span>
              </label>
              <label className={styles.checkItem}>
                <input
                  type="checkbox"
                  checked={draftSettings.store_request_body}
                  disabled={!draftSettings.request_log_enabled}
                  onChange={(event) => handleLogPartToggle('store_request_body', event.currentTarget.checked)}
                />
                <span>{t('settings.gateway.logParts.requestBody')}</span>
              </label>
              <label className={styles.checkItem}>
                <input
                  type="checkbox"
                  checked={draftSettings.store_response_body}
                  disabled={!draftSettings.request_log_enabled}
                  onChange={(event) => handleLogPartToggle('store_response_body', event.currentTarget.checked)}
                />
                <span>{t('settings.gateway.logParts.response')}</span>
              </label>
            </div>
            <FieldRow label={t('settings.gateway.fields.retentionDays')}>
              <input
                className={styles.numberInput}
                type="number"
                min={1}
                value={draftSettings.log_retention_days}
                onChange={(event) =>
                  updateDraftSetting(
                    'log_retention_days',
                    toInteger(event.currentTarget.value, draftSettings.log_retention_days, 1),
                  )
                }
              />
            </FieldRow>
            <FieldRow label={t('settings.gateway.fields.maxDirSize')}>
              <input
                className={styles.numberInput}
                type="number"
                min={1}
                value={draftSettings.log_max_dir_size_mb}
                onChange={(event) =>
                  updateDraftSetting(
                    'log_max_dir_size_mb',
                    toInteger(event.currentTarget.value, draftSettings.log_max_dir_size_mb, 1),
                  )
                }
              />
            </FieldRow>
            <FieldRow label={t('settings.gateway.fields.maxBodySize')}>
              <input
                className={styles.numberInput}
                type="number"
                min={1}
                value={draftSettings.log_max_body_size_kb}
                onChange={(event) =>
                  updateDraftSetting(
                    'log_max_body_size_kb',
                    toInteger(event.currentTarget.value, draftSettings.log_max_body_size_kb, 1),
                  )
                }
              />
            </FieldRow>
          </div>
        </Section>

        <Section icon={<Shield size={15} aria-hidden="true" />} title={t('settings.gateway.sections.circuitBreaker')}>
          <div className={styles.fieldStack}>
            <FieldRow
              label={t('settings.gateway.fields.failureThreshold')}
              help={t('settings.gateway.fieldHelp.failureThreshold')}
            >
              <input
                className={styles.numberInput}
                type="number"
                min={1}
                value={draftSettings.model_failure_score_threshold}
                onChange={(event) =>
                  updateDraftSetting(
                    'model_failure_score_threshold',
                    toInteger(event.currentTarget.value, draftSettings.model_failure_score_threshold, 1),
                  )
                }
              />
            </FieldRow>
            <FieldRow
              label={t('settings.gateway.fields.failureWindow')}
              help={t('settings.gateway.fieldHelp.failureWindow')}
            >
              <input
                className={styles.numberInput}
                type="number"
                min={30}
                value={draftSettings.model_failure_window_seconds}
                onChange={(event) =>
                  updateDraftSetting(
                    'model_failure_window_seconds',
                    toInteger(event.currentTarget.value, draftSettings.model_failure_window_seconds, 30),
                  )
                }
              />
            </FieldRow>
            <FieldRow
              label={t('settings.gateway.fields.baseCooldown')}
              help={t('settings.gateway.fieldHelp.baseCooldown')}
            >
              <input
                className={styles.numberInput}
                type="number"
                min={30}
                value={draftSettings.model_base_cooldown_seconds}
                onChange={(event) =>
                  updateDraftSetting(
                    'model_base_cooldown_seconds',
                    toInteger(event.currentTarget.value, draftSettings.model_base_cooldown_seconds, 30),
                  )
                }
              />
            </FieldRow>
            <FieldRow
              label={t('settings.gateway.fields.maxCooldown')}
              help={t('settings.gateway.fieldHelp.maxCooldown')}
            >
              <input
                className={styles.numberInput}
                type="number"
                min={60}
                value={draftSettings.model_max_cooldown_seconds}
                onChange={(event) =>
                  updateDraftSetting(
                    'model_max_cooldown_seconds',
                    toInteger(event.currentTarget.value, draftSettings.model_max_cooldown_seconds, 60),
                  )
                }
              />
            </FieldRow>
            <FieldRow
              label={t('settings.gateway.fields.probeSuccess')}
              help={t('settings.gateway.fieldHelp.probeSuccess')}
            >
              <input
                className={styles.numberInput}
                type="number"
                min={1}
                value={draftSettings.half_open_success_required}
                onChange={(event) =>
                  updateDraftSetting(
                    'half_open_success_required',
                    toInteger(event.currentTarget.value, draftSettings.half_open_success_required, 1),
                  )
                }
              />
            </FieldRow>
          </div>
        </Section>
      </div>

      <div className={styles.footerMetrics}>
        <div>
          <Gauge size={14} aria-hidden="true" />
          <span>{t('settings.gateway.metrics.logLevel', { level: deriveRequestLogLevel(draftSettings) })}</span>
        </div>
        <div>
          <FileText size={14} aria-hidden="true" />
          <span>{t('settings.gateway.metrics.logStorage')}</span>
        </div>
      </div>
    </div>
  );
};

export default GatewaySettingsPanel;
