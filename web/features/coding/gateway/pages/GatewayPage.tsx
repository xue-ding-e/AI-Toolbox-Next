import React from 'react';
import {
  Activity,
  AlertCircle,
  BarChart3,
  CheckCircle2,
  FileText,
  Loader2,
  Network,
  Power,
  RefreshCw,
  Settings,
  Square,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useLocation, useNavigate } from 'react-router-dom';
import GatewaySettingsPanel from '@/features/settings/pages/GatewaySettingsPanel';
import {
  checkProxyGatewayHealth,
  getProxyGatewayCliStatuses,
  getProxyGatewaySettings,
  getProxyGatewayStatus,
  preflightStopProxyGateway,
  startProxyGateway,
  stopProxyGateway,
  updateProxyGatewaySettings,
  type ProxyGatewayHealthCheckResult,
  type ProxyGatewaySettings,
  type ProxyGatewayStatus,
} from '@/services';
import GatewayRequestsView from '../components/GatewayRequestsView';
import GatewayStatisticsView from '../components/GatewayStatisticsView';
import { formatGatewayError, joinClassNames } from '../utils/gatewayFormatters';
import {
  DEFAULT_GATEWAY_PATH,
  GATEWAY_TABS,
  getGatewayPathForTab,
  resolveGatewayTabFromPath,
  type GatewayPageTab,
} from '../utils/gatewayNavigation';
import styles from './GatewayPage.module.less';

type GatewayAction = 'load' | 'start' | 'stop' | 'health' | 'refresh';
type GatewayNoticeKind = 'success' | 'error';

const cloneGatewaySettings = (settings: ProxyGatewaySettings): ProxyGatewaySettings => ({
  ...settings,
  enabled_cli_keys: [...settings.enabled_cli_keys],
});

