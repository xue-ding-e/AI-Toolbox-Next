//! Backup repository connection settings (`settings:backup_repository`).
//!
//! The repository connection lives in its own record so the token never joins the
//! AppSettings payload; the frontend only receives `has_token`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Manager;

use super::credentials::{self, backup_error};
use super::repository::validate_config;
use crate::db::helpers::{db_get, db_patch_fields, db_put, db_transaction};
use crate::db::schema::DbTable;
use crate::db::SqliteDbState;
use crate::settings::store;
use crate::settings::types::BackupEncryptionConfig;

pub const BACKUP_REPOSITORY_SETTINGS_ID: &str = "backup_repository";
/// Retired configuration-sync scheme record. Users of the never-released preview
/// may still carry a connection there; it migrates on first read of the new record.
const LEGACY_REPOSITORY_SYNC_ID: &str = "repository_sync";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackupRepositoryPlatform {
    #[default]
    Github,
    Gitee,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BackupRepositoryConfig {
    pub platform: BackupRepositoryPlatform,
    pub owner: String,
    pub repository: String,
    pub branch: String,
    pub directory: String,
}

impl BackupRepositoryConfig {
    pub fn is_unconfigured(&self) -> bool {
        self.owner.is_empty() && self.repository.is_empty() && self.branch.is_empty()
    }

    /// True when no usable connection is present: owner and repository are both
    /// empty. Unlike `is_unconfigured` this ignores branch/directory defaults, so
    /// a fresh-install form draft (branch "main") still counts as "no connection"
    /// instead of failing owner/repo validation and blocking saves on the other
    /// storage channels.
    pub fn is_blank_connection(&self) -> bool {
        self.owner.trim().is_empty() && self.repository.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupRepositorySettings {
    pub config: BackupRepositoryConfig,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupRepositoryView {
    pub config: BackupRepositoryConfig,
    pub has_token: bool,
}

impl From<BackupRepositorySettings> for BackupRepositoryView {
    fn from(settings: BackupRepositorySettings) -> Self {
        Self {
            config: settings.config,
            has_token: !settings.token.is_empty(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupEncryptionStatus {
    pub enabled: bool,
    pub has_password: bool,
    /// False when the OS credential store could not be read on this machine:
    /// `has_password` is then unknown rather than a confirmed "no password".
    pub password_known: bool,
}

pub(crate) fn normalize(mut config: BackupRepositoryConfig) -> BackupRepositoryConfig {
    config.owner = config.owner.trim().to_string();
    config.repository = config.repository.trim().to_string();
    config.branch = config.branch.trim().to_string();
    config.directory = config.directory.trim_matches('/').trim().to_string();
    config
}

/// Load the backup repository connection. A missing record falls back to the
/// retired `settings:repository_sync` connection (one-time migration, connection
/// fields and token only — scopes/baselines are obsolete), and a missing legacy
/// record simply means the repository channel is not configured yet.
pub fn load_backup_repository_settings(
    db: &SqliteDbState,
) -> Result<BackupRepositorySettings, String> {
    let existing = db.with_conn(|connection| {
        db_get(connection, DbTable::Settings, BACKUP_REPOSITORY_SETTINGS_ID)?
            .map(serde_json::from_value::<BackupRepositorySettings>)
            .transpose()
            .map_err(|error| error.to_string())
    })?;
    if let Some(settings) = existing {
        return Ok(settings);
    }

    let migrated = db.with_conn(|connection| {
        db_get(connection, DbTable::Settings, LEGACY_REPOSITORY_SYNC_ID)?
            .map(legacy_sync_to_backup_repository)
            .transpose()
            .map_err(|error| error.to_string())
    })?;
    let Some(settings) = migrated else {
        return Ok(BackupRepositorySettings::default());
    };

    // Persist the migrated connection so every later read is stable and the legacy
    // record can stay untouched. A persistence failure must not lose the value for
    // this load, so it is only logged.
    if let Ok(value) = serde_json::to_value(&settings) {
        if let Err(error) =
            db.with_conn(|connection| db_put(connection, DbTable::Settings, BACKUP_REPOSITORY_SETTINGS_ID, &value))
        {
            log::warn!("Failed to persist migrated backup repository connection: {error}");
        }
    }
    Ok(settings)
}

/// Map a retired `settings:repository_sync` record to the backup connection shape.
/// Only the connection fields and the token carry over; scopes, baselines and sync
/// status are obsolete by design. Unknown/missing fields degrade to empty strings.
fn legacy_sync_to_backup_repository(value: Value) -> Result<BackupRepositorySettings, String> {
    let config = value.get("config").cloned().unwrap_or(Value::Null);
    let string_field = |key: &str| -> String {
        config
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Ok(BackupRepositorySettings {
        config: BackupRepositoryConfig {
            platform: match config.get("platform").and_then(Value::as_str) {
                Some("gitee") => BackupRepositoryPlatform::Gitee,
                _ => BackupRepositoryPlatform::Github,
            },
            owner: string_field("owner"),
            repository: string_field("repository"),
            branch: string_field("branch"),
            directory: string_field("directory"),
        },
        token: value
            .get("token")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

/// Resolve the token for a repository draft. A non-empty draft token replaces the
/// stored one; an empty draft token reuses the stored token only when the platform
/// is unchanged — a token only ever works for the platform it was created for, so
/// a platform switch without a new token fails before any network request. Shared
/// by the save path and the test-connection path.
pub(crate) fn resolve_repository_token(
    previous: &BackupRepositorySettings,
    draft_config: &BackupRepositoryConfig,
    draft_token: Option<String>,
) -> Result<String, String> {
    match draft_token
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
    {
        Some(token) => Ok(token),
        None => {
            if previous.config.platform != draft_config.platform && !previous.token.is_empty() {
                return Err(backup_error(
                    "tokenRequired",
                    "settings.backupSettings.repository.errors.tokenRequired",
                    "switching platforms requires a new token",
                ));
            }
            Ok(previous.token.clone())
        }
    }
}

#[tauri::command]
pub async fn get_backup_repository_settings(
    db: tauri::State<'_, SqliteDbState>,
) -> Result<BackupRepositoryView, String> {
    Ok(load_backup_repository_settings(&db)?.into())
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupSettingsSaveOutcome {
    pub repository: BackupRepositoryView,
    pub encryption: BackupEncryptionStatus,
}

/// Payload for the unified backup settings save entry. Only backup-related
/// AppSettings fields are patched, so concurrent writes to other settings survive.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BackupSettingsPayload {
    pub backup_type: String,
    pub local_backup_path: String,
    pub webdav: crate::settings::types::WebDAVConfig,
    pub backup_encryption_enabled: bool,
    /// `None` keeps the stored credential; a non-empty value replaces it.
    pub encryption_password: Option<String>,
    pub repository: BackupRepositoryConfig,
    /// `None`/empty keeps the stored token (platform switch still requires a new one).
    pub repository_token: Option<String>,
    pub backup_image_assets_enabled: bool,
    pub backup_cli_config_files_enabled: bool,
    pub backup_custom_entries: Vec<crate::settings::types::BackupCustomEntry>,
    pub backup_file_filter_rules: Vec<crate::settings::types::BackupFileFilterRule>,
    pub auto_backup_enabled: bool,
    pub auto_backup_interval_days: u32,
    pub auto_backup_max_keep: u32,
}

impl Default for BackupSettingsPayload {
    fn default() -> Self {
        let defaults = crate::settings::types::AppSettings::default();
        Self {
            backup_type: "local".to_string(),
            local_backup_path: String::new(),
            webdav: crate::settings::types::WebDAVConfig::default(),
            backup_encryption_enabled: false,
            encryption_password: None,
            repository: BackupRepositoryConfig::default(),
            repository_token: None,
            backup_image_assets_enabled: defaults.backup_image_assets_enabled,
            backup_cli_config_files_enabled: defaults.backup_cli_config_files_enabled,
            backup_custom_entries: Vec::new(),
            backup_file_filter_rules: defaults.backup_file_filter_rules,
            auto_backup_enabled: false,
            auto_backup_interval_days: 7,
            auto_backup_max_keep: 10,
        }
    }
}

/// Unified backup settings save: patches the backup fields of AppSettings, updates
/// the repository connection record, and stores a newly submitted encryption
/// password in the OS credential store — in one flow, with explicit consistency
/// handling: the credential write happens first; if the database write fails the
/// previous password is restored so the visible state stays usable.
#[tauri::command]
pub async fn save_backup_settings(
    app: tauri::AppHandle,
    payload: BackupSettingsPayload,
) -> Result<BackupSettingsSaveOutcome, String> {
    let db = app.state::<SqliteDbState>().inner().clone();

    if !matches!(payload.backup_type.as_str(), "local" | "webdav" | "repository") {
        return Err(backup_error(
            "invalidBackupType",
            "settings.backupSettings.errors.invalidBackupType",
            &format!("unknown backup_type: {}", payload.backup_type),
        ));
    }

    // Repository draft validation: a blank connection is allowed on any channel
    // (the channel simply is not usable yet and must not block local/WebDAV saves),
    // anything partially filled must be valid.
    let repository_config = normalize(payload.repository.clone());
    if !repository_config.is_blank_connection() {
        validate_config(&repository_config)?;
    }

    // Credential store first: a failure here keeps the old settings untouched.
    // Outer Option tracks whether a password update was attempted; the inner one is
    // the credential that existed before the update (None = nothing was stored).
    let mut previous_password: Option<Option<String>> = None;
    if let Some(password) = payload.encryption_password.as_deref().filter(|p| !p.is_empty()) {
        let password = password.to_string();
        previous_password = Some(
            tauri::async_runtime::spawn_blocking(credentials::read_password)
                .await
                .map_err(|error| error.to_string())??,
        );
        tauri::async_runtime::spawn_blocking(move || credentials::store_password(&password))
            .await
            .map_err(|error| error.to_string())??;
    }

    let settings_patch: Vec<(&str, Value)> = vec![
        ("backup_type", json!(payload.backup_type)),
        ("local_backup_path", json!(payload.local_backup_path)),
        ("webdav", serde_json::to_value(&payload.webdav).map_err(|e| e.to_string())?),
        (
            "backup_encryption",
            serde_json::to_value(&BackupEncryptionConfig {
                enabled: payload.backup_encryption_enabled,
                credential_ref: BackupEncryptionConfig::default().credential_ref,
            })
            .map_err(|e| e.to_string())?,
        ),
        (
            "backup_image_assets_enabled",
            json!(payload.backup_image_assets_enabled),
        ),
        (
            "backup_cli_config_files_enabled",
            json!(payload.backup_cli_config_files_enabled),
        ),
        (
            "backup_custom_entries",
            json!(payload
                .backup_custom_entries
                .iter()
                .map(crate::settings::backup::utils::normalize_backup_custom_entry)
                .collect::<Vec<_>>()),
        ),
        (
            "backup_file_filter_rules",
            json!(payload.backup_file_filter_rules),
        ),
        ("auto_backup_enabled", json!(payload.auto_backup_enabled)),
        (
            "auto_backup_interval_days",
            json!(payload.auto_backup_interval_days),
        ),
        ("auto_backup_max_keep", json!(payload.auto_backup_max_keep)),
    ];

    let db_for_save = db.clone();
    let repository_config_for_save = repository_config.clone();
    let repository_token = payload.repository_token.clone();
    let backup_type_for_save = payload.backup_type.clone();
    let settings_result = tauri::async_runtime::spawn_blocking(move || {
        save_settings_and_repository(
            &db_for_save,
            &settings_patch,
            repository_config_for_save,
            repository_token,
            &backup_type_for_save,
        )
    })
    .await
    .map_err(|error| error.to_string())?;

    if let Err(error) = settings_result {
        // Roll the credential store back so the saved settings and the stored
        // password stay consistent. Both the join failure and the keyring failure
        // itself must surface, otherwise a new password could survive a failed save.
        let rollback = if let Some(previous) = previous_password {
            tauri::async_runtime::spawn_blocking(move || {
                match previous {
                    Some(password) => credentials::store_password(&password),
                    // No stored password before: remove the just-written one.
                    None => credentials::delete_password(),
                }
            })
            .await
            .map_err(|join_error| join_error.to_string())
            .and_then(std::convert::identity)
        } else {
            Ok(())
        };
        return Err(combine_rollback_error(&error, rollback));
    }

    let repository = load_backup_repository_settings(&db)?;
    Ok(BackupSettingsSaveOutcome {
        repository: repository.into(),
        // Infallible w.r.t. the credential store: the settings ARE saved at this
        // point, so a store read failure must report "password state unknown"
        // instead of turning the finished save into a frontend-side error.
        encryption: encryption_status(&db)?,
    })
}

/// Combine the save error with a failed credential rollback. A successful rollback
/// returns the original error untouched; a failed one appends the credentialRollback
/// payload so the user learns the stored password may no longer match the settings.
fn combine_rollback_error(save_error: &str, rollback: Result<(), String>) -> String {
    match rollback {
        Ok(()) => save_error.to_string(),
        Err(rollback_error) => format!(
            "{save_error}; {}",
            backup_error(
                "credentialRollback",
                "settings.backupSettings.encryption.errors.credentialRollback",
                &format!("restoring the previous credential also failed: {rollback_error}"),
            )
        ),
    }
}

fn save_settings_and_repository(
    db: &SqliteDbState,
    settings_patch: &[(&str, Value)],
    repository_config: BackupRepositoryConfig,
    repository_token: Option<String>,
    backup_type: &str,
) -> Result<(), String> {
    db.with_conn_mut(|connection| {
        db_transaction(connection, |tx| {
            // Patch only the fields this form owns; other settings fields are kept.
            // The settings record is created on demand so a fresh install can save
            // backup settings without a prior full save.
            let existing = db_get(tx, DbTable::Settings, "app")?;
            if existing.is_none() {
                db_put(tx, DbTable::Settings, "app", &json!({}))?;
            }
            db_patch_fields(tx, DbTable::Settings, "app", settings_patch)?;

            // Repository connection record with token-keeping semantics. A blank
            // draft only rewrites the record when the user is saving from the
            // repository channel itself: hidden/untouched fields from the other
            // channels must never act as a deletion command for a stored connection.
            let config = repository_config;
            if !config.is_blank_connection() {
                validate_config(&config)?;
            }
            let previous = read_repository_in_tx(tx)?;
            let next = if config.is_blank_connection() {
                if backup_type != "repository" {
                    return Ok(());
                }
                // Deliberate clear from the repository channel: the stored token
                // belongs to a connection that no longer exists, so it is dropped
                // with it instead of surviving as a dangling credential.
                BackupRepositorySettings {
                    config,
                    token: String::new(),
                }
            } else {
                let token = resolve_repository_token(&previous, &config, repository_token)?;
                BackupRepositorySettings { config, token }
            };
            db_put(
                tx,
                DbTable::Settings,
                BACKUP_REPOSITORY_SETTINGS_ID,
                &serde_json::to_value(&next).map_err(|error| error.to_string())?,
            )?;
            Ok(())
        })
    })
}

fn read_repository_in_tx(
    tx: &rusqlite::Transaction<'_>,
) -> Result<BackupRepositorySettings, String> {
    db_get(tx, DbTable::Settings, BACKUP_REPOSITORY_SETTINGS_ID)?
        .map(serde_json::from_value::<BackupRepositorySettings>)
        .transpose()
        .map_err(|error| error.to_string())
        .map(Option::unwrap_or_default)
}

/// Report the encryption switch plus whether this machine currently has a stored
/// password. The password itself never leaves the backend. A credential-store read
/// failure degrades to `password_known: false` — callers use this for display and
/// post-save status only, so it must not fail a finished settings save; backup
/// generation treats a store failure independently and still refuses to continue.
pub fn encryption_status(db: &SqliteDbState) -> Result<BackupEncryptionStatus, String> {
    let settings = store::load_settings_from_sqlite_state(db)?;
    let (has_password, password_known) = match credentials::read_password() {
        Ok(Some(_)) => (true, true),
        Ok(None) => (false, true),
        Err(error) => {
            log::warn!("Backup credential store read failed: {error}");
            (false, false)
        }
    };
    Ok(BackupEncryptionStatus {
        enabled: settings.backup_encryption.enabled,
        has_password,
        password_known,
    })
}

#[tauri::command]
pub async fn get_backup_encryption_status(
    db: tauri::State<'_, SqliteDbState>,
) -> Result<BackupEncryptionStatus, String> {
    // Reading the credential store is a blocking OS call.
    let db = db.inner().clone();
    tauri::async_runtime::spawn_blocking(move || encryption_status(&db))
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_encryption_defaults_to_disabled() {
        let settings = crate::settings::adapter::from_db_value(json!({}));
        assert!(!settings.backup_encryption.enabled);
        assert!(!settings.backup_encryption.credential_ref.is_empty());

        let settings = crate::settings::adapter::from_db_value(json!({
            "backup_encryption": {"enabled": true}
        }));
        assert!(settings.backup_encryption.enabled);
    }

    fn stored_record(db: &SqliteDbState) -> Option<BackupRepositorySettings> {
        db.with_conn(|connection| {
            db_get(connection, DbTable::Settings, BACKUP_REPOSITORY_SETTINGS_ID)?
                .map(serde_json::from_value::<BackupRepositorySettings>)
                .transpose()
                .map_err(|error| error.to_string())
        })
        .expect("read backup repository record")
    }

    fn write_record(db: &SqliteDbState, id: &str, value: &Value) {
        db.with_conn(|connection| db_put(connection, DbTable::Settings, id, value))
            .expect("write settings record");
    }

    fn github_connection() -> BackupRepositoryConfig {
        BackupRepositoryConfig {
            platform: BackupRepositoryPlatform::Github,
            owner: "someone".into(),
            repository: "backups".into(),
            branch: "main".into(),
            directory: "ai-toolbox".into(),
        }
    }

    #[test]
    fn blank_draft_defaults_are_not_a_connection() {
        // A fresh-install form draft carries branch/directory defaults; only a
        // missing owner+repository means "no connection".
        let draft = BackupRepositoryConfig {
            platform: BackupRepositoryPlatform::Github,
            owner: String::new(),
            repository: String::new(),
            branch: "main".into(),
            directory: "ai-toolbox".into(),
        };
        assert!(draft.is_blank_connection());
        assert!(!draft.is_unconfigured(), "branch default must not count as a configured connection");
    }

    #[test]
    fn legacy_repository_sync_connection_migrates_once() {
        let db = SqliteDbState::in_memory_for_test().expect("sqlite state");
        write_record(
            &db,
            LEGACY_REPOSITORY_SYNC_ID,
            &json!({
                "config": {
                    "platform": "gitee",
                    "owner": "old-owner",
                    "repository": "old-repo",
                    "branch": "master",
                    "directory": "legacy-dir",
                    "scopes": ["providers", "prompts"],
                    "include_credentials": false,
                },
                "token": "legacy-token",
                "status": {"last_sync_at": "2026-01-01T00:00:00Z"},
            }),
        );

        let settings = load_backup_repository_settings(&db).expect("migrated load");
        assert_eq!(settings.config.platform, BackupRepositoryPlatform::Gitee);
        assert_eq!(settings.config.owner, "old-owner");
        assert_eq!(settings.config.repository, "old-repo");
        assert_eq!(settings.config.branch, "master");
        assert_eq!(settings.token, "legacy-token");

        // The migration persists: the second load reads the new record directly.
        let persisted = stored_record(&db).expect("migrated record stored");
        assert_eq!(persisted, settings);
        let again = load_backup_repository_settings(&db).expect("second load");
        assert_eq!(again, settings);
    }

    #[test]
    fn existing_backup_record_is_never_overwritten_by_migration() {
        let db = SqliteDbState::in_memory_for_test().expect("sqlite state");
        let new_record = BackupRepositorySettings {
            config: github_connection(),
            token: "new-token".into(),
        };
        write_record(
            &db,
            BACKUP_REPOSITORY_SETTINGS_ID,
            &serde_json::to_value(&new_record).unwrap(),
        );
        write_record(
            &db,
            LEGACY_REPOSITORY_SYNC_ID,
            &json!({
                "config": {"platform": "github", "owner": "legacy", "repository": "legacy"},
                "token": "legacy-token",
            }),
        );

        let settings = load_backup_repository_settings(&db).expect("load");
        assert_eq!(settings, new_record);
    }

    #[test]
    fn blank_draft_from_other_channel_keeps_stored_connection() {
        let db = SqliteDbState::in_memory_for_test().expect("sqlite state");
        let stored = BackupRepositorySettings {
            config: github_connection(),
            token: "stored-token".into(),
        };
        write_record(
            &db,
            BACKUP_REPOSITORY_SETTINGS_ID,
            &serde_json::to_value(&stored).unwrap(),
        );

        let blank = BackupRepositoryConfig {
            platform: BackupRepositoryPlatform::Github,
            owner: String::new(),
            repository: String::new(),
            branch: "main".into(),
            directory: "ai-toolbox".into(),
        };
        save_settings_and_repository(&db, &[], blank, None, "local").expect("save from local");

        assert_eq!(stored_record(&db), Some(stored));
    }

    #[test]
    fn blank_draft_from_repository_channel_clears_connection_and_token() {
        let db = SqliteDbState::in_memory_for_test().expect("sqlite state");
        write_record(
            &db,
            BACKUP_REPOSITORY_SETTINGS_ID,
            &serde_json::to_value(&BackupRepositorySettings {
                config: github_connection(),
                token: "stored-token".into(),
            })
            .unwrap(),
        );

        let blank = BackupRepositoryConfig {
            platform: BackupRepositoryPlatform::Github,
            owner: String::new(),
            repository: String::new(),
            branch: String::new(),
            directory: String::new(),
        };
        save_settings_and_repository(&db, &[], blank, None, "repository")
            .expect("deliberate clear");

        let record = stored_record(&db).expect("record rewritten");
        assert!(record.config.is_blank_connection());
        assert!(record.token.is_empty(), "a cleared connection must drop its token");
    }

    #[test]
    fn save_blocks_when_only_owner_is_missing() {
        let db = SqliteDbState::in_memory_for_test().expect("sqlite state");
        let partial = BackupRepositoryConfig {
            platform: BackupRepositoryPlatform::Github,
            owner: String::new(),
            repository: "backups".into(),
            branch: "main".into(),
            directory: String::new(),
        };
        assert!(save_settings_and_repository(&db, &[], partial, None, "repository").is_err());
    }

    #[test]
    fn resolve_repository_token_rules() {
        let previous = BackupRepositorySettings {
            config: github_connection(),
            token: "stored-token".into(),
        };

        // Draft token wins.
        let draft = github_connection();
        assert_eq!(
            resolve_repository_token(&previous, &draft, Some(" new-token ".into())).unwrap(),
            "new-token"
        );

        // Same platform: an empty draft token reuses the stored one.
        assert_eq!(
            resolve_repository_token(&previous, &draft, None).unwrap(),
            "stored-token"
        );

        // Cross platform without a new token must fail before any network request.
        let mut gitee_draft = github_connection();
        gitee_draft.platform = BackupRepositoryPlatform::Gitee;
        let error = resolve_repository_token(&previous, &gitee_draft, None)
            .expect_err("cross-platform reuse must fail");
        assert!(error.contains("tokenRequired"), "unexpected error: {error}");

        // Cross platform with an explicit new token is fine.
        assert_eq!(
            resolve_repository_token(&previous, &gitee_draft, Some("gitee-token".into())).unwrap(),
            "gitee-token"
        );
    }

    #[test]
    fn combine_rollback_error_layers() {
        assert_eq!(combine_rollback_error("save failed", Ok(())), "save failed");

        let combined = combine_rollback_error("save failed", Err("keyring locked".into()));
        assert!(combined.starts_with("save failed;"));
        assert!(combined.contains("credentialRollback"));
        assert!(combined.contains("keyring locked"));
    }
}
