import type { MetricRollupItem, ProxyGatewaySettings, ProxyGatewayStatus } from '@/services';

export const joinClassNames = (...classNames: Array<string | false | null | undefined>) =>
  classNames.filter(Boolean).join(' ');

export const formatGatewayError = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

export const deriveRequestLogLevel = (settings: ProxyGatewaySettings | null) => {
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

export const buildGatewayOrigin = (status: ProxyGatewayStatus | null) => {
  if (!status) {
    return '-';
  }
  if (status.base_url) {
    return status.base_url;
  }
  return status.listen_port ? `http://${status.listen_host}:${status.listen_port}` : '-';
};

export const formatDuration = (durationMs: number) => {
  if (durationMs < 1000) {
    return `${durationMs}ms`;
  }
  return `${(durationMs / 1000).toFixed(durationMs < 10_000 ? 1 : 0)}s`;
};

export const formatDateTime = (value: string | null | undefined) => {
  if (!value) {
    return '-';
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
};

export const formatInteger = (value: number | null | undefined) => {
  if (value == null) {
    return '-';
  }
  return value.toLocaleString();
};

interface AttemptCountsInput {
  attempt_count: number;
  total_attempt_count?: number | null;
}

export const normalizeAttemptCounts = (value: AttemptCountsInput) => {
  const current = Math.max(value.attempt_count || 0, 1);
  return {
    current,
    total: Math.max(value.total_attempt_count || 0, current),
  };
};

export const successRateText = (successCount: number, totalCount: number) => {
  if (totalCount <= 0) {
    return '-';
  }
  return `${Math.round((successCount / totalCount) * 100)}%`;
};

export const averageLatency = (rollup: MetricRollupItem) => {
  if (rollup.total_requests <= 0) {
    return 0;
  }
  return Math.round(rollup.total_duration_ms / rollup.total_requests);
};

export const stringifyDetailValue = (value: unknown) => {
  if (value == null) {
    return '';
  }
  if (typeof value === 'string') {
    return value;
  }
  return JSON.stringify(value, null, 2);
};
