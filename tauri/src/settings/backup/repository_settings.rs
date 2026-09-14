//! Backup repository connection settings (`settings:backup_repository`).
//!
//! The repository connection lives in its own record so the token never joins the
//! AppSettings payload; the frontend only receives `has_token`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Manager;
use zeroize::Zeroizing;

use super::credentials::{self, backup_error, PasswordStore};
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
    db.with_conn_mut(|connection| db_transaction(connection, read_repository_in_tx))
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
    // Keep blocking OS calls off the runtime and serialize the complete operation,
    // including rollback, with password reads made by background backups.
    tauri::async_runtime::spawn_blocking(move || {
        credentials::with_password_store(|password_store| {
            save_backup_settings_with_store(&db, payload, password_store)
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn save_backup_settings_with_store(
    db: &SqliteDbState,
    mut payload: BackupSettingsPayload,
    password_store: &dyn PasswordStore,
) -> Result<BackupSettingsSaveOutcome, String> {
    if !matches!(
        payload.backup_type.as_str(),
        "local" | "webdav" | "repository"
    ) {
        return Err(backup_error(
            "invalidBackupType",
            "settings.backupSettings.errors.invalidBackupType",
            &format!("unknown backup_type: {}", payload.backup_type),
        ));
    }

    // Only the selected repository form can edit its stored connection. Hidden
    // drafts may be incomplete or stale and must not block another channel's save.
    let repository_config = normalize(payload.repository.clone());
    if payload.backup_type == "repository" && !repository_config.is_blank_connection() {
        validate_config(&repository_config)?;
    }

    // Credential store first: a failure here keeps the old settings untouched.
    // Outer Option tracks whether a password update was attempted; the inner one is
    // the credential that existed before the update (None = nothing was stored).
    let mut previous_password: Option<Option<Zeroizing<String>>> = None;
    let submitted_password = payload.encryption_password.take().map(Zeroizing::new);
    if let Some(password) = submitted_password.as_deref().filter(|p| !p.is_empty()) {
        previous_password = Some(password_store.read_password()?.map(Zeroizing::new));
        password_store.store_password(password)?;
    }

    let settings_patch: Vec<(&str, Value)> = vec![
        ("backup_type", json!(payload.backup_type)),
        ("local_backup_path", json!(payload.local_backup_path)),
        (
            "webdav",
            serde_json::to_value(&payload.webdav).map_err(|e| e.to_string())?,
        ),
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

    // A failed database task must use the same rollback path as a returned error.
    let settings_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        save_settings_and_repository(
            db,
            &settings_patch,
            repository_config,
            payload.repository_token,
            &payload.backup_type,
        )
    }))
    .map_err(|_| "backup settings database task failed".to_string())
    .and_then(std::convert::identity);
    let repository = match settings_result {
        Ok(repository) => repository,
        Err(error) => {
            let rollback = match previous_password {
                Some(Some(password)) => password_store.store_password(&password),
                Some(None) => password_store.delete_password(),
                None => Ok(()),
            };
            return Err(combine_rollback_error(&error, rollback));
        }
    };
    Ok(BackupSettingsSaveOutcome {
        repository: repository.into(),
        // Infallible w.r.t. the credential store: the settings ARE saved at this
        // point, so a store read failure must report "password state unknown"
        // instead of turning the finished save into a frontend-side error.
        encryption: password_status(
            payload.backup_encryption_enabled,
            password_store.read_password(),
        ),
    })
}

/// Combine the save error with a failed credential rollback. A successful rollback
/// returns the original error untouched; a failed one appends the credentialRollback
/// payload so the user learns the stored password may no longer match the settings.
fn combine_rollback_error(save_error: &str, rollback: Result<(), String>) -> String {
    match rollback {
        Ok(()) => save_error.to_string(),
        Err(rollback_error) => backup_error(
            "credentialRollback",
            "settings.backupSettings.encryption.errors.credentialRollback",
            &format!(
                "{save_error}; restoring the previous credential also failed: {rollback_error}"
            ),
        ),
    }
}

fn save_settings_and_repository(
    db: &SqliteDbState,
    settings_patch: &[(&str, Value)],
    repository_config: BackupRepositoryConfig,
    repository_token: Option<String>,
    backup_type: &str,
) -> Result<BackupRepositorySettings, String> {
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

            let previous = read_repository_in_tx(tx)?;
            if backup_type != "repository" {
                return Ok(previous);
            }
            // Only this channel owns repository edits, including an explicit clear.
            let config = repository_config;
            if !config.is_blank_connection() {
                validate_config(&config)?;
            }
            let next = if config.is_blank_connection() {
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
            Ok(next)
        })
    })
}

