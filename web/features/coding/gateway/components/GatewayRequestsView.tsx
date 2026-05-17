import React from 'react';
import { AlertCircle, FileText, Network, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  getProxyGatewayRequestLogDetail,
  listProxyGatewayRequestLogs,
  type GatewayRequestLogDetail,
  type GatewayRequestLogSummary,
} from '@/services';
import {
  formatDateTime,
  formatDuration,
  formatGatewayError,
  formatInteger,
  joinClassNames,
  stringifyDetailValue,
} from '../utils/gatewayFormatters';
import styles from './GatewayRequestsView.module.less';

type RequestDetailTabKey = 'record' | 'body' | 'headers' | 'response';

const REQUEST_DETAIL_TABS: RequestDetailTabKey[] = ['record', 'body', 'headers', 'response'];

const GatewayRequestsView: React.FC = () => {
  const { t } = useTranslation();
  const [logs, setLogs] = React.useState<GatewayRequestLogSummary[]>([]);
  const [selectedTraceId, setSelectedTraceId] = React.useState<string | null>(null);
  const [detail, setDetail] = React.useState<GatewayRequestLogDetail | null>(null);
  const [activeDetailTab, setActiveDetailTab] = React.useState<RequestDetailTabKey>('record');
  const [loading, setLoading] = React.useState(false);
  const [detailLoading, setDetailLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const loadDetail = React.useCallback(
    async (traceId: string) => {
      setSelectedTraceId(traceId);
      setDetailLoading(true);
      setError(null);
      try {
        const nextDetail = await getProxyGatewayRequestLogDetail(traceId);
        setDetail(nextDetail);
        setActiveDetailTab('record');
      } catch (detailError) {
        setError(t('gateway.page.requests.loadFailed', { error: formatGatewayError(detailError) }));
      } finally {
        setDetailLoading(false);
      }
    },
    [t],
  );

  const loadRequests = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const nextLogs = await listProxyGatewayRequestLogs({ limit: 100 });
      setLogs(nextLogs);
      const nextSelected = nextLogs[0]?.trace_id ?? null;
      if (nextSelected) {
        await loadDetail(nextSelected);
      } else {
        setSelectedTraceId(null);
        setDetail(null);
      }
    } catch (loadError) {
      setError(t('gateway.page.requests.loadFailed', { error: formatGatewayError(loadError) }));
    } finally {
      setLoading(false);
    }
  }, [loadDetail, t]);

  React.useEffect(() => {
    void loadRequests();
  }, [loadRequests]);

  const renderDetailContent = () => {
    if (detailLoading) {
      return (
        <div className={styles.emptyState}>
          <RefreshCw size={18} className={styles.spin} aria-hidden="true" />
          <span>{t('common.loading')}</span>
        </div>
      );
    }
    if (!detail) {
      return (
        <div className={styles.emptyState}>
          <FileText size={18} aria-hidden="true" />
          <span>{t('gateway.page.requests.detailEmpty')}</span>
        </div>
      );
    }

    if (activeDetailTab === 'record') {
      return (
        <div className={styles.detailGrid}>
          <span>{t('gateway.page.requests.fields.traceId')}</span>
          <code>{detail.trace_id}</code>
          <span>{t('gateway.page.requests.fields.time')}</span>
          <strong>{formatDateTime(detail.ended_at)}</strong>
          <span>{t('gateway.page.requests.fields.provider')}</span>
          <strong>{detail.provider_name ?? detail.provider_id ?? '-'}</strong>
          <span>{t('gateway.page.requests.fields.model')}</span>
          <strong>{detail.requested_model ?? '-'}</strong>
          <span>{t('gateway.page.requests.fields.status')}</span>
          <strong>{detail.status_code ?? '-'}</strong>
          <span>{t('gateway.page.requests.fields.duration')}</span>
          <strong>{formatDuration(detail.duration_ms)}</strong>
          <span>{t('gateway.page.requests.fields.tokens')}</span>
          <strong>
            {t('gateway.page.requests.tokensValue', {
              input: formatInteger(detail.input_tokens),
              output: formatInteger(detail.output_tokens),
              total: formatInteger(detail.total_tokens),
            })}
          </strong>
          <span>{t('gateway.page.requests.fields.attempts')}</span>
          <strong>{detail.attempt_count}</strong>
          <span>{t('gateway.page.requests.fields.upstream')}</span>
          <code>{detail.upstream_url ?? '-'}</code>
          <span>{t('gateway.page.requests.fields.error')}</span>
          <strong>{detail.error_category ?? '-'}</strong>
        </div>
      );
    }

    if (activeDetailTab === 'body') {
      return (
        <pre className={styles.detailPre}>
          {detail.request_body ?? t('gateway.page.requests.notStored')}
        </pre>
      );
    }

    if (activeDetailTab === 'headers') {
      return (
        <div className={styles.detailStack}>
          <span className={styles.detailSubtitle}>{t('gateway.page.requests.requestHeaders')}</span>
          <pre className={styles.detailPre}>
            {stringifyDetailValue(detail.request_headers) || t('gateway.page.requests.notStored')}
          </pre>
          <span className={styles.detailSubtitle}>{t('gateway.page.requests.responseHeaders')}</span>
          <pre className={styles.detailPre}>
            {stringifyDetailValue(detail.response_headers) || t('gateway.page.requests.notStored')}
          </pre>
        </div>
      );
    }

    return (
      <pre className={styles.detailPre}>
        {detail.response_body ?? t('gateway.page.requests.notStored')}
      </pre>
    );
  };

  return (
    <div className={styles.viewStack}>
      <div className={styles.viewToolbar}>
        <div>
          <h2>{t('gateway.page.requests.title')}</h2>
          <p>{t('gateway.page.requests.subtitle')}</p>
        </div>
        <button type="button" className={styles.toolButton} disabled={loading} onClick={() => void loadRequests()}>
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
      <div className={styles.requestGrid}>
        <section className={styles.dataPanel}>
          <div className={styles.panelHeader}>
            <span>
              <FileText size={14} aria-hidden="true" />
              {t('gateway.page.requests.records')}
            </span>
            <span className={styles.panelCount}>{logs.length}</span>
          </div>
          {logs.length ? (
            <div className={styles.requestList}>
              {logs.map((log) => (
                <button
                  key={log.trace_id}
                  type="button"
                  className={joinClassNames(
                    styles.requestRow,
                    selectedTraceId === log.trace_id && styles.requestRowActive,
                  )}
                  onClick={() => void loadDetail(log.trace_id)}
                >
                  <span className={styles.requestMethod}>{log.method}</span>
                  <span className={styles.requestMain}>
                    <strong>{log.requested_model ?? log.path}</strong>
                    <small>
                      {log.cli_key ? t(`settings.gateway.cli.${log.cli_key}`) : '-'} · {log.provider_name ?? log.provider_id ?? '-'}
                    </small>
                    <small>
                      {formatDateTime(log.ended_at)} · {t('gateway.page.requests.tokensShort', {
                        input: formatInteger(log.input_tokens),
                        output: formatInteger(log.output_tokens),
                      })}
                    </small>
                  </span>
                  <span className={styles.requestBadges}>
                    <span className={joinClassNames(styles.statusCode, log.success ? styles.statusCodeSuccess : styles.statusCodeError)}>
                      {log.status_code ?? '-'}
                    </span>
                    {log.failover || log.attempt_count > 1 ? (
                      <span className={styles.failoverBadge}>
                        {t('gateway.page.requests.attemptBadge', { count: log.attempt_count })}
                      </span>
                    ) : null}
                  </span>
                  <span className={styles.requestDuration}>{formatDuration(log.duration_ms)}</span>
                </button>
              ))}
            </div>
          ) : (
            <div className={styles.emptyState}>
              <FileText size={18} aria-hidden="true" />
              <span>{loading ? t('common.loading') : t('gateway.page.requests.empty')}</span>
            </div>
          )}
        </section>
        <section className={styles.dataPanel}>
          <div className={styles.panelHeader}>
            <span>
              <Network size={14} aria-hidden="true" />
              {t('gateway.page.requests.detail')}
            </span>
          </div>
          <div className={styles.detailTabList}>
            {REQUEST_DETAIL_TABS.map((tabKey) => (
              <button
                key={tabKey}
                type="button"
                className={joinClassNames(styles.detailTabButton, activeDetailTab === tabKey && styles.detailTabButtonActive)}
                onClick={() => setActiveDetailTab(tabKey)}
              >
                {t(`gateway.page.requests.detailTabs.${tabKey}`)}
              </button>
            ))}
          </div>
          {renderDetailContent()}
        </section>
      </div>
    </div>
  );
};

export default GatewayRequestsView;