const GatewayPage: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const activeTab = resolveGatewayTabFromPath(location.pathname);
  const [status, setStatus] = React.useState<ProxyGatewayStatus | null>(null);
  const [health, setHealth] = React.useState<ProxyGatewayHealthCheckResult | null>(null);
  const [busyAction, setBusyAction] = React.useState<GatewayAction | null>('load');
  const [notice, setNotice] = React.useState<{ kind: GatewayNoticeKind; text: string } | null>(null);
  const [contentRefreshKey, setContentRefreshKey] = React.useState(0);
  const settingsDraftRef = React.useRef<ProxyGatewaySettings | null>(null);

  React.useEffect(() => {
    if (location.pathname === '/gateway') {
      navigate(DEFAULT_GATEWAY_PATH, { replace: true });
    }
  }, [location.pathname, navigate]);

  const refreshGatewayState = React.useCallback(async () => {
    const nextStatus = await getProxyGatewayStatus();
    setStatus(nextStatus);
    await Promise.all([
      getProxyGatewaySettings(),
      getProxyGatewayCliStatuses(),
    ]);
    return nextStatus;
  }, []);

  React.useEffect(() => {
    let disposed = false;

    const loadGatewayState = async () => {
      setBusyAction('load');
      try {
        const nextStatus = await getProxyGatewayStatus();
        if (!disposed) {
          setStatus(nextStatus);
        }
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

    void loadGatewayState();

    return () => {
      disposed = true;
    };
  }, [t]);

  const handleTabChange = (tabKey: GatewayPageTab) => {
    navigate(getGatewayPathForTab(tabKey));
  };

  const handleSettingsDraftChange = React.useCallback((settings: ProxyGatewaySettings | null) => {
    settingsDraftRef.current = settings ? cloneGatewaySettings(settings) : null;
  }, []);

  const handleStart = async () => {
    setBusyAction('start');
    try {
      const settings = settingsDraftRef.current
        ? cloneGatewaySettings(settingsDraftRef.current)
        : await getProxyGatewaySettings();
      const nextSettings = await updateProxyGatewaySettings({
        ...settings,
        enabled_on_startup: false,
      });
      const nextStatus = await startProxyGateway(nextSettings);
      setStatus(nextStatus);
      setContentRefreshKey((currentKey) => currentKey + 1);
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
      setContentRefreshKey((currentKey) => currentKey + 1);
      setNotice({ kind: 'success', text: t('settings.gateway.notice.stopped') });
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

  const handleRefresh = async () => {
    setBusyAction('refresh');
    try {
      await refreshGatewayState();
      setContentRefreshKey((currentKey) => currentKey + 1);
      setNotice({ kind: 'success', text: t('settings.gateway.notice.refreshed') });
    } catch (error) {
      setNotice({
        kind: 'error',
        text: t('settings.gateway.notice.loadFailed', { error: formatGatewayError(error) }),
      });
    } finally {
      setBusyAction(null);
    }
  };

  const statusKind = status?.running
    ? 'running'
    : status?.last_error
      ? 'error'
      : 'stopped';

  return (
    <div className={styles.gatewayPage}>
      <div className={styles.header}>
        <div className={styles.titleBlock}>
          <span className={styles.titleIcon}>
            <Network size={18} aria-hidden="true" />
          </span>
          <div>
            <h1>{t('gateway.page.title')}</h1>
            <p>{t('gateway.page.subtitle')}</p>
          </div>
        </div>
        <div className={styles.statusStrip}>
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
          <span className={joinClassNames(styles.healthBadge, health?.ok === false && styles.healthBadgeError)}>
            {health
              ? health.ok
                ? t('settings.gateway.status.healthOk', { statusCode: health.status_code ?? '-' })
                : t('settings.gateway.status.healthFailed')
              : t('settings.gateway.status.healthUnknown')}
          </span>
        </div>
      </div>

      <div className={styles.controlBar}>
        <div className={styles.actionBar}>
          {status?.running ? (
            <button
              type="button"
              className={styles.actionButton}
              disabled={Boolean(busyAction)}
              onClick={() => void handleStop()}
            >
              {busyAction === 'stop' ? (
                <Loader2 size={14} className={styles.spin} aria-hidden="true" />
              ) : (
                <Square size={13} aria-hidden="true" />
              )}
              <span>{t('settings.gateway.actions.stop')}</span>
            </button>
          ) : (
            <button
              type="button"
              className={joinClassNames(styles.actionButton, styles.actionButtonPrimary)}
              disabled={Boolean(busyAction)}
              onClick={() => void handleStart()}
            >
              {busyAction === 'start' ? (
                <Loader2 size={14} className={styles.spin} aria-hidden="true" />
              ) : (
                <Power size={14} aria-hidden="true" />
              )}
              <span>{t('settings.gateway.actions.start')}</span>
            </button>
          )}
          <button
            type="button"
            className={styles.actionButton}
            disabled={Boolean(busyAction)}
            onClick={() => void handleHealthCheck()}
          >
            {busyAction === 'health' ? (
              <Loader2 size={14} className={styles.spin} aria-hidden="true" />
            ) : (
              <Activity size={14} aria-hidden="true" />
            )}
            <span>{t('settings.gateway.actions.health')}</span>
          </button>
          <button
            type="button"
            className={styles.actionButton}
            disabled={Boolean(busyAction)}
            onClick={() => void handleRefresh()}
          >
            {busyAction === 'refresh' || busyAction === 'load' ? (
              <Loader2 size={14} className={styles.spin} aria-hidden="true" />
            ) : (
              <RefreshCw size={14} aria-hidden="true" />
            )}
            <span>{t('common.refresh')}</span>
          </button>
        </div>
        <div className={styles.tabList} role="tablist" aria-label={t('gateway.page.title')}>
          {GATEWAY_TABS.map((tab) => (
            <button
              key={tab.key}
              type="button"
              role="tab"
              aria-selected={activeTab === tab.key}
              className={joinClassNames(styles.tabButton, activeTab === tab.key && styles.tabButtonActive)}
              onClick={() => handleTabChange(tab.key)}
            >
              {tab.key === 'statistics' ? <BarChart3 size={14} aria-hidden="true" /> : null}
              {tab.key === 'requests' ? <FileText size={14} aria-hidden="true" /> : null}
              {tab.key === 'settings' ? <Settings size={14} aria-hidden="true" /> : null}
              <span>{t(tab.labelKey)}</span>
            </button>
          ))}
        </div>
      </div>

      {notice ? (
        <div className={joinClassNames(styles.notice, styles[`notice_${notice.kind}`])} role="status" aria-live="polite">
          {notice.text}
        </div>
      ) : null}
      {status?.last_error ? (
        <div className={joinClassNames(styles.notice, styles.notice_error)} role="alert">
          {status.last_error}
        </div>
      ) : null}

      {activeTab === 'statistics' ? <GatewayStatisticsView key={`statistics-${contentRefreshKey}`} /> : null}
      {activeTab === 'requests' ? <GatewayRequestsView key={`requests-${contentRefreshKey}`} /> : null}
      {activeTab === 'settings' ? (
        <GatewaySettingsPanel
          key={`settings-${contentRefreshKey}`}
          showTitleBlock={false}
          onStatusChange={setStatus}
          onDraftSettingsChange={handleSettingsDraftChange}
        />
      ) : null}
    </div>
  );
};

export default GatewayPage;
