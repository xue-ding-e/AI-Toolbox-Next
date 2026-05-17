use super::paths::ProxyGatewayPaths;
use super::types::{MetricEvent, MetricRollupItem};
use std::collections::BTreeMap;
use std::fs;

const ROLLUPS_FILE_NAME: &str = "rollups.json";

pub fn rollup_key(event: &MetricEvent) -> String {
    format!(
        "{}|{}|{}|{}",
        event.cli_key.as_str(),
        event.provider_id,
        event.requested_model,
        event.upstream_model_id
    )
}

pub fn latency_bucket(duration_ms: u64) -> &'static str {
    match duration_ms {
        0..=999 => "lt_1s",
        1_000..=2_999 => "1s_3s",
        3_000..=9_999 => "3s_10s",
        10_000..=29_999 => "10s_30s",
        _ => "gte_30s",
    }
}

pub fn apply_metric_event(rollups: &mut BTreeMap<String, MetricRollupItem>, event: &MetricEvent) {
    let key = rollup_key(event);
    let item = rollups.entry(key).or_insert_with(|| MetricRollupItem {
        cli_key: event.cli_key,
        provider_id: event.provider_id.clone(),
        requested_model: event.requested_model.clone(),
        upstream_model_id: event.upstream_model_id.clone(),
        ..MetricRollupItem::default()
    });

    item.total_requests += 1;
    if event.success {
        item.success_requests += 1;
    } else {
        item.failed_requests += 1;
    }
    if event.failover {
        item.failover_requests += 1;
    }
    item.total_attempts += u64::from(event.attempt_count);
    item.total_duration_ms += event.duration_ms;
    item.min_duration_ms = Some(
        item.min_duration_ms
            .map(|current| current.min(event.duration_ms))
            .unwrap_or(event.duration_ms),
    );
    item.max_duration_ms = Some(
        item.max_duration_ms
            .map(|current| current.max(event.duration_ms))
            .unwrap_or(event.duration_ms),
    );
    if let Some(status_code) = event.status_code {
        *item
            .status_counts
            .entry(status_code.to_string())
            .or_insert(0) += 1;
    }
    if let Some(error_category) = &event.error_category {
        *item
            .error_category_counts
            .entry(error_category.clone())
            .or_insert(0) += 1;
    }
    *item
        .latency_buckets
        .entry(latency_bucket(event.duration_ms).to_string())
        .or_insert(0) += 1;
    item.input_tokens += event.input_tokens.unwrap_or(0);
    item.output_tokens += event.output_tokens.unwrap_or(0);
}

pub fn record_metric_event(paths: &ProxyGatewayPaths, event: &MetricEvent) -> Result<(), String> {
    let mut rollups = read_rollups(paths)?;
    apply_metric_event(&mut rollups, event);
    write_rollups(paths, &rollups)
}

pub fn list_metric_rollups(paths: &ProxyGatewayPaths) -> Result<Vec<MetricRollupItem>, String> {
    let mut rollups: Vec<MetricRollupItem> = read_rollups(paths)?.into_values().collect();
    rollups.sort_by(|left, right| {
        right
            .total_requests
            .cmp(&left.total_requests)
            .then_with(|| left.cli_key.as_str().cmp(right.cli_key.as_str()))
            .then_with(|| left.provider_id.cmp(&right.provider_id))
            .then_with(|| left.requested_model.cmp(&right.requested_model))
    });
    Ok(rollups)
}

fn read_rollups(paths: &ProxyGatewayPaths) -> Result<BTreeMap<String, MetricRollupItem>, String> {
    let path = rollups_path(paths);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let content = fs::read_to_string(&path).map_err(|error| {
        format!(
            "Failed to read proxy gateway metric rollups {}: {}",
            path.display(),
            error
        )
    })?;
    serde_json::from_str(&content).map_err(|error| {
        format!(
            "Failed to parse proxy gateway metric rollups {}: {}",
            path.display(),
            error
        )
    })
}

