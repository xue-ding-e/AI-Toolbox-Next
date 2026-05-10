use serde_json::Value;

use super::{MigrationOutcome, load_table_records, mark_migration_applied};
use crate::coding::db_record_id;
use crate::coding::skills::central_repo::skill_storage_dir_name;

pub const MIGRATION_ID: &str = "2026_04_09_skills_restore_name_normalization_v1";

pub fn run_migration<'a>(
    db: &'a surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> super::MigrationFuture<'a> {
    Box::pin(async move {
        let records = load_table_records(db, "skill").await?;
        let mut updated_count = 0usize;

        for record in records {
            let Some(skill_id) = record.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(name) = record.get("name").and_then(Value::as_str) else {
                continue;
            };

            let normalized_name = skill_storage_dir_name(name);
            let Some(next_central_path) = normalize_central_path(
                record.get("central_path").and_then(Value::as_str),
                &normalized_name,
            ) else {
                continue;
            };

            let current_central_path = record
                .get("central_path")
                .and_then(Value::as_str)
                .unwrap_or_default();

            if normalized_name == name && next_central_path == current_central_path {
                continue;
            }

            let record_id = db_record_id("skill", skill_id);
            db.query(format!(
                "UPDATE {} SET name = $name, central_path = $central_path",
                record_id
            ))
            .bind(("name", normalized_name))
            .bind(("central_path", next_central_path))
            .await
            .map_err(|error| {
                format!(
                    "Failed to normalize restored skill metadata for '{}': {}",
                    skill_id, error
                )
            })?;
            updated_count += 1;
        }

        let status = format!("updated={updated_count}");
        mark_migration_applied(db, MIGRATION_ID, &status).await?;

        Ok(if updated_count > 0 {
            MigrationOutcome::Applied
        } else {
            MigrationOutcome::SkippedNoOp
        })
    })
}

fn normalize_central_path(current: Option<&str>, normalized_name: &str) -> Option<String> {
    let current = current?;
    let trimmed = current.trim();
    if trimmed.is_empty() {
        return Some(normalized_name.to_string());
    }

    let normalized_path = trimmed.replace('\\', "/");
    let mut segments: Vec<&str> = normalized_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if segments.is_empty() {
        return Some(normalized_name.to_string());
    }

    if normalized_path.starts_with('/') {
        return Some(normalized_name.to_string());
    }

    if normalized_path.len() >= 3 {
        let bytes = normalized_path.as_bytes();
        if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/' {
            return Some(normalized_name.to_string());
        }
    }

    if let Some(first_segment) = segments.first_mut() {
        *first_segment = normalized_name;
    }

    Some(segments.join("/"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn normalize_central_path_rewrites_relative_root_segment() {
        let normalized = super::normalize_central_path(Some("foo:bar/examples/demo"), "foo_bar")
            .expect("expected normalized path");
        assert_eq!(normalized, "foo_bar/examples/demo");
    }

    #[test]
    fn normalize_central_path_rewrites_legacy_absolute_path_to_name_only() {
        let normalized =
            super::normalize_central_path(Some(r"C:\Users\tester\.skills\CON"), "CON_")
                .expect("expected normalized path");
        assert_eq!(normalized, "CON_");
    }

    #[test]
    fn normalize_central_path_defaults_empty_path_to_normalized_name() {
        let normalized = super::normalize_central_path(Some(""), "unnamed-skill")
            .expect("expected normalized path");
        assert_eq!(normalized, "unnamed-skill");
    }
}
