import React from 'react';
import {
  Activity,
  AlertCircle,
  BarChart3,
  CheckCircle2,
  Clock3,
  FileText,
  Gauge,
  Network,
  RefreshCw,
  Settings,
  Shield,
  Terminal,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useLocation, useNavigate } from 'react-router-dom';
import GatewaySettingsPanel from '@/features/settings/pages/GatewaySettingsPanel';
import {
  checkProxyGatewayHealth,
  getProxyGatewayCliStatuses,
  getProxyGatewaySettings,
  getProxyGatewayStatus,
  type GatewayCliTakeoverStatus,
  type ProxyGatewayHealthCheckResult,
  type ProxyGatewaySettings,
  type ProxyGatewayStatus,
} from '@/services';
import {
  DEFAULT_GATEWAY_PATH,
  GATEWAY_TABS,
  getGatewayPathForTab,
  resolveGatewayTabFromPath,
  type GatewayPageTab,
} from '../utils/gatewayNavigation';
import styles from './GatewayPage.module.less';

interface GatewaySummaryState {
  settings: ProxyGatewaySettings | null;
  status: ProxyGatewayStatus | null;
  health: ProxyGatewayHealthCheckResult | null;
  cliStatuses: GatewayCliTakeoverStatus[];
}

const joinClassNames = (...classNames: Array<string | false | null | undefined>) =>
  classNames.filter(Boolean).join(' ');

const formatGatewayError = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

