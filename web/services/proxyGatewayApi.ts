import { invoke } from '@tauri-apps/api/core';

export type GatewayCliKey = 'claude' | 'codex' | 'gemini' | 'opencode';

export interface ProxyGatewaySettings {
  enabled_on_startup: boolean;
  listen_host: string;
  listen_port: number;
  port_auto_select: boolean;
  enabled_cli_keys: GatewayCliKey[];
  request_log_enabled: boolean;
  request_log_level: string;
  metrics_enabled: boolean;
  store_request_body: boolean;
  store_headers: boolean;
  store_response_body: boolean;
  log_retention_days: number;
  log_max_dir_size_mb: number;
  log_max_body_size_kb: number;
  model_failure_score_threshold: number;
  model_failure_window_seconds: number;
  model_base_cooldown_seconds: number;
  model_max_cooldown_seconds: number;
  half_open_success_required: number;
}

export interface ProxyGatewayStatus {
  running: boolean;
  base_url: string | null;
  listen_host: string;
  listen_port: number | null;
  last_error: string | null;
}

export interface ProxyGatewayPortCheckInput {
  listen_host: string;
  listen_port: number;
}

export interface ProxyGatewayPortCheckResult {
  available: boolean;
  listen_host: string;
  listen_port: number;
}

export interface ProxyGatewayHealthCheckResult {
  ok: boolean;
  status_code: number | null;
  error: string | null;
}

export type GatewayCliTakeoverState =
  | 'direct'
  | 'takeover_applied'
  | 'gateway_stopped'
  | 'outdated_origin'
  | 'drifted'
  | 'restore_unavailable'
  | 'unsupported'
  | 'error';

export type GatewayCliStatusDot = 'gray' | 'green' | 'orange' | 'red';

export interface GatewayManagedTarget {
  kind: string;
  path: string;
  existed: boolean;
}

export interface GatewayCliTakeoverStatus {
  cli_key: GatewayCliKey;
  state: GatewayCliTakeoverState;
  dot: GatewayCliStatusDot;
  can_takeover: boolean;
  can_restore_direct: boolean;
  gateway_origin: string | null;
  runtime_root: string | null;
  managed_targets: GatewayManagedTarget[];
  message: string | null;
}

export interface ProxyGatewayStopPreflight {
  allowed: boolean;
  blocking_cli_takeovers: GatewayCliTakeoverStatus[];
  message: string | null;
}

export const getProxyGatewaySettings = async (): Promise<ProxyGatewaySettings> => {
  return invoke<ProxyGatewaySettings>('proxy_gateway_get_settings');
};

export const updateProxyGatewaySettings = async (
  settings: ProxyGatewaySettings
): Promise<ProxyGatewaySettings> => {
  return invoke<ProxyGatewaySettings>('proxy_gateway_update_settings', { settings });
};

export const startProxyGateway = async (
  settings?: ProxyGatewaySettings
): Promise<ProxyGatewayStatus> => {
  return invoke<ProxyGatewayStatus>('proxy_gateway_start', { settings: settings ?? null });
};

export const stopProxyGateway = async (): Promise<ProxyGatewayStatus> => {
  return invoke<ProxyGatewayStatus>('proxy_gateway_stop');
};

export const getProxyGatewayStatus = async (): Promise<ProxyGatewayStatus> => {
  return invoke<ProxyGatewayStatus>('proxy_gateway_status');
};

export const checkProxyGatewayHealth = async (): Promise<ProxyGatewayHealthCheckResult> => {
  return invoke<ProxyGatewayHealthCheckResult>('proxy_gateway_health_check');
};

export const checkProxyGatewayPortAvailable = async (
  input: ProxyGatewayPortCheckInput
): Promise<ProxyGatewayPortCheckResult> => {
  return invoke<ProxyGatewayPortCheckResult>('proxy_gateway_check_port_available', { input });
};

export const getProxyGatewayCliStatuses = async (): Promise<GatewayCliTakeoverStatus[]> => {
  return invoke<GatewayCliTakeoverStatus[]>('proxy_gateway_cli_statuses');
};

export const getProxyGatewayCliStatus = async (
  cliKey: GatewayCliKey
): Promise<GatewayCliTakeoverStatus> => {
  return invoke<GatewayCliTakeoverStatus>('proxy_gateway_cli_status', { cliKey });
};

export const takeoverProxyGatewayCli = async (
  cliKey: GatewayCliKey
): Promise<GatewayCliTakeoverStatus> => {
  return invoke<GatewayCliTakeoverStatus>('proxy_gateway_takeover_cli', { cliKey });
};

export const restoreProxyGatewayCliDirect = async (
  cliKey: GatewayCliKey
): Promise<GatewayCliTakeoverStatus> => {
  return invoke<GatewayCliTakeoverStatus>('proxy_gateway_restore_cli_direct', { cliKey });
};

export const preflightStopProxyGateway = async (): Promise<ProxyGatewayStopPreflight> => {
  return invoke<ProxyGatewayStopPreflight>('proxy_gateway_stop_preflight');
};
