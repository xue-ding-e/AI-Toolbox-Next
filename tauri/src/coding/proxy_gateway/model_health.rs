use super::types::{
    ModelHealthEntry, ModelHealthStateKind, ProviderHealthKey, ProviderModelHealthKey,
    ProxyGatewaySettings,
};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayFailureKind {
    Timeout,
    Connection,
    RateLimit,
    Upstream5xx,
    ModelNotFound,
    Auth,
    RequestSchema,
    ClientCancelled,
    GatewayParse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureScope {
    Model,
    Provider,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailureWeight {
    pub scope: FailureScope,
    pub score: i32,
    pub category: &'static str,
}

pub fn classify_failure(kind: GatewayFailureKind) -> FailureWeight {
    match kind {
        GatewayFailureKind::Timeout => FailureWeight {
            scope: FailureScope::Model,
            score: 3,
            category: "timeout",
        },
        GatewayFailureKind::Connection => FailureWeight {
            scope: FailureScope::Model,
            score: 3,
            category: "connection",
        },
        GatewayFailureKind::RateLimit => FailureWeight {
            scope: FailureScope::Model,
            score: 3,
            category: "rate_limit",
        },
        GatewayFailureKind::Upstream5xx => FailureWeight {
            scope: FailureScope::Model,
            score: 2,
            category: "upstream_5xx",
        },
        GatewayFailureKind::ModelNotFound => FailureWeight {
            scope: FailureScope::Model,
            score: 5,
            category: "model_not_found",
        },
        GatewayFailureKind::Auth => FailureWeight {
            scope: FailureScope::Provider,
            score: 5,
            category: "auth",
        },
        GatewayFailureKind::RequestSchema => FailureWeight {
            scope: FailureScope::None,
            score: 0,
            category: "request_schema",
        },
        GatewayFailureKind::ClientCancelled => FailureWeight {
            scope: FailureScope::None,
            score: 0,
            category: "client_cancelled",
        },
        GatewayFailureKind::GatewayParse => FailureWeight {
            scope: FailureScope::None,
            score: 0,
            category: "gateway_parse",
        },
    }
}

pub struct ModelHealthRegistry {
    settings: ProxyGatewaySettings,
    model_entries: HashMap<ProviderModelHealthKey, ModelHealthEntry>,
    provider_entries: HashMap<ProviderHealthKey, ModelHealthEntry>,
}

impl ModelHealthRegistry {
    pub fn new(settings: ProxyGatewaySettings) -> Self {
        Self {
            settings,
            model_entries: HashMap::new(),
            provider_entries: HashMap::new(),
        }
    }

    pub fn model_entry(&self, key: &ProviderModelHealthKey) -> Option<&ModelHealthEntry> {
        self.model_entries.get(key)
    }

    pub fn provider_entry(&self, key: &ProviderHealthKey) -> Option<&ModelHealthEntry> {
        self.provider_entries.get(key)
    }

    pub fn is_model_available(&self, key: &ProviderModelHealthKey, now: DateTime<Utc>) -> bool {
        if self
            .provider_entries
            .get(&ProviderHealthKey::from(key))
            .is_some_and(|entry| is_cooling(entry, now))
        {
            return false;
        }

        self.model_entries
            .get(key)
            .map(|entry| !is_cooling(entry, now))
            .unwrap_or(true)
    }

    pub fn refresh_due_cooldowns(&mut self, now: DateTime<Utc>) {
        for entry in self
            .model_entries
            .values_mut()
            .chain(self.provider_entries.values_mut())
        {
            if entry.state == ModelHealthStateKind::CoolingDown
                && entry.next_retry_at.is_some_and(|retry_at| retry_at <= now)
            {
                entry.state = ModelHealthStateKind::Probing;
                entry.half_open_success_count = 0;
            }
        }
    }

    pub fn record_success(&mut self, key: &ProviderModelHealthKey) {
        let success_required = self.settings.half_open_success_required;
        if let Some(entry) = self.provider_entries.get_mut(&ProviderHealthKey::from(key)) {
            record_entry_success(entry, success_required);
        }

        let entry = self.model_entries.entry(key.clone()).or_default();
        record_entry_success(entry, success_required);
    }

    pub fn record_failure(
        &mut self,
        key: &ProviderModelHealthKey,
        kind: GatewayFailureKind,
        now: DateTime<Utc>,
    ) {
        let weight = classify_failure(kind);
        match weight.scope {
            FailureScope::None => {}
            FailureScope::Model => {
                let threshold = self.settings.model_failure_score_threshold;
                let entry = self.model_entries.entry(key.clone()).or_default();
                add_failure(entry, weight, &self.settings, now);
                if entry.failure_score >= threshold || entry.state == ModelHealthStateKind::Probing
                {
                    open_entry(entry, &self.settings, now);
                } else if entry.failure_score > 0 {
                    entry.state = ModelHealthStateKind::Degraded;
                }
            }
            FailureScope::Provider => {
                let provider_key = ProviderHealthKey::from(key);
                let threshold = self.settings.model_failure_score_threshold;
                let entry = self.provider_entries.entry(provider_key).or_default();
                add_failure(entry, weight, &self.settings, now);
                if entry.failure_score >= threshold || entry.state == ModelHealthStateKind::Probing
                {
                    open_entry(entry, &self.settings, now);
                } else if entry.failure_score > 0 {
                    entry.state = ModelHealthStateKind::Degraded;
                }
            }
        }
    }
}

fn record_entry_success(entry: &mut ModelHealthEntry, half_open_success_required: u32) {
    if entry.state == ModelHealthStateKind::Probing {
        entry.half_open_success_count += 1;
        if entry.half_open_success_count < half_open_success_required {
            return;
        }
    }

    *entry = ModelHealthEntry::default();
}

fn add_failure(
    entry: &mut ModelHealthEntry,
    weight: FailureWeight,
    settings: &ProxyGatewaySettings,
    now: DateTime<Utc>,
) {
    if failure_window_elapsed(entry, settings, now) {
        entry.failure_score = 0;
        entry.half_open_success_count = 0;
        entry.last_error_category = None;
        if entry.state == ModelHealthStateKind::Degraded {
            entry.state = ModelHealthStateKind::Healthy;
        }
    }

    entry.failure_score += weight.score;
    entry.last_failure_at = Some(now);
    entry.last_error_category = Some(weight.category.to_string());
}

fn failure_window_elapsed(
    entry: &ModelHealthEntry,
    settings: &ProxyGatewaySettings,
    now: DateTime<Utc>,
) -> bool {
    entry.last_failure_at.is_some_and(|last_failure_at| {
        now.signed_duration_since(last_failure_at)
            > Duration::seconds(settings.model_failure_window_seconds as i64)
    })
}

fn is_cooling(entry: &ModelHealthEntry, now: DateTime<Utc>) -> bool {
    entry.state == ModelHealthStateKind::CoolingDown
        && entry.next_retry_at.is_none_or(|retry_at| retry_at > now)
}

fn open_entry(entry: &mut ModelHealthEntry, settings: &ProxyGatewaySettings, now: DateTime<Utc>) {
    entry.state = ModelHealthStateKind::CoolingDown;
    entry.consecutive_open_count = entry.consecutive_open_count.saturating_add(1);
    entry.half_open_success_count = 0;
    let exponent = entry.consecutive_open_count.saturating_sub(1).min(16);
    let multiplier = 2_u64.saturating_pow(exponent);
    let cooldown_seconds = settings
        .model_base_cooldown_seconds
        .saturating_mul(multiplier)
        .min(settings.model_max_cooldown_seconds);
    entry.next_retry_at = Some(now + Duration::seconds(cooldown_seconds as i64));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::proxy_gateway::types::GatewayCliKey;

    fn key(model: &str) -> ProviderModelHealthKey {
        ProviderModelHealthKey {
            cli_key: GatewayCliKey::Claude,
            provider_id: "provider-a".to_string(),
            upstream_model_id: model.to_string(),
        }
    }

    fn test_settings() -> ProxyGatewaySettings {
        ProxyGatewaySettings {
            model_failure_score_threshold: 5,
            model_base_cooldown_seconds: 60,
            model_max_cooldown_seconds: 600,
            half_open_success_required: 2,
            ..ProxyGatewaySettings::default()
        }
    }

    #[test]
    fn request_schema_failure_has_no_health_penalty() {
        let failure = classify_failure(GatewayFailureKind::RequestSchema);
        assert_eq!(failure.scope, FailureScope::None);
        assert_eq!(failure.score, 0);
    }

    #[test]
    fn auth_failure_is_provider_wide() {
        let failure = classify_failure(GatewayFailureKind::Auth);
        assert_eq!(failure.scope, FailureScope::Provider);
        assert_eq!(failure.score, 5);
    }

    #[test]
    fn model_failure_does_not_affect_other_models_on_same_provider() {
        let now = Utc::now();
        let mut registry = ModelHealthRegistry::new(test_settings());
        let haiku = key("haiku");
        let opus = key("opus");

        registry.record_failure(&haiku, GatewayFailureKind::ModelNotFound, now);

        assert!(!registry.is_model_available(&haiku, now));
        assert!(registry.is_model_available(&opus, now));
    }

    #[test]
    fn rate_limit_accumulates_degraded_before_cooling() {
        let now = Utc::now();
        let mut registry = ModelHealthRegistry::new(test_settings());
        let haiku = key("haiku");

        registry.record_failure(&haiku, GatewayFailureKind::RateLimit, now);
        let entry = registry.model_entry(&haiku).unwrap();

        assert_eq!(entry.state, ModelHealthStateKind::Degraded);
        assert!(registry.is_model_available(&haiku, now));
    }

    #[test]
    fn repeated_rate_limit_enters_cooling() {
        let now = Utc::now();
        let mut registry = ModelHealthRegistry::new(test_settings());
        let haiku = key("haiku");

        registry.record_failure(&haiku, GatewayFailureKind::RateLimit, now);
        registry.record_failure(&haiku, GatewayFailureKind::RateLimit, now);

        let entry = registry.model_entry(&haiku).unwrap();
        assert_eq!(entry.state, ModelHealthStateKind::CoolingDown);
        assert_eq!(entry.next_retry_at, Some(now + Duration::seconds(60)));
    }

    #[test]
    fn cooling_transitions_to_probing_after_retry_time() {
        let now = Utc::now();
        let mut registry = ModelHealthRegistry::new(test_settings());
        let haiku = key("haiku");

        registry.record_failure(&haiku, GatewayFailureKind::ModelNotFound, now);
        registry.refresh_due_cooldowns(now + Duration::seconds(61));

        assert_eq!(
            registry.model_entry(&haiku).unwrap().state,
            ModelHealthStateKind::Probing
        );
    }

    #[test]
    fn probing_requires_configured_success_count() {
        let now = Utc::now();
        let mut registry = ModelHealthRegistry::new(test_settings());
        let haiku = key("haiku");

        registry.record_failure(&haiku, GatewayFailureKind::ModelNotFound, now);
        registry.refresh_due_cooldowns(now + Duration::seconds(61));
        registry.record_success(&haiku);
        assert_eq!(
            registry.model_entry(&haiku).unwrap().state,
            ModelHealthStateKind::Probing
        );

        registry.record_success(&haiku);
        assert_eq!(
            registry.model_entry(&haiku).unwrap().state,
            ModelHealthStateKind::Healthy
        );
    }

    #[test]
    fn probing_failure_reopens_with_longer_backoff() {
        let now = Utc::now();
        let mut registry = ModelHealthRegistry::new(test_settings());
        let haiku = key("haiku");

        registry.record_failure(&haiku, GatewayFailureKind::ModelNotFound, now);
        registry.refresh_due_cooldowns(now + Duration::seconds(61));
        registry.record_failure(
            &haiku,
            GatewayFailureKind::Upstream5xx,
            now + Duration::seconds(62),
        );

        let entry = registry.model_entry(&haiku).unwrap();
        assert_eq!(entry.state, ModelHealthStateKind::CoolingDown);
        assert_eq!(entry.consecutive_open_count, 2);
        assert_eq!(entry.next_retry_at, Some(now + Duration::seconds(62 + 120)));
    }

    #[test]
    fn auth_failure_cools_provider_and_blocks_all_models() {
        let now = Utc::now();
        let mut registry = ModelHealthRegistry::new(test_settings());
        let haiku = key("haiku");
        let opus = key("opus");

        registry.record_failure(&haiku, GatewayFailureKind::Auth, now);

        assert!(!registry.is_model_available(&haiku, now));
        assert!(!registry.is_model_available(&opus, now));
        assert!(registry
            .provider_entry(&ProviderHealthKey::from(&haiku))
            .is_some());
    }

    #[test]
    fn provider_probe_success_resets_provider_health() {
        let now = Utc::now();
        let mut registry = ModelHealthRegistry::new(test_settings());
        let haiku = key("haiku");
        let provider_key = ProviderHealthKey::from(&haiku);

        registry.record_failure(&haiku, GatewayFailureKind::Auth, now);
        registry.refresh_due_cooldowns(now + Duration::seconds(61));
        assert_eq!(
            registry.provider_entry(&provider_key).unwrap().state,
            ModelHealthStateKind::Probing
        );

        registry.record_success(&haiku);
        assert_eq!(
            registry.provider_entry(&provider_key).unwrap().state,
            ModelHealthStateKind::Probing
        );

        registry.record_success(&haiku);
        let entry = registry.provider_entry(&provider_key).unwrap();
        assert_eq!(entry.state, ModelHealthStateKind::Healthy);
        assert_eq!(entry.failure_score, 0);
        assert_eq!(entry.consecutive_open_count, 0);

        let next_failure_at = now + Duration::seconds(70);
        registry.record_failure(&haiku, GatewayFailureKind::Auth, next_failure_at);
        let entry = registry.provider_entry(&provider_key).unwrap();
        assert_eq!(entry.consecutive_open_count, 1);
        assert_eq!(
            entry.next_retry_at,
            Some(next_failure_at + Duration::seconds(60))
        );
    }

    #[test]
    fn model_failure_score_resets_after_failure_window() {
        let now = Utc::now();
        let mut settings = test_settings();
        settings.model_failure_window_seconds = 30;
        let mut registry = ModelHealthRegistry::new(settings);
        let haiku = key("haiku");

        registry.record_failure(&haiku, GatewayFailureKind::RateLimit, now);
        let entry = registry.model_entry(&haiku).unwrap();
        assert_eq!(entry.state, ModelHealthStateKind::Degraded);
        assert_eq!(entry.failure_score, 3);
        assert_eq!(entry.last_failure_at, Some(now));

        let outside_window = now + Duration::seconds(31);
        registry.record_failure(&haiku, GatewayFailureKind::RateLimit, outside_window);
        let entry = registry.model_entry(&haiku).unwrap();
        assert_eq!(entry.state, ModelHealthStateKind::Degraded);
        assert_eq!(entry.failure_score, 3);
        assert_eq!(entry.last_failure_at, Some(outside_window));
        assert!(registry.is_model_available(&haiku, outside_window));

        let inside_window = outside_window + Duration::seconds(1);
        registry.record_failure(&haiku, GatewayFailureKind::RateLimit, inside_window);
        let entry = registry.model_entry(&haiku).unwrap();
        assert_eq!(entry.state, ModelHealthStateKind::CoolingDown);
        assert_eq!(entry.failure_score, 6);
    }
}