fn read_repository_in_tx(
    tx: &rusqlite::Transaction<'_>,
) -> Result<BackupRepositorySettings, String> {
    if let Some(value) = db_get(tx, DbTable::Settings, BACKUP_REPOSITORY_SETTINGS_ID)? {
        return serde_json::from_value(value).map_err(|error| error.to_string());
    }
    let Some(legacy) = db_get(tx, DbTable::Settings, LEGACY_REPOSITORY_SYNC_ID)? else {
        return Ok(BackupRepositorySettings::default());
    };
    let migrated = legacy_sync_to_backup_repository(legacy)?;
    db_put(
        tx,
        DbTable::Settings,
        BACKUP_REPOSITORY_SETTINGS_ID,
        &serde_json::to_value(&migrated).map_err(|error| error.to_string())?,
    )?;
    Ok(migrated)
}

/// Report the encryption switch plus whether this machine currently has a stored
/// password. The password itself never leaves the backend. A credential-store read
/// failure degrades to `password_known: false` — callers use this for display and
/// post-save status only, so it must not fail a finished settings save; backup
/// generation treats a store failure independently and still refuses to continue.
pub fn encryption_status(db: &SqliteDbState) -> Result<BackupEncryptionStatus, String> {
    let settings = store::load_settings_from_sqlite_state(db)?;
    Ok(password_status(
        settings.backup_encryption.enabled,
        credentials::read_password(),
    ))
}

