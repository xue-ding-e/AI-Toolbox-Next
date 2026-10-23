//! Internal master-data half of the GUI-owned context operation.
//!
//! This is not an exposed operation or an effects executor. The owning adapter
//! must establish profile/runtime ownership and admission before calling it.
use ai_toolbox_codex_config::plan::Target;
use ai_toolbox_storage::helpers::{db_get, db_patch_fields, db_query_by_bool};
use ai_toolbox_storage::schema::{DbTable, JsonFieldPath};
use rusqlite::{Connection, Transaction};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MasterError {
    Storage,
    MissingCommon,
    NoAppliedProvider,
    AmbiguousAppliedProvider,
    InvalidRecord,
    ProviderUnavailable,
    Conflict,
}

// Deliberately neither Debug nor Serialize: these records may contain secrets.
pub(crate) struct MasterSnapshot {
    common: Value,
    provider: Value,
    settings: Value,
    settings_field: &'static str,
    pub(crate) provider_id: String,
    pub(crate) common_config: String,
    pub(crate) provider_config: String,
}

fn config_text(record: &Value) -> Result<String, MasterError> {
    match record.get("config") {
        None => Ok(String::new()),
        Some(Value::String(text)) => Ok(text.clone()),
        _ => Err(MasterError::InvalidRecord),
    }
}

/// Read existing records only. Never call the desktop list/bootstrap adapter:
/// listing can import a default provider, and opening storage can migrate/seed.
pub(crate) fn read_master(conn: &Connection) -> Result<MasterSnapshot, MasterError> {
    let common = db_get(conn, DbTable::CodexCommonConfig, "common")
        .map_err(|_| MasterError::Storage)?
        .ok_or(MasterError::MissingCommon)?;
    if !common.is_object() {
        return Err(MasterError::InvalidRecord);
    }
    let field = JsonFieldPath::new("is_applied").map_err(|_| MasterError::InvalidRecord)?;
    let mut applied = db_query_by_bool(conn, DbTable::CodexProvider, &field, true, None, Some(2))
        .map_err(|_| MasterError::Storage)?;
    match applied.len() {
        0 => return Err(MasterError::NoAppliedProvider),
        1 => (),
        _ => return Err(MasterError::AmbiguousAppliedProvider),
    }
    let provider = applied.remove(0);
    let provider_id = provider
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && *id != "__local__")
        .ok_or(MasterError::ProviderUnavailable)?
        .to_owned();
    // The query helper has coercion semantics. Do not use that as proof that
    // malformed stored values satisfy the new typed operation contract.
    if provider.get("is_applied").and_then(Value::as_bool) != Some(true) {
        return Err(MasterError::InvalidRecord);
    }
    match provider
        .get("is_disabled")
        .or_else(|| provider.get("isDisabled"))
    {
        None | Some(Value::Bool(false)) => (),
        Some(Value::Bool(true)) => return Err(MasterError::ProviderUnavailable),
        _ => return Err(MasterError::InvalidRecord),
    }
    // Match the desktop adapter: settings_config is the authoritative stored field.
    let settings_field = "settings_config";
    let settings_text = provider
        .get(settings_field)
        .and_then(Value::as_str)
        .ok_or(MasterError::InvalidRecord)?;
    let settings: Value =
        serde_json::from_str(settings_text).map_err(|_| MasterError::InvalidRecord)?;
    if !settings.is_object() {
        return Err(MasterError::InvalidRecord);
    }
    let common_config = config_text(&common)?;
    let provider_config = config_text(&settings)?;
    Ok(MasterSnapshot {
        common,
        provider,
        settings,
        settings_field,
        provider_id,
        common_config,
        provider_config,
    })
}

impl MasterSnapshot {
    pub(crate) fn revision_bytes(&self) -> Result<Vec<u8>, MasterError> {
        serde_json::to_vec(&(&self.common, &self.provider)).map_err(|_| MasterError::InvalidRecord)
    }

