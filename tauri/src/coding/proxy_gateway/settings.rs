use super::listen::validate_settings;
use super::types::ProxyGatewaySettings;
use serde_json::Value;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

const SETTINGS_RECORD_QUERY: &str =
    "SELECT * OMIT id FROM proxy_gateway_settings:`gateway` LIMIT 1";
const SETTINGS_UPSERT_QUERY: &str = "UPSERT proxy_gateway_settings:`gateway` CONTENT $data";

pub async fn load_settings(db: &Surreal<Db>) -> Result<ProxyGatewaySettings, String> {
    let mut result = db
        .query(SETTINGS_RECORD_QUERY)
        .await
        .map_err(|error| format!("Failed to query proxy gateway settings: {error}"))?;

    let records: Vec<Value> = result
        .take(0)
        .map_err(|error| format!("Failed to parse proxy gateway settings: {error}"))?;

    let Some(record) = records.into_iter().next() else {
        return Ok(ProxyGatewaySettings::default());
    };

    settings_from_value(record)
}

pub async fn save_settings(
    db: &Surreal<Db>,
    settings: ProxyGatewaySettings,
) -> Result<ProxyGatewaySettings, String> {
    validate_settings(&settings)?;
    let data = serde_json::to_value(&settings)
        .map_err(|error| format!("Failed to serialize proxy gateway settings: {error}"))?;
    db.query(SETTINGS_UPSERT_QUERY)
        .bind(("data", data))
        .await
        .map_err(|error| format!("Failed to save proxy gateway settings: {error}"))?;
    Ok(settings)
}

pub fn settings_from_value(value: Value) -> Result<ProxyGatewaySettings, String> {
    let mut settings: ProxyGatewaySettings =
        serde_json::from_value(value).unwrap_or_else(|_| ProxyGatewaySettings::default());
    if settings.enabled_cli_keys.is_empty() {
        settings.enabled_cli_keys = ProxyGatewaySettings::default().enabled_cli_keys;
    }
    validate_settings(&settings)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::proxy_gateway::types::GatewayCliKey;
    use serde_json::json;

    #[test]
    fn missing_settings_fields_use_defaults() {
        let settings = settings_from_value(json!({})).unwrap();
        assert_eq!(settings.listen_host, "127.0.0.1");
        assert_eq!(settings.listen_port, 37123);
        assert!(settings.metrics_enabled);
        assert!(!settings.enabled_on_startup);
    }

    #[test]
    fn enabled_on_startup_preserves_explicit_true() {
        let settings = settings_from_value(json!({
            "enabled_on_startup": true,
        }))
        .unwrap();

        assert!(settings.enabled_on_startup);
    }

    #[test]
    fn empty_enabled_cli_keys_are_repaired_to_mvp_defaults() {
        let settings = settings_from_value(json!({
            "enabled_cli_keys": []
        }))
        .unwrap();

        assert_eq!(
            settings.enabled_cli_keys,
            vec![
                GatewayCliKey::Claude,
                GatewayCliKey::Codex,
                GatewayCliKey::Gemini
            ]
        );
    }

    #[test]
    fn invalid_persisted_host_is_rejected() {
        assert!(settings_from_value(json!({
            "listen_host": "http://127.0.0.1"
        }))
        .is_err());
    }
}
