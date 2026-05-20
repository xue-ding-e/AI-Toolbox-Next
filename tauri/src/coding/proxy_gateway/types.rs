use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayCliKey {
    Claude,
    Codex,
    Gemini,
    OpenCode,
}

impl GatewayCliKey {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::OpenCode => "opencode",
        }
    }

    pub fn supported_mvp() -> Vec<Self> {
        vec![Self::Claude, Self::Codex, Self::Gemini]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProxyGatewaySettings {
    pub enabled_on_startup: bool,
    pub listen_host: String,
    pub listen_port: u16,
    pub port_auto_select: bool,
    pub enabled_cli_keys: Vec<GatewayCliKey>,
    pub request_log_enabled: bool,
    pub request_log_level: String,
    pub metrics_enabled: bool,
    pub store_request_body: bool,
    pub store_headers: bool,
    pub store_response_body: bool,
    pub thinking_rectifier_enabled: bool,
    pub log_retention_days: u32,
    pub log_max_dir_size_mb: u64,
    pub log_max_body_size_kb: u64,
    pub per_provider_retry_count: u32,
    pub max_retry_count: u32,
    pub model_failure_score_threshold: i32,
    pub model_failure_window_seconds: u64,
    pub model_base_cooldown_seconds: u64,
    pub model_max_cooldown_seconds: u64,
    pub half_open_success_required: u32,
}

impl Default for ProxyGatewaySettings {
    fn default() -> Self {
        Self {
            enabled_on_startup: false,
            listen_host: "127.0.0.1".to_string(),
            listen_port: 37123,
            port_auto_select: false,
            enabled_cli_keys: GatewayCliKey::supported_mvp(),
            request_log_enabled: true,
            request_log_level: "summary".to_string(),
            metrics_enabled: true,
            store_request_body: false,
            store_headers: false,
            store_response_body: false,
            thinking_rectifier_enabled: true,
            log_retention_days: 7,
            log_max_dir_size_mb: 512,
            log_max_body_size_kb: 256,
            per_provider_retry_count: 0,
            max_retry_count: 8,
            model_failure_score_threshold: 5,
            model_failure_window_seconds: 300,
            model_base_cooldown_seconds: 120,
            model_max_cooldown_seconds: 1800,
            half_open_success_required: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProxyGatewayStatus {
    pub running: bool,
    pub base_url: Option<String>,
    pub listen_host: String,
    pub listen_port: Option<u16>,
    pub last_error: Option<String>,
}

impl ProxyGatewayStatus {
    pub fn stopped(settings: &ProxyGatewaySettings, last_error: Option<String>) -> Self {
        Self {
            running: false,
            base_url: None,
            listen_host: settings.listen_host.clone(),
            listen_port: None,
            last_error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProxyGatewayPortCheckInput {
    pub listen_host: String,
    pub listen_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProxyGatewayPortCheckResult {
    pub available: bool,
    pub listen_host: String,
    pub listen_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProxyGatewayHealthCheckResult {
    pub ok: bool,
    pub status_code: Option<u16>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayCliTakeoverState {
    Direct,
    TakeoverApplied,
    GatewayStopped,
    OutdatedOrigin,
    Drifted,
    RestoreUnavailable,
    Unsupported,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayCliStatusDot {
    Gray,
    Green,
    Orange,
    Red,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayManagedTarget {
    pub kind: String,
    pub path: String,
    pub existed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayCliTakeoverStatus {
    pub cli_key: GatewayCliKey,
    pub state: GatewayCliTakeoverState,
    pub dot: GatewayCliStatusDot,
    pub can_takeover: bool,
    pub can_restore_direct: bool,
    pub gateway_origin: Option<String>,
    pub runtime_root: Option<String>,
    pub managed_targets: Vec<GatewayManagedTarget>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProxyGatewayStopPreflight {
    pub allowed: bool,
    pub blocking_cli_takeovers: Vec<GatewayCliTakeoverStatus>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProviderModelHealthKey {
    pub cli_key: GatewayCliKey,
    pub provider_id: String,
    pub upstream_model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProviderHealthKey {
    pub cli_key: GatewayCliKey,
    pub provider_id: String,
}

impl From<&ProviderModelHealthKey> for ProviderHealthKey {
    fn from(key: &ProviderModelHealthKey) -> Self {
        Self {
            cli_key: key.cli_key,
            provider_id: key.provider_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelHealthStateKind {
    Healthy,
    Degraded,
    CoolingDown,
    Probing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelHealthEntry {
    pub state: ModelHealthStateKind,
    pub failure_score: i32,
    pub consecutive_open_count: u32,
    pub half_open_success_count: u32,
    pub next_retry_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_failure_at: Option<DateTime<Utc>>,
    pub last_error_category: Option<String>,
}

impl Default for ModelHealthEntry {
    fn default() -> Self {
        Self {
            state: ModelHealthStateKind::Healthy,
            failure_score: 0,
            consecutive_open_count: 0,
            half_open_success_count: 0,
            next_retry_at: None,
            last_failure_at: None,
            last_error_category: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MetricEvent {
    pub schema_version: u32,
    pub trace_id: String,
    pub ended_at: DateTime<Utc>,
    pub cli_key: GatewayCliKey,
    pub provider_id: String,
    pub requested_model: String,
    pub upstream_model_id: String,
    pub success: bool,
    pub status_code: Option<u16>,
    pub error_category: Option<String>,
    pub duration_ms: u64,
    pub attempt_count: u32,
    #[serde(default)]
    pub total_attempt_count: u32,
    pub failover: bool,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MetricRollupItem {
    pub cli_key: GatewayCliKey,
    pub provider_id: String,
    pub requested_model: String,
    pub upstream_model_id: String,
    pub total_requests: u64,
    pub success_requests: u64,
    pub failed_requests: u64,
    pub failover_requests: u64,
    pub total_attempts: u64,
    pub total_duration_ms: u64,
    pub min_duration_ms: Option<u64>,
    pub max_duration_ms: Option<u64>,
    pub status_counts: BTreeMap<String, u64>,
    pub error_category_counts: BTreeMap<String, u64>,
    pub latency_buckets: BTreeMap<String, u64>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProxyGatewayRequestLogListInput {
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayRequestLogFilters {
    pub cli_key: Option<GatewayCliKey>,
    pub provider_name: Option<String>,
    pub model: Option<String>,
    pub status_code: Option<u16>,
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayPaginatedRequestLogs {
    pub data: Vec<GatewayRequestLogItem>,
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayRequestLogItem {
    pub trace_id: String,
    pub cli_key: GatewayCliKey,
    pub provider_id: String,
    pub provider_name: Option<String>,
    pub requested_model: Option<String>,
    pub upstream_model_id: String,
    pub status_code: u16,
    pub success: bool,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub total_cost_usd: String,
    pub is_streaming: bool,
    pub route_name: Option<String>,
    pub path: Option<String>,
    pub request_body_bytes: u64,
    pub response_body_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayUsageSummary {
    pub total_requests: u64,
    pub total_cost_usd: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub success_rate: f32,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayUsageSummaryByCli {
    pub cli_key: GatewayCliKey,
    pub summary: GatewayUsageSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayUsageTrendPoint {
    pub date: String,
    pub request_count: u64,
    pub total_cost_usd: String,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayProviderStats {
    pub cli_key: GatewayCliKey,
    pub provider_id: String,
    pub provider_name: Option<String>,
    pub request_count: u64,
    pub total_tokens: u64,
    pub total_cost_usd: String,
    pub success_rate: u32,
    pub avg_latency_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayModelStats {
    pub cli_key: GatewayCliKey,
    pub model: String,
    pub request_count: u64,
    pub total_tokens: u64,
    pub total_cost_usd: String,
    pub avg_latency_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayRequestLogSummary {
    pub trace_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub cli_key: Option<GatewayCliKey>,
    pub route_name: String,
    pub method: String,
    pub path: String,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub requested_model: Option<String>,
    pub upstream_model_id: Option<String>,
    pub upstream_url: Option<String>,
    pub status_code: Option<u16>,
    pub success: bool,
    pub error_category: Option<String>,
    pub error_message: Option<String>,
    pub duration_ms: u64,
    pub attempt_count: u32,
    #[serde(default)]
    pub total_attempt_count: u32,
    pub failover: bool,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub request_body_bytes: u64,
    pub response_body_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayRequestLogDetail {
    #[serde(flatten)]
    pub summary: GatewayRequestLogSummary,
    pub request_headers: Option<BTreeMap<String, String>>,
    pub request_body: Option<String>,
    #[serde(default)]
    pub upstream_request_body: Option<String>,
    pub response_headers: Option<BTreeMap<String, String>>,
    pub response_body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayRequestLogRecord {
    pub schema_version: u32,
    #[serde(flatten)]
    pub detail: GatewayRequestLogDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayModelHealthScope {
    Model,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayModelHealthItem {
    pub scope: GatewayModelHealthScope,
    pub cli_key: GatewayCliKey,
    pub provider_id: String,
    #[serde(default)]
    pub provider_name: Option<String>,
    pub upstream_model_id: Option<String>,
    pub state: ModelHealthStateKind,
    pub failure_score: i32,
    pub consecutive_open_count: u32,
    pub half_open_success_count: u32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub last_error_category: Option<String>,
}

impl Default for GatewayCliKey {
    fn default() -> Self {
        Self::Claude
    }
}