    /// Initial internal effect slice requires an explicit stored absolute root.
    /// Default HOME/runtime-location resolution belongs to the later adapter.
    pub(crate) fn explicit_root(&self) -> Result<&str, MasterError> {
        self.common
            .get("root_dir")
            .and_then(Value::as_str)
            .filter(|root| !root.is_empty() && std::path::Path::new(root).is_absolute())
            .ok_or(MasterError::InvalidRecord)
    }
    /// Exact internal comparison includes both common/provider JSON records, so
    /// their root or configuration changes cannot hide behind a matching context pair.
    /// Separate account-table records are not covered by this comparison.
    pub(crate) fn unchanged_from(&self, current: &Self) -> bool {
        self.common == current.common && self.provider == current.provider
    }
}

/// Must run in the same caller-owned transaction as the initial journal row.
/// No file write, event, runtime refresh, account operation or second DB open.
/// The caller must perform replay lookup before invoking this function.
pub(crate) fn patch_master(
    tx: &Transaction<'_>,
    expected: &MasterSnapshot,
    target: Target,
    candidate: &str,
) -> Result<bool, MasterError> {
    let current = read_master(tx)?;
    if !expected.unchanged_from(&current) {
        return Err(MasterError::Conflict);
    }
    let original = match target {
        Target::Common => &current.common_config,
        Target::Provider => &current.provider_config,
    };
    if original == candidate {
        return Ok(false);
    }
    let (table, id, field, value) = match target {
        Target::Common => (
            DbTable::CodexCommonConfig,
            "common",
            "config",
            Value::String(candidate.to_owned()),
        ),
        Target::Provider => {
            let mut settings = current.settings.clone();
            settings
                .as_object_mut()
                .ok_or(MasterError::InvalidRecord)?
                .insert("config".to_owned(), Value::String(candidate.to_owned()));
            let serialized =
                serde_json::to_string(&settings).map_err(|_| MasterError::InvalidRecord)?;
            (
                DbTable::CodexProvider,
                current.provider_id.as_str(),
                current.settings_field,
                Value::String(serialized),
            )
        }
    };
    db_patch_fields(tx, table, id, &[(field, value)])
        .map_err(|_| MasterError::Storage)?
        .ok_or(MasterError::Conflict)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_toolbox_storage::{helpers::db_put, SqliteDbState};
    use serde_json::json;

    fn seeded() -> SqliteDbState {
        let state = SqliteDbState::in_memory_for_test().unwrap();
        state
            .with_conn(|conn| {
                db_put(
                    conn,
                    DbTable::CodexCommonConfig,
                    "common",
                    &json!({
                        "config":"model_context_window = 1000000\n",
                        "root_dir":"synthetic-root", "unknown":{"keep":true}
                    }),
                )?;
                db_put(
                    conn,
                    DbTable::CodexProvider,
                    "synthetic-provider",
                    &json!({
                        "is_applied":true,"is_disabled":false,"unknown_outer":{"keep":7},
                        "settings_config":json!({
                            "config":"model_auto_compact_token_limit = 900000\n",
                            "auth":{"SYNTHETIC":"not-a-credential"},
                            "models":[{"id":"synthetic-model"}],"unknown":{"nested":true}
                        }).to_string()
                    }),
                )
            })
            .unwrap();
        state
    }

    #[test]
    fn master_common_patch_preserves_non_config_fields() {
        let state = seeded();
        state
            .with_conn_mut(|conn| {
                let before = read_master(conn).unwrap();
                let tx = conn.transaction().unwrap();
                assert!(patch_master(
                    &tx,
                    &before,
                    Target::Common,
                    "model_context_window = 2000000\n"
                )
                .unwrap());
                tx.commit().unwrap();
                let after = read_master(conn).unwrap();
                assert_eq!(after.common["root_dir"], before.common["root_dir"]);
                assert_eq!(after.common["unknown"], before.common["unknown"]);
                assert_eq!(after.provider, before.provider);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn master_provider_string_patch_preserves_auth_models_and_unknowns() {
        let state = seeded();
        state
            .with_conn_mut(|conn| {
                let before = read_master(conn).unwrap();
                let tx = conn.transaction().unwrap();
                patch_master(
                    &tx,
                    &before,
                    Target::Provider,
                    "model_auto_compact_token_limit = 800000\n",
                )
                .unwrap();
                tx.commit().unwrap();
                let after = read_master(conn).unwrap();
                assert!(after.provider["settings_config"].is_string());
                for field in ["auth", "models", "unknown"] {
                    assert_eq!(after.settings[field], before.settings[field]);
                }
                assert_eq!(
                    after.provider["unknown_outer"],
                    before.provider["unknown_outer"]
                );
                assert_eq!(after.common, before.common);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn master_noop_and_rollback_preserve_original_records() {
        let state = seeded();
        state
            .with_conn_mut(|conn| {
                let before = read_master(conn).unwrap();
                let tx = conn.transaction().unwrap();
                assert!(
                    !patch_master(&tx, &before, Target::Provider, &before.provider_config).unwrap()
                );
                tx.commit().unwrap();
                assert!(before.unchanged_from(&read_master(conn).unwrap()));
                let tx = conn.transaction().unwrap();
                patch_master(
                    &tx,
                    &before,
                    Target::Common,
                    "model_context_window = 2000000\n",
                )
                .unwrap();
                tx.rollback().unwrap();
                assert!(before.unchanged_from(&read_master(conn).unwrap()));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn master_rejects_ambiguous_applied_rows() {
        let state = seeded();
        state
            .with_conn(|conn| {
                db_put(
                    conn,
                    DbTable::CodexProvider,
                    "second",
                    &json!({"is_applied":true}),
                )?;
                assert!(matches!(
                    read_master(conn),
                    Err(MasterError::AmbiguousAppliedProvider)
                ));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn master_detects_unrelated_record_drift_before_patch() {
        let state = seeded();
        state
            .with_conn_mut(|conn| {
                let before = read_master(conn).unwrap();
                db_patch_fields(
                    conn,
                    DbTable::CodexCommonConfig,
                    "common",
                    &[("unknown", json!({"changed":true}))],
                )?;
                let tx = conn.transaction().unwrap();
                assert_eq!(
                    patch_master(
                        &tx,
                        &before,
                        Target::Common,
                        "model_context_window = 2000000\n"
                    ),
                    Err(MasterError::Conflict)
                );
                tx.rollback().unwrap();
                assert_eq!(
                    read_master(conn).unwrap().common_config,
                    before.common_config
                );
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn master_disabled_alias_matches_desktop_precedence_but_rejects_invalid_types() {
        for (snake, camel, expected) in [
            (
                None,
                Some(json!(true)),
                Some(MasterError::ProviderUnavailable),
            ),
            (None, Some(json!(false)), None),
            (Some(json!(false)), Some(json!(true)), None),
            (
                Some(json!(true)),
                Some(json!(false)),
                Some(MasterError::ProviderUnavailable),
            ),
            (
                Some(json!("false")),
                Some(json!(false)),
                Some(MasterError::InvalidRecord),
            ),
            (None, Some(json!("false")), Some(MasterError::InvalidRecord)),
        ] {
            let state = seeded();
            state
                .with_conn(|conn| {
                    let mut row =
                        db_get(conn, DbTable::CodexProvider, "synthetic-provider")?.unwrap();
                    let object = row.as_object_mut().unwrap();
                    object.remove("is_disabled");
                    if let Some(value) = snake {
                        object.insert("is_disabled".into(), value);
                    }
                    if let Some(value) = camel {
                        object.insert("isDisabled".into(), value);
                    }
                    db_put(conn, DbTable::CodexProvider, "synthetic-provider", &row)?;
                    assert_eq!(read_master(conn).err(), expected);
                    Ok(())
                })
                .unwrap();
        }
    }

    #[test]
    fn master_rejects_settings_alias_not_consumed_by_desktop() {
        let state = seeded();
        state
            .with_conn(|conn| {
                let mut row = db_get(conn, DbTable::CodexProvider, "synthetic-provider")?.unwrap();
                let object = row.as_object_mut().unwrap();
                let settings = object.remove("settings_config").unwrap();
                object.insert("settingsConfig".into(), settings);
                db_put(conn, DbTable::CodexProvider, "synthetic-provider", &row)?;
                assert!(matches!(read_master(conn), Err(MasterError::InvalidRecord)));
                Ok(())
            })
            .unwrap();
    }
}