const deriveRequestLogLevel = (settings: ProxyGatewaySettings | null) => {
  if (!settings?.request_log_enabled) {
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

const buildGatewayOrigin = (status: ProxyGatewayStatus | null) => {
  if (!status) {
    return '-';
  }
  if (status.base_url) {
    return status.base_url;
  }
  return status.listen_port ? `http://${status.listen_host}:${status.listen_port}` : '-';
};

interface StatTileProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  tone?: 'default' | 'success' | 'error' | 'muted';
  meta?: string;
}

const StatTile: React.FC<StatTileProps> = ({ icon, label, value, tone = 'default', meta }) => (
  <section className={styles.statTile}>
    <div className={styles.statIcon}>{icon}</div>
    <div className={styles.statBody}>
      <span className={styles.statLabel}>{label}</span>
      <span className={joinClassNames(styles.statValue, styles[`statValue_${tone}`])}>{value}</span>
      {meta ? <span className={styles.statMeta}>{meta}</span> : null}
    </div>
  </section>
);

interface GatewayStatisticsViewProps {
  state: GatewaySummaryState;
  loading: boolean;
  error: string | null;
  onRefresh: () => void;
}

const GatewayStatisticsView: React.FC<GatewayStatisticsViewProps> = ({ state, loading, error, onRefresh }) => {
  const { t } = useTranslation();
  const statusKind = state.status?.running ? 'running' : state.status?.last_error ? 'error' : 'stopped';
  const activeCliCount = state.cliStatuses.filter((cliStatus) => cliStatus.can_restore_direct).length;
  const logLevel = deriveRequestLogLevel(state.settings);

  return (
    <div className={styles.viewStack}>
      <div className={styles.viewToolbar}>
        <div>
          <h2>{t('gateway.page.statistics.title')}</h2>
          <p>{t('gateway.page.statistics.subtitle')}</p>
        </div>
        <button type="button" className={styles.toolButton} disabled={loading} onClick={onRefresh}>
          <RefreshCw size={14} className={loading ? styles.spin : undefined} aria-hidden="true" />
          <span>{t('common.refresh')}</span>
        </button>
      </div>

      {error ? (
        <div className={styles.inlineAlert} role="alert">
          <AlertCircle size={14} aria-hidden="true" />
          <span>{error}</span>
        </div>
      ) : null}

      <div className={styles.statGrid}>
        <StatTile
          icon={statusKind === 'running' ? <CheckCircle2 size={15} /> : <Network size={15} />}
          label={t('gateway.page.statistics.state')}
          value={t(`settings.gateway.status.${statusKind}`)}
          tone={statusKind === 'running' ? 'success' : statusKind === 'error' ? 'error' : 'muted'}
          meta={buildGatewayOrigin(state.status)}
        />
        <StatTile
          icon={<Activity size={15} />}
          label={t('gateway.page.statistics.health')}
          value={
            state.health
              ? state.health.ok
                ? t('settings.gateway.status.healthOk', { statusCode: state.health.status_code ?? '-' })
                : t('settings.gateway.status.healthFailed')
              : t('settings.gateway.status.healthUnknown')
          }
          tone={state.health?.ok ? 'success' : state.health?.ok === false ? 'error' : 'muted'}
          meta={state.health?.error ?? undefined}
        />
        <StatTile
          icon={<Terminal size={15} />}
          label={t('gateway.page.statistics.takeover')}
          value={t('gateway.page.statistics.takeoverCount', { count: activeCliCount })}
          meta={t('settings.gateway.sections.cli')}
        />
        <StatTile
          icon={<FileText size={15} />}
          label={t('gateway.page.statistics.requestLog')}
          value={t(`gateway.page.logLevels.${logLevel}`)}
          tone={logLevel === 'off' ? 'muted' : 'default'}
          meta={state.settings?.metrics_enabled ? t('settings.gateway.fields.metrics') : undefined}
        />
      </div>

      <div className={styles.dataPanels}>
        <section className={styles.dataPanel}>
          <div className={styles.panelHeader}>
            <span>
              <BarChart3 size={14} aria-hidden="true" />
              {t('gateway.page.statistics.modelHealth')}
            </span>
          </div>
          <div className={styles.emptyState}>
            <Shield size={18} aria-hidden="true" />
            <span>{t('gateway.page.statistics.empty')}</span>
          </div>
        </section>
        <section className={styles.dataPanel}>
          <div className={styles.panelHeader}>
            <span>
              <Clock3 size={14} aria-hidden="true" />
              {t('gateway.page.statistics.latency')}
            </span>
          </div>
          <div className={styles.emptyState}>
            <Gauge size={18} aria-hidden="true" />
            <span>{t('gateway.page.statistics.empty')}</span>
          </div>
        </section>
      </div>
    </div>
  );
};

const GatewayRequestsView: React.FC = () => {
  const { t } = useTranslation();
  const detailTabs = [
    t('gateway.page.requests.detailTabs.record'),
    t('gateway.page.requests.detailTabs.body'),
    t('gateway.page.requests.detailTabs.headers'),
    t('gateway.page.requests.detailTabs.response'),
  ];

  return (
    <div className={styles.viewStack}>
      <div className={styles.viewToolbar}>
        <div>
          <h2>{t('gateway.page.requests.title')}</h2>
          <p>{t('gateway.page.requests.subtitle')}</p>
        </div>
      </div>
      <div className={styles.requestGrid}>
        <section className={styles.dataPanel}>
          <div className={styles.panelHeader}>
            <span>
              <FileText size={14} aria-hidden="true" />
              {t('gateway.page.requests.records')}
            </span>
            <span className={styles.panelCount}>0</span>
          </div>
          <div className={styles.emptyState}>
            <FileText size={18} aria-hidden="true" />
            <span>{t('gateway.page.requests.empty')}</span>
          </div>
        </section>
        <section className={styles.dataPanel}>
          <div className={styles.panelHeader}>
            <span>
              <Network size={14} aria-hidden="true" />
              {t('gateway.page.requests.detail')}
            </span>
          </div>
          <div className={styles.detailTabList}>
            {detailTabs.map((label) => (
              <span key={label}>{label}</span>
            ))}
          </div>
          <div className={styles.emptyState}>
            <FileText size={18} aria-hidden="true" />
            <span>{t('gateway.page.requests.detailEmpty')}</span>
          </div>
        </section>
      </div>
    </div>
  );
};

const GatewayPage: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const activeTab = resolveGatewayTabFromPath(location.pathname);
  const [summaryState, setSummaryState] = React.useState<GatewaySummaryState>({
    settings: null,
    status: null,
    health: null,
    cliStatuses: [],
  });
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (location.pathname === '/gateway') {
      navigate(DEFAULT_GATEWAY_PATH, { replace: true });
    }
  }, [location.pathname, navigate]);

  const loadSummary = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [settings, status, health, cliStatuses] = await Promise.all([
        getProxyGatewaySettings(),
        getProxyGatewayStatus(),
        checkProxyGatewayHealth(),
        getProxyGatewayCliStatuses(),
      ]);
      setSummaryState({ settings, status, health, cliStatuses });
    } catch (loadError) {
      setError(t('gateway.page.statistics.loadFailed', { error: formatGatewayError(loadError) }));
    } finally {
      setLoading(false);
    }
  }, [t]);

  React.useEffect(() => {
    if (activeTab === 'statistics') {
      void loadSummary();
    }
  }, [activeTab, loadSummary]);

  const handleTabChange = (tabKey: GatewayPageTab) => {
    navigate(getGatewayPathForTab(tabKey));
  };

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

      {activeTab === 'statistics' ? (
        <GatewayStatisticsView
          state={summaryState}
          loading={loading}
          error={error}
          onRefresh={() => void loadSummary()}
        />
      ) : null}
      {activeTab === 'requests' ? <GatewayRequestsView /> : null}
      {activeTab === 'settings' ? <GatewaySettingsPanel showTitleBlock={false} /> : null}
    </div>
  );
};

export default GatewayPage;