fn write_rollups(
    paths: &ProxyGatewayPaths,
    rollups: &BTreeMap<String, MetricRollupItem>,
) -> Result<(), String> {
    let path = rollups_path(paths);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create proxy gateway metrics directory {}: {}",
                parent.display(),
                error
            )
        })?;
    }
    let content = serde_json::to_string_pretty(rollups)
        .map_err(|error| format!("Failed to serialize proxy gateway metric rollups: {error}"))?;
    fs::write(&path, format!("{content}\n")).map_err(|error| {
        format!(
            "Failed to write proxy gateway metric rollups {}: {}",
            path.display(),
            error
        )
    })
}

fn rollups_path(paths: &ProxyGatewayPaths) -> std::path::PathBuf {
    paths.metrics_root().join(ROLLUPS_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::proxy_gateway::types::GatewayCliKey;
    use chrono::Utc;

    fn event(provider_id: &str, requested_model: &str, duration_ms: u64) -> MetricEvent {
        MetricEvent {
            schema_version: 1,
            trace_id: "trace".to_string(),
            ended_at: Utc::now(),
            cli_key: GatewayCliKey::Claude,
            provider_id: provider_id.to_string(),
            requested_model: requested_model.to_string(),
            upstream_model_id: requested_model.to_string(),
            success: true,
            status_code: Some(200),
            error_category: None,
            duration_ms,
            attempt_count: 1,
            failover: false,
            input_tokens: Some(10),
            output_tokens: Some(20),
        }
    }

    #[test]
    fn latency_bucket_classifies_boundaries() {
        assert_eq!(latency_bucket(999), "lt_1s");
        assert_eq!(latency_bucket(1_000), "1s_3s");
        assert_eq!(latency_bucket(3_000), "3s_10s");
        assert_eq!(latency_bucket(10_000), "10s_30s");
        assert_eq!(latency_bucket(30_000), "gte_30s");
    }

    #[test]
    fn rollup_key_uses_cli_provider_and_models() {
        let event = event("provider-a", "haiku", 500);
        assert_eq!(rollup_key(&event), "claude|provider-a|haiku|haiku");
    }

    #[test]
    fn metric_events_group_by_provider_and_model() {
        let mut rollups = BTreeMap::new();
        apply_metric_event(&mut rollups, &event("provider-a", "haiku", 500));
        apply_metric_event(&mut rollups, &event("provider-a", "opus", 700));
        apply_metric_event(&mut rollups, &event("provider-b", "haiku", 900));

        assert_eq!(rollups.len(), 3);
    }

    #[test]
    fn metric_rollup_accumulates_counts_and_tokens() {
        let mut failed = event("provider-a", "haiku", 1_500);
        failed.success = false;
        failed.status_code = Some(429);
        failed.error_category = Some("rate_limit".to_string());
        failed.attempt_count = 2;
        failed.failover = true;

        let mut rollups = BTreeMap::new();
        apply_metric_event(&mut rollups, &event("provider-a", "haiku", 500));
        apply_metric_event(&mut rollups, &failed);

        let item = rollups.get("claude|provider-a|haiku|haiku").unwrap();
        assert_eq!(item.total_requests, 2);
        assert_eq!(item.success_requests, 1);
        assert_eq!(item.failed_requests, 1);
        assert_eq!(item.failover_requests, 1);
        assert_eq!(item.total_attempts, 3);
        assert_eq!(item.status_counts.get("200"), Some(&1));
        assert_eq!(item.status_counts.get("429"), Some(&1));
        assert_eq!(item.error_category_counts.get("rate_limit"), Some(&1));
        assert_eq!(item.latency_buckets.get("lt_1s"), Some(&1));
        assert_eq!(item.latency_buckets.get("1s_3s"), Some(&1));
        assert_eq!(item.input_tokens, 20);
        assert_eq!(item.output_tokens, 40);
    }

    #[test]
    fn metric_rollups_persist_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = ProxyGatewayPaths::new(dir.path());

        record_metric_event(&paths, &event("provider-a", "haiku", 500)).unwrap();
        record_metric_event(&paths, &event("provider-a", "haiku", 700)).unwrap();

        let rollups = list_metric_rollups(&paths).unwrap();
        assert_eq!(rollups.len(), 1);
        assert_eq!(rollups[0].total_requests, 2);
        assert_eq!(rollups[0].total_duration_ms, 1_200);
    }
}
