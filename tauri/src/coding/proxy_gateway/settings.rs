use super::listen::validate_settings;
use super::types::ProxyGatewaySettings;
use crate::db::helpers::{db_get, db_put};
use crate::db::schema::DbTable;
use crate::db::sqlite_state::SqliteDbState;
use serde_json::Value;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

const SETTINGS_RECORD_QUERY: &str =
    "SELECT * OMIT id FROM proxy_gateway_settings:`gateway` LIMIT 1";
const SETTINGS_UPSERT_QUERY: &str = "UPSERT proxy_gateway_settings:`gateway` CONTENT $data";
const SETTINGS_ID: &str = "gateway";

pub fn load_settings_from_sqlite_state(
    sqlite_state: &SqliteDbState,
) -> Result<ProxyGatewaySettings, String> {
    sqlite_state.with_conn(|conn| {
        let Some(record) = db_get(conn, DbTable::ProxyGatewaySettings, SETTINGS_ID)? else {
            return Ok(ProxyGatewaySettings::default());
        };
        settings_from_value(record)
    })
}

pub fn save_settings_to_sqlite_state(
    sqlite_state: &SqliteDbState,
    settings: ProxyGatewaySettings,
) -> Result<ProxyGatewaySettings, String> {
    validate_settings(&settings)?;
    let data = serde_json::to_value(&settings)
        .map_err(|error| format!("Failed to serialize proxy gateway settings: {error}"))?;
    sqlite_state
        .with_conn(|conn| db_put(conn, DbTable::ProxyGatewaySettings, SETTINGS_ID, &data))?;
    Ok(settings)
}

pub async fn save_settings_dual(
    sqlite_state: &SqliteDbState,
    db: &Surreal<Db>,
    settings: ProxyGatewaySettings,
) -> Result<ProxyGatewaySettings, String> {
    let settings = save_settings_to_sqlite_state(sqlite_state, settings)?;
    save_settings(db, settings.clone()).await?;
    Ok(settings)
}

pub async fn sync_sqlite_settings_from_surreal_if_missing(
    sqlite_state: &SqliteDbState,
    db: &Surreal<Db>,
) -> Result<(), String> {
    let has_sqlite_settings = sqlite_state.with_conn(|conn| {
        Ok(db_get(conn, DbTable::ProxyGatewaySettings, SETTINGS_ID)?.is_some())
    })?;
    if has_sqlite_settings {
        return Ok(());
    }

    let mut result = db
        .query(SETTINGS_RECORD_QUERY)
        .await
        .map_err(|error| format!("Failed to query proxy gateway settings: {error}"))?;
    let records: Vec<Value> = result
        .take(0)
        .map_err(|error| format!("Failed to parse proxy gateway settings: {error}"))?;
    let Some(record) = records.into_iter().next() else {
        return Ok(());
    };

    let settings = settings_from_value(record)?;
    save_settings_to_sqlite_state(sqlite_state, settings)?;
    Ok(())
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
    use crate::db::sqlite_state::SqliteDbState;
    use serde_json::json;

    #[test]
    fn missing_settings_fields_use_defaults() {
        let settings = settings_from_value(json!({})).unwrap();
        assert_eq!(settings.listen_host, "127.0.0.1");
        assert_eq!(settings.listen_port, 37123);
        assert!(settings.metrics_enabled);
        assert!(!settings.enabled_on_startup);
        assert_eq!(settings.per_provider_retry_count, 0);
        assert_eq!(settings.max_retry_count, 8);
        assert!(settings.thinking_rectifier_enabled);
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

    #[test]
    fn retry_count_cannot_exceed_global_retry_count() {
        assert!(settings_from_value(json!({
            "per_provider_retry_count": 3,
            "max_retry_count": 2,
        }))
        .is_err());
    }

    #[test]
    fn sqlite_settings_round_trip_uses_defaults_and_validation() {
        let sqlite_state = SqliteDbState::in_memory_for_test().expect("sqlite");

        let defaults = load_settings_from_sqlite_state(&sqlite_state).expect("load defaults");
        assert_eq!(defaults.listen_host, "127.0.0.1");
        assert_eq!(defaults.listen_port, 37123);

        let mut settings = defaults;
        settings.listen_port = 38123;
        settings.enabled_on_startup = true;
        save_settings_to_sqlite_state(&sqlite_state, settings).expect("save settings");

        let loaded = load_settings_from_sqlite_state(&sqlite_state).expect("reload settings");
        assert_eq!(loaded.listen_port, 38123);
        assert!(loaded.enabled_on_startup);
    }
}
