use serde_json::Value;

use crate::coding::oh_my_openagent::commands::{
    OH_MY_OPENAGENT_CONFIG_TABLE, OH_MY_OPENAGENT_GLOBAL_CONFIG_TABLE,
};
use crate::db_migration::{
    count_records, load_table_records, mark_migration_applied, migration_record_id,
    MigrationOutcome,
};

const LEGACY_CONFIG_TABLE: &str = "oh_my_opencode_config";
const LEGACY_GLOBAL_CONFIG_TABLE: &str = "oh_my_opencode_global_config";
pub const MIGRATION_ID: &str = "oh_my_openagent_rename_v1";

fn clean_record_id(raw_id: &str) -> String {
    let without_prefix = if let Some(pos) = raw_id.find(':') {
        &raw_id[pos + 1..]
    } else {
        raw_id
    };

    without_prefix
        .trim_start_matches('⟨')
        .trim_end_matches('⟩')
        .trim_start_matches('`')
        .trim_end_matches('`')
        .to_string()
}

/// Rename the persisted main-plugin tables from the legacy OpenCode name to
/// the canonical OpenAgent name.
///
/// Behavior:
/// - fresh installs: no legacy data exists, so this becomes a no-op and still
///   writes a marker to avoid repeated startup probes;
/// - normal upgrades: copies all legacy records into canonical tables inside a
///   single transaction, writes the marker, then deletes legacy rows;
/// - ambiguous databases: if both legacy and canonical tables already contain
///   data, startup is aborted so we never merge user data implicitly.
pub fn run_migration(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<MigrationOutcome, String>> + Send + '_>,
> {
    Box::pin(async move {
        let legacy_config_count = count_records(db, LEGACY_CONFIG_TABLE).await?;
        let legacy_global_count = count_records(db, LEGACY_GLOBAL_CONFIG_TABLE).await?;
        let canonical_config_count = count_records(db, OH_MY_OPENAGENT_CONFIG_TABLE).await?;
        let canonical_global_count = count_records(db, OH_MY_OPENAGENT_GLOBAL_CONFIG_TABLE).await?;

        let has_legacy_data = legacy_config_count > 0 || legacy_global_count > 0;
        let has_canonical_data = canonical_config_count > 0 || canonical_global_count > 0;

        if !has_legacy_data {
            mark_migration_applied(db, MIGRATION_ID, "skipped_noop").await?;
            return Ok(MigrationOutcome::SkippedNoOp);
        }

        if has_canonical_data {
            return Err(format!(
                "Database migration '{}' aborted: both legacy tables ({}/{}) and canonical tables ({}/{}) already contain data. Refusing to merge automatically.",
                MIGRATION_ID,
                LEGACY_CONFIG_TABLE,
                LEGACY_GLOBAL_CONFIG_TABLE,
                OH_MY_OPENAGENT_CONFIG_TABLE,
                OH_MY_OPENAGENT_GLOBAL_CONFIG_TABLE
            ));
        }

        let legacy_configs = load_table_records(db, LEGACY_CONFIG_TABLE).await?;
        let legacy_global_configs = load_table_records(db, LEGACY_GLOBAL_CONFIG_TABLE).await?;

        let mut transaction = String::from("BEGIN TRANSACTION;\n");

        for record in &legacy_configs {
            transaction.push_str(&build_upsert_statement(
                OH_MY_OPENAGENT_CONFIG_TABLE,
                record,
            )?);
            transaction.push_str(";\n");
        }

        for record in &legacy_global_configs {
            transaction.push_str(&build_upsert_statement(
                OH_MY_OPENAGENT_GLOBAL_CONFIG_TABLE,
                record,
            )?);
            transaction.push_str(";\n");
        }

        let migration_record_id = migration_record_id(MIGRATION_ID);
        transaction.push_str(&format!(
            "UPSERT {} CONTENT {{ migration_id: '{}', status: 'applied', applied_at: time::now() }};\n",
            migration_record_id, MIGRATION_ID
        ));
        transaction.push_str(&format!("DELETE {};\n", LEGACY_CONFIG_TABLE));
        transaction.push_str(&format!("DELETE {};\n", LEGACY_GLOBAL_CONFIG_TABLE));
        transaction.push_str("COMMIT TRANSACTION;");

        db.query(transaction).await.map_err(|error| {
            format!(
                "Failed to apply database migration '{}': {}",
                MIGRATION_ID, error
            )
        })?;

        Ok(MigrationOutcome::Applied)
    })
}

fn build_upsert_statement(target_table: &str, record: &Value) -> Result<String, String> {
    let record_object = record
        .as_object()
        .ok_or_else(|| format!("Expected object record when migrating {}", target_table))?;

    let record_id = record_object
        .get("id")
        .and_then(|value| value.as_str())
        .map(clean_record_id)
        .ok_or_else(|| format!("Failed to extract SurrealDB record id for {}", target_table))?;

    let mut payload = record.clone();
    if let Some(payload_object) = payload.as_object_mut() {
        payload_object.remove("id");
    }

    let payload_json = serde_json::to_string(&payload).map_err(|error| {
        format!(
            "Failed to serialize migration payload for {}: {}",
            target_table, error
        )
    })?;

    Ok(format!(
        "UPSERT {}:`{}` CONTENT {}",
        target_table, record_id, payload_json
    ))
}