fn password_status(
    enabled: bool,
    password_result: Result<Option<String>, String>,
) -> BackupEncryptionStatus {
    let (has_password, password_known) = match password_result {
        Ok(Some(password)) => {
            let _password = Zeroizing::new(password);
            (true, true)
        }
        Ok(None) => (false, true),
        Err(error) => {
            log::warn!("Backup credential store read failed: {error}");
            (false, false)
        }
    };
    BackupEncryptionStatus {
        enabled,
        has_password,
        password_known,
    }
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
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct TestPasswordStore {
        password: RefCell<Option<String>>,
        read_error: bool,
        fail_write: Option<usize>,
        fail_delete: bool,
        writes: Cell<usize>,
    }

    impl PasswordStore for TestPasswordStore {
        fn read_password(&self) -> Result<Option<String>, String> {
            if self.read_error {
                Err("fixture credential store unavailable".into())
            } else {
                Ok(self.password.borrow().clone())
            }
        }

        fn store_password(&self, password: &str) -> Result<(), String> {
            self.writes.set(self.writes.get() + 1);
            if self.fail_write == Some(self.writes.get()) {
                return Err("fixture credential write failed".into());
            }
            self.password.replace(Some(password.into()));
            Ok(())
        }

        fn delete_password(&self) -> Result<(), String> {
            if self.fail_delete {
                Err("fixture credential delete failed".into())
            } else {
                self.password.replace(None);
                Ok(())
            }
        }
    }

    fn stored_app(db: &SqliteDbState) -> Value {
        db.with_conn(|connection| db_get(connection, DbTable::Settings, "app"))
            .unwrap()
            .unwrap()
    }

    fn seed_connection(db: &SqliteDbState) -> BackupRepositorySettings {
        let previous = BackupRepositorySettings {
            config: github_connection(),
            token: "stored-token".into(),
        };
        write_record(
            db,
            BACKUP_REPOSITORY_SETTINGS_ID,
            &serde_json::to_value(&previous).unwrap(),
        );
        write_record(
            db,
            "app",
            &json!({
                "backup_type": "local",
                "keep_computer_awake": true,
                "last_auto_backup_time": "2026-09-14T00:00:00Z",
            }),
        );
        previous
    }

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
        assert!(
            !draft.is_unconfigured(),
            "branch default must not count as a configured connection"
        );
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
        assert!(
            record.token.is_empty(),
            "a cleared connection must drop its token"
        );
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
        let parsed: Value = serde_json::from_str(&combined).expect("one frontend-readable error");
        assert_eq!(parsed["type"], "credentialRollback");
        assert!(parsed["message"].as_str().unwrap().contains("save failed"));
        assert!(parsed["message"]
            .as_str()
            .unwrap()
            .contains("keyring locked"));
    }

    #[test]
    fn save_inactive_repository_drafts_preserves_connection_and_unrelated_settings() {
        for channel in ["local", "webdav"] {
            let db = SqliteDbState::in_memory_for_test().unwrap();
            let previous = seed_connection(&db);
            let passwords = TestPasswordStore::default();
            let payload = BackupSettingsPayload {
                backup_type: channel.into(),
                repository: BackupRepositoryConfig {
                    platform: BackupRepositoryPlatform::Gitee,
                    owner: String::new(),
                    repository: "unfinished-draft".into(),
                    ..Default::default()
                },
                repository_token: Some("uncommitted-token".into()),
                auto_backup_enabled: true,
                auto_backup_interval_days: 2,
                auto_backup_max_keep: 3,
                ..Default::default()
            };
            let outcome = save_backup_settings_with_store(&db, payload, &passwords).unwrap();
            assert_eq!(outcome.repository.config, previous.config);
            assert_eq!(stored_record(&db), Some(previous));
            let settings = store::load_settings_from_sqlite_state(&db).unwrap();
            assert_eq!(settings.backup_type, channel);
            assert!(settings.auto_backup_enabled);
            assert_eq!(settings.auto_backup_interval_days, 2);
            assert_eq!(settings.auto_backup_max_keep, 3);
            assert!(settings.keep_computer_awake);
            assert_eq!(
                settings.last_auto_backup_time.as_deref(),
                Some("2026-09-14T00:00:00Z")
            );
            assert_eq!(passwords.writes.get(), 0);
        }
    }

    #[test]
    fn plain_settings_save_succeeds_when_password_status_is_unavailable() {
        let db = SqliteDbState::in_memory_for_test().unwrap();
        let passwords = TestPasswordStore {
            read_error: true,
            ..Default::default()
        };
        let payload: BackupSettingsPayload = serde_json::from_value(json!({
            "backup_type": "local",
            "local_backup_path": "fixture-backups",
        }))
        .unwrap();
        let outcome = save_backup_settings_with_store(&db, payload, &passwords).unwrap();
        assert!(!outcome.encryption.enabled);
        assert!(!outcome.encryption.password_known);
        assert_eq!(stored_app(&db)["local_backup_path"], "fixture-backups");
        assert_eq!(passwords.writes.get(), 0);
    }

    #[test]
    fn repository_save_round_trips_root_directory_token_and_private_password_status() {
        let db = SqliteDbState::in_memory_for_test().unwrap();
        let passwords = TestPasswordStore::default();
        let mut connection = github_connection();
        connection.directory.clear();
        let payload = BackupSettingsPayload {
            backup_type: "repository".into(),
            repository: connection.clone(),
            repository_token: Some("fixture-token".into()),
            backup_encryption_enabled: true,
            encryption_password: Some("fixture-password".into()),
            ..Default::default()
        };
        let outcome = save_backup_settings_with_store(&db, payload, &passwords).unwrap();
        assert_eq!(outcome.repository.config, connection);
        assert!(outcome.repository.has_token);
        assert!(outcome.encryption.enabled && outcome.encryption.has_password);
        assert!(outcome.encryption.password_known);
        let returned = serde_json::to_string(&outcome).unwrap();
        assert!(!returned.contains("fixture-password"));
        assert!(!returned.contains("fixture-token"));
        assert!(!stored_app(&db).to_string().contains("fixture-password"));
        assert_eq!(
            passwords.read_password().unwrap().as_deref(),
            Some("fixture-password")
        );

        let payload = BackupSettingsPayload {
            backup_type: "repository".into(),
            repository: connection,
            ..Default::default()
        };
        save_backup_settings_with_store(&db, payload, &passwords).unwrap();
        assert_eq!(stored_record(&db).unwrap().token, "fixture-token");
        assert_eq!(passwords.writes.get(), 1);
    }

    #[test]
    fn failed_save_rolls_back_database_and_password_and_reports_rollback_failures() {
        for (previous_password, fail_write, fail_delete) in [
            (Some("old-password"), None, false),
            (None, None, false),
            (Some("old-password"), Some(2), false),
            (None, None, true),
        ] {
            let db = SqliteDbState::in_memory_for_test().unwrap();
            let previous = seed_connection(&db);
            let previous_app = stored_app(&db);
            let passwords = TestPasswordStore {
                password: RefCell::new(previous_password.map(str::to_string)),
                fail_write,
                fail_delete,
                ..Default::default()
            };
            let mut draft = github_connection();
            draft.platform = BackupRepositoryPlatform::Gitee;
            let payload = BackupSettingsPayload {
                backup_type: "repository".into(),
                repository: draft,
                encryption_password: Some("new-password".into()),
                backup_encryption_enabled: true,
                ..Default::default()
            };
            let error = save_backup_settings_with_store(&db, payload, &passwords)
                .err()
                .expect("a platform switch without a token fails");
            assert_eq!(stored_app(&db), previous_app);
            assert_eq!(stored_record(&db), Some(previous));
            let parsed: Value = serde_json::from_str(&error).unwrap();
            if fail_write.is_some() || fail_delete {
                assert_eq!(parsed["type"], "credentialRollback");
                assert!(parsed["message"]
                    .as_str()
                    .unwrap()
                    .contains("tokenRequired"));
            } else {
                assert_eq!(parsed["type"], "tokenRequired");
                assert_eq!(
                    passwords.read_password().unwrap().as_deref(),
                    previous_password
                );
            }
            assert!(!error.contains("old-password"));
            assert!(!error.contains("new-password"));
        }
    }

    #[test]
    fn credential_write_failure_keeps_database_and_previous_password() {
        let db = SqliteDbState::in_memory_for_test().unwrap();
        seed_connection(&db);
        let previous_app = stored_app(&db);
        let passwords = TestPasswordStore {
            password: RefCell::new(Some("old-password".into())),
            fail_write: Some(1),
            ..Default::default()
        };
        let payload = BackupSettingsPayload {
            encryption_password: Some("new-password".into()),
            ..Default::default()
        };
        assert!(save_backup_settings_with_store(&db, payload, &passwords).is_err());
        assert_eq!(stored_app(&db), previous_app);
        assert_eq!(
            passwords.read_password().unwrap().as_deref(),
            Some("old-password")
        );
    }

    #[test]
    fn saving_before_loading_migrates_legacy_connection_and_preserves_its_token() {
        let db = SqliteDbState::in_memory_for_test().unwrap();
        write_record(
            &db,
            LEGACY_REPOSITORY_SYNC_ID,
            &json!({
                "config": github_connection(),
                "token": "legacy-token",
                "scopes": ["providers"],
            }),
        );
        let passwords = TestPasswordStore::default();
        let payload = BackupSettingsPayload {
            backup_type: "repository".into(),
            repository: github_connection(),
            ..Default::default()
        };
        save_backup_settings_with_store(&db, payload, &passwords).unwrap();
        let record = stored_record(&db).unwrap();
        assert_eq!(record.token, "legacy-token");
        let value = serde_json::to_value(record).unwrap();
        assert!(value.get("scopes").is_none());
    }
}
