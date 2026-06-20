use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{TimeZone, Utc};
use regex::Regex;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::official_accounts::auth_has_official_runtime;
use uuid::Uuid;

const STATE_DB_FILE_NAME: &str = "state_5.sqlite";
const CONFIG_FILE_NAME: &str = "config.toml";
const AUTH_FILE_NAME: &str = "auth.json";
const OFFICIAL_MODEL_PROVIDER_ID: &str = "openai";
const SESSION_INDEX_FILE_NAME: &str = "session_index.jsonl";
const SESSIONS_DIR_NAME: &str = "sessions";
const BACKUP_DIR_NAME: &str = "history_sync_backups";
const WRITE_LOCK_RETRY_LIMIT: usize = 40;
const WRITE_LOCK_RETRY_DELAY: Duration = Duration::from_millis(250);
const FILE_RETRY_LIMIT: usize = 20;
const FILE_RETRY_DELAY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistorySyncStatus {
    pub codex_home: String,
    pub config_path: String,
    pub db_path: String,
    pub sessions_dir: String,
    pub session_index_path: String,
    pub backup_dir: String,
    pub current_provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_model: Option<String>,
    pub total_threads: usize,
    pub provider_mismatch_threads: usize,
    pub model_mismatch_threads: usize,
    pub model_column_exists: bool,
    pub session_file_count: usize,
    pub session_meta_mismatch_count: usize,
    pub indexed_threads: usize,
    pub missing_session_index_entries: usize,
    pub backup_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_backup_path: Option<String>,
    pub has_work: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistoryBackupResult {
    pub backup_path: String,
    pub backup_dir: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistorySyncResult {
    pub status: CodexHistorySyncStatus,
    pub backup_path: String,
    pub updated_thread_rows: usize,
    pub updated_session_files: usize,
    pub failed_session_files: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_session_file_error: Option<String>,
    pub rewritten_index_entries: usize,
    pub missing_session_index_entries_before: usize,
    pub preserved_index_only_entries: usize,
    pub attempts: usize,
    pub lock_wait_ms: u128,
    pub duration_ms: u128,
    pub partial_success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistoryRestoreResult {
    pub restored_backup_path: String,
    pub safety_backup_path: String,
    pub restored_session_meta_files: usize,
    pub skipped_session_meta_files: usize,
    pub rewritten_index_entries: usize,
    pub attempts: usize,
    pub lock_wait_ms: u128,
    pub duration_ms: u128,
    pub status: CodexHistorySyncStatus,
}

#[derive(Debug, Clone)]
struct Paths {
    codex_home: PathBuf,
    config_path: PathBuf,
    db_path: PathBuf,
    sessions_dir: PathBuf,
    session_index_path: PathBuf,
    auth_path: PathBuf,
    backup_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct CurrentTarget {
    provider: String,
    model: Option<String>,
}

#[derive(Debug, Clone)]
struct ThreadRow {
    id: String,
    title: Option<String>,
    updated_at: Option<i64>,
}

#[derive(Debug, Clone)]
struct SessionRecord {
    path: PathBuf,
    relative_path: String,
    model_provider: Option<String>,
    first_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionIndexEntry {
    id: String,
    thread_name: String,
    #[serde(default)]
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionMetaBackupEntry {
    relative_path: String,
    first_line: String,
}

#[derive(Debug, Clone)]
struct BackupOutcome {
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct RetryStats<T> {
    value: T,
    attempts: usize,
    lock_wait_ms: u128,
}

#[derive(Debug, Clone)]
struct IndexRebuildResult {
    rewritten_entries: usize,
    missing_entries_before: usize,
    preserved_index_only_entries: usize,
}

pub fn get_status(codex_home: &Path) -> Result<CodexHistorySyncStatus, String> {
    let paths = resolve_paths(codex_home);
    let target = read_current_target(&paths)?;
    let columns = read_thread_columns(&paths.db_path)?;
    let thread_rows = read_visible_thread_rows(&paths.db_path, &columns)?;
    let total_threads = read_total_thread_count(&paths.db_path)?;
    let provider_mismatch_threads =
        count_provider_mismatch_threads(&paths.db_path, &target.provider)?;
    let model_column_exists = columns.contains("model");
    // History sync intentionally preserves existing model values. Keep the field for
    // API compatibility, but treat model differences as informational only.
    let model_mismatch_threads = 0;
    let session_records = scan_session_records(&paths)?;
    let session_meta_mismatch_count = session_records
        .iter()
        .filter(|record| session_record_needs_sync(record, &target))
        .count();
    let session_index_entries = read_session_index(&paths.session_index_path)?;
    let visible_thread_ids: BTreeSet<String> =
        thread_rows.iter().map(|row| row.id.clone()).collect();
    let missing_session_index_entries = visible_thread_ids
        .iter()
        .filter(|id| !session_index_entries.contains_key(*id))
        .count();
    let backups = list_backup_files(&paths.backup_dir)?;
    let latest_backup_path = backups
        .last()
        .map(|path| path.to_string_lossy().to_string());
    let has_work = provider_mismatch_threads > 0
        || session_meta_mismatch_count > 0
        || missing_session_index_entries > 0;

    Ok(CodexHistorySyncStatus {
        codex_home: paths.codex_home.to_string_lossy().to_string(),
        config_path: paths.config_path.to_string_lossy().to_string(),
        db_path: paths.db_path.to_string_lossy().to_string(),
        sessions_dir: paths.sessions_dir.to_string_lossy().to_string(),
        session_index_path: paths.session_index_path.to_string_lossy().to_string(),
        backup_dir: paths.backup_dir.to_string_lossy().to_string(),
        current_provider: target.provider,
        current_model: target.model,
        total_threads,
        provider_mismatch_threads,
        model_mismatch_threads,
        model_column_exists,
        session_file_count: session_records.len(),
        session_meta_mismatch_count,
        indexed_threads: session_index_entries.len(),
        missing_session_index_entries,
        backup_count: backups.len(),
        latest_backup_path,
        has_work,
    })
}

pub fn backup(codex_home: &Path, label: &str) -> Result<CodexHistoryBackupResult, String> {
    let start = Instant::now();
    let paths = resolve_paths(codex_home);
    ensure_environment(&paths)?;
    let outcome = make_backup(&paths, label)?;
    Ok(CodexHistoryBackupResult {
        backup_path: outcome.path.to_string_lossy().to_string(),
        backup_dir: paths.backup_dir.to_string_lossy().to_string(),
        duration_ms: start.elapsed().as_millis(),
    })
}

pub fn sync(codex_home: &Path) -> Result<CodexHistorySyncResult, String> {
    let start = Instant::now();
    let paths = resolve_paths(codex_home);
    let target = read_current_target(&paths)?;
    ensure_environment(&paths)?;
    let backup = make_backup(&paths, "pre-sync")?;

    let db_sync = retry_sqlite_write(|| sync_database(&paths.db_path, &target))?;
    let session_sync = sync_session_records(&paths, &target)?;
    let index_result = rebuild_session_index(&paths)?;
    let status = get_status(codex_home)?;

    Ok(CodexHistorySyncResult {
        status,
        backup_path: backup.path.to_string_lossy().to_string(),
        updated_thread_rows: db_sync.value,
        updated_session_files: session_sync.updated,
        failed_session_files: session_sync.failed,
        first_session_file_error: session_sync.first_error,
        rewritten_index_entries: index_result.rewritten_entries,
        missing_session_index_entries_before: index_result.missing_entries_before,
        preserved_index_only_entries: index_result.preserved_index_only_entries,
        attempts: db_sync.attempts,
        lock_wait_ms: db_sync.lock_wait_ms,
        duration_ms: start.elapsed().as_millis(),
        partial_success: session_sync.failed > 0,
    })
}

pub fn restore_latest(codex_home: &Path) -> Result<CodexHistoryRestoreResult, String> {
    let start = Instant::now();
    let paths = resolve_paths(codex_home);
    ensure_environment(&paths)?;
    let latest_backup = list_backup_files(&paths.backup_dir)?
        .last()
        .cloned()
        .ok_or_else(|| "No Codex history backup found".to_string())?;
    let safety_backup = make_backup(&paths, "pre-restore")?;
    let restore_db =
        retry_sqlite_write(|| restore_database_from_backup(&paths.db_path, &latest_backup))?;
    let (restored_session_meta_files, skipped_session_meta_files) =
        restore_metadata_sidecars(&paths, &latest_backup)?;
    let index_result = rebuild_session_index(&paths)?;
    let status = get_status(codex_home)?;

    Ok(CodexHistoryRestoreResult {
        restored_backup_path: latest_backup.to_string_lossy().to_string(),
        safety_backup_path: safety_backup.path.to_string_lossy().to_string(),
        restored_session_meta_files,
        skipped_session_meta_files,
        rewritten_index_entries: index_result.rewritten_entries,
        attempts: restore_db.attempts,
        lock_wait_ms: restore_db.lock_wait_ms,
        duration_ms: start.elapsed().as_millis(),
        status,
    })
}

fn resolve_paths(codex_home: &Path) -> Paths {
    let codex_home = codex_home.to_path_buf();
    Paths {
        config_path: codex_home.join(CONFIG_FILE_NAME),
        db_path: codex_home.join(STATE_DB_FILE_NAME),
        sessions_dir: codex_home.join(SESSIONS_DIR_NAME),
        session_index_path: codex_home.join(SESSION_INDEX_FILE_NAME),
        auth_path: codex_home.join(AUTH_FILE_NAME),
        backup_dir: codex_home.join(BACKUP_DIR_NAME),
        codex_home,
    }
}

fn ensure_environment(paths: &Paths) -> Result<(), String> {
    if !paths.config_path.exists() {
        return Err(format!(
            "Codex config.toml does not exist: {}",
            paths.config_path.display()
        ));
    }
    if !paths.db_path.exists() {
        return Err(format!(
            "Codex history database does not exist: {}",
            paths.db_path.display()
        ));
    }
    Ok(())
}

fn read_current_target(paths: &Paths) -> Result<CurrentTarget, String> {
    ensure_environment(paths)?;
    let config_text = fs::read_to_string(&paths.config_path)
        .map_err(|error| format!("Failed to read Codex config.toml: {error}"))?;
    let value = config_text
        .parse::<toml::Value>()
        .map_err(|error| format!("Failed to parse Codex config.toml: {error}"))?;
    let provider = match value
        .get("model_provider")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
    {
        Some(provider) => provider,
        None => read_official_provider_fallback(paths, &value)?
            .ok_or_else(|| "Could not find model_provider in Codex config.toml".to_string())?,
    };
    let model = value
        .get("model")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    Ok(CurrentTarget { provider, model })
}

fn read_official_provider_fallback(
    paths: &Paths,
    config: &toml::Value,
) -> Result<Option<String>, String> {
    if !paths.auth_path.exists() {
        return Ok(None);
    }
    let auth_text = fs::read_to_string(&paths.auth_path)
        .map_err(|error| format!("Failed to read Codex auth.json: {error}"))?;
    let auth: Value = serde_json::from_str(&auth_text)
        .map_err(|error| format!("Failed to parse Codex auth.json: {error}"))?;
    if auth_has_official_runtime(&auth)
        && !auth_has_custom_api_key(&auth)
        && !config_has_custom_base_url(config)
    {
        Ok(Some(OFFICIAL_MODEL_PROVIDER_ID.to_string()))
    } else {
        Ok(None)
    }
}

fn auth_has_custom_api_key(auth: &Value) -> bool {
    auth.get("OPENAI_API_KEY")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn config_has_custom_base_url(config: &toml::Value) -> bool {
    toml_string_value(config.get("base_url")).is_some()
        || config
            .get("model_providers")
            .and_then(toml::Value::as_table)
            .is_some_and(|providers| {
                providers.values().any(|provider| {
                    provider
                        .as_table()
                        .and_then(|table| toml_string_value(table.get("base_url")))
                        .is_some()
                })
            })
}

fn toml_string_value(value: Option<&toml::Value>) -> Option<&str> {
    value
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn open_read_connection(db_path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("Failed to open Codex history database: {error}"))
}

fn open_write_connection(db_path: &Path) -> Result<Connection, String> {
    Connection::open(db_path)
        .map_err(|error| format!("Failed to open Codex history database: {error}"))
}

fn read_thread_columns(db_path: &Path) -> Result<BTreeSet<String>, String> {
    let conn = open_read_connection(db_path)?;
    let mut statement = conn
        .prepare("PRAGMA table_info(threads)")
        .map_err(|error| format!("Failed to inspect Codex threads table: {error}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("Failed to read Codex threads columns: {error}"))?;
    let mut columns = BTreeSet::new();
    for row in rows {
        columns.insert(row.map_err(|error| format!("Failed to read Codex column: {error}"))?);
    }
    if columns.is_empty() {
        return Err("Codex history database missing threads table".to_string());
    }
    Ok(columns)
}

fn read_total_thread_count(db_path: &Path) -> Result<usize, String> {
    let conn = open_read_connection(db_path)?;
    conn.query_row("SELECT COUNT(*) FROM threads", [], |row| {
        row.get::<_, i64>(0)
    })
    .map(|value| value.max(0) as usize)
    .map_err(|error| format!("Failed to count Codex threads: {error}"))
}

fn count_provider_mismatch_threads(
    db_path: &Path,
    current_provider: &str,
) -> Result<usize, String> {
    let conn = open_read_connection(db_path)?;
    conn.query_row(
        "SELECT COUNT(*) FROM threads WHERE model_provider IS NULL OR model_provider <> ?1",
        [current_provider],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value.max(0) as usize)
    .map_err(|error| format!("Failed to count Codex provider mismatches: {error}"))
}

fn read_visible_thread_rows(
    db_path: &Path,
    columns: &BTreeSet<String>,
) -> Result<Vec<ThreadRow>, String> {
    let conn = open_read_connection(db_path)?;
    let title_expr = if columns.contains("title") {
        "title"
    } else {
        "NULL"
    };
    let updated_expr = if columns.contains("updated_at") {
        "updated_at"
    } else {
        "NULL"
    };
    let archived_filter = if columns.contains("archived") {
        " WHERE archived IS NULL OR archived = 0"
    } else {
        ""
    };
    let sql = format!("SELECT id, {title_expr}, {updated_expr} FROM threads{archived_filter}");
    let mut statement = conn
        .prepare(&sql)
        .map_err(|error| format!("Failed to read Codex threads: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(ThreadRow {
                id: row.get::<_, String>(0)?,
                title: row.get::<_, Option<String>>(1)?,
                updated_at: read_optional_i64(row, 2)?,
            })
        })
        .map_err(|error| format!("Failed to query Codex threads: {error}"))?;
    let mut threads = Vec::new();
    for row in rows {
        let thread = row.map_err(|error| format!("Failed to parse Codex thread: {error}"))?;
        if !thread.id.trim().is_empty() {
            threads.push(thread);
        }
    }
    Ok(threads)
}

fn read_optional_i64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<i64>> {
    match row.get_ref(index)? {
        rusqlite::types::ValueRef::Null => Ok(None),
        rusqlite::types::ValueRef::Integer(value) => Ok(Some(value)),
        rusqlite::types::ValueRef::Real(value) => Ok(Some(value as i64)),
        rusqlite::types::ValueRef::Text(value) => Ok(std::str::from_utf8(value)
            .ok()
            .and_then(|text| text.parse::<i64>().ok())),
        rusqlite::types::ValueRef::Blob(_) => Ok(None),
    }
}

fn sync_database(db_path: &Path, target: &CurrentTarget) -> Result<usize, String> {
    let mut conn = open_write_connection(db_path)?;
    conn.busy_timeout(Duration::from_millis(500))
        .map_err(|error| format!("Failed to set Codex database busy timeout: {error}"))?;
    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to begin Codex history sync transaction: {error}"))?;
    let updated = transaction
        .execute(
            "UPDATE threads SET model_provider = ?1 WHERE model_provider IS NULL OR model_provider <> ?1",
            [target.provider.as_str()],
        )
        .map_err(|error| format!("Failed to sync Codex history database: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit Codex history sync: {error}"))?;
    let _ = open_write_connection(db_path)?.pragma_update(None, "wal_checkpoint", "PASSIVE");
    Ok(updated)
}

fn retry_sqlite_write<T>(
    mut operation: impl FnMut() -> Result<T, String>,
) -> Result<RetryStats<T>, String> {
    let mut attempts = 0;
    let start = Instant::now();
    loop {
        attempts += 1;
        match operation() {
            Ok(value) => {
                return Ok(RetryStats {
                    value,
                    attempts,
                    lock_wait_ms: start.elapsed().as_millis(),
                });
            }
            Err(error) if is_sqlite_locked_error(&error) && attempts < WRITE_LOCK_RETRY_LIMIT => {
                thread::sleep(WRITE_LOCK_RETRY_DELAY);
            }
            Err(error) if is_sqlite_locked_error(&error) => {
                return Err(format!(
                    "Codex is writing local history. Wait for the current response or autosave to finish, then try again. Last error: {error}"
                ));
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_sqlite_locked_error(error: &str) -> bool {
    let value = error.to_ascii_lowercase();
    value.contains("database is locked")
        || value.contains("database table is locked")
        || value.contains("database is busy")
        || value.contains("destination database is in use")
        || value.contains("locked")
}

fn scan_session_records(paths: &Paths) -> Result<Vec<SessionRecord>, String> {
    let mut files = Vec::new();
    collect_rollout_jsonl_files(&paths.sessions_dir, &mut files)?;
    let mut records = Vec::new();
    for path in files {
        if let Some(record) = read_session_record(paths, &path)? {
            records.push(record);
        }
    }
    Ok(records)
}

fn collect_rollout_jsonl_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(dir).map_err(|error| {
        format!(
            "Failed to read Codex sessions dir {}: {error}",
            dir.display()
        )
    })?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Failed to read Codex session entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rollout_jsonl_files(&path, files)?;
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if file_name.starts_with("rollout-") && file_name.ends_with(".jsonl") {
            files.push(path);
        }
    }
    files.sort();
    Ok(())
}

fn read_session_record(paths: &Paths, path: &Path) -> Result<Option<SessionRecord>, String> {
    let file = fs::File::open(path).map_err(|error| {
        format!(
            "Failed to open Codex session file {}: {error}",
            path.display()
        )
    })?;
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    let read = reader.read_line(&mut first_line).map_err(|error| {
        format!(
            "Failed to read Codex session file {}: {error}",
            path.display()
        )
    })?;
    if read == 0 {
        return Ok(None);
    }
    let normalized_first_line = first_line.trim_end_matches(['\r', '\n']).to_string();
    let value: Value = match serde_json::from_str(&normalized_first_line) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    if value.get("type").and_then(Value::as_str) != Some("session_meta") {
        return Ok(None);
    }
    let Some(payload) = value.get("payload").and_then(Value::as_object) else {
        return Ok(None);
    };
    if payload
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        return Ok(None);
    }
    Ok(Some(SessionRecord {
        relative_path: relative_path(paths, path)?,
        path: path.to_path_buf(),
        model_provider: payload
            .get("model_provider")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        first_line: normalized_first_line,
    }))
}

fn relative_path(paths: &Paths, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(&paths.codex_home).map_err(|_| {
        format!(
            "Codex session path {} is outside Codex home {}",
            path.display(),
            paths.codex_home.display()
        )
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn session_record_needs_sync(record: &SessionRecord, target: &CurrentTarget) -> bool {
    record.model_provider.as_deref() != Some(target.provider.as_str())
}

struct SessionSyncResult {
    updated: usize,
    failed: usize,
    first_error: Option<String>,
}

fn sync_session_records(
    paths: &Paths,
    target: &CurrentTarget,
) -> Result<SessionSyncResult, String> {
    let records = scan_session_records(paths)?;
    let mut updated = 0;
    let mut failed = 0;
    let mut first_error = None;
    for record in records {
        if !session_record_needs_sync(&record, target) {
            continue;
        }
        match rewrite_session_meta_first_line(&record, target) {
            Ok(true) => updated += 1,
            Ok(false) => {}
            Err(error) => {
                failed += 1;
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    Ok(SessionSyncResult {
        updated,
        failed,
        first_error,
    })
}

fn rewrite_session_meta_first_line(
    record: &SessionRecord,
    target: &CurrentTarget,
) -> Result<bool, String> {
    let text = fs::read_to_string(&record.path).map_err(|error| {
        format!(
            "Failed to read Codex session file {}: {error}",
            record.path.display()
        )
    })?;
    let (first_line, line_ending, remainder) = split_first_line(&text);
    let mut value: Value = serde_json::from_str(first_line).map_err(|error| {
        format!(
            "Failed to parse Codex session metadata {}: {error}",
            record.path.display()
        )
    })?;
    let payload = value
        .get_mut("payload")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            format!(
                "Codex session metadata missing payload: {}",
                record.path.display()
            )
        })?;
    payload.insert(
        "model_provider".to_string(),
        Value::String(target.provider.clone()),
    );
    let new_first_line = serde_json::to_string(&value).map_err(|error| {
        format!(
            "Failed to serialize Codex session metadata {}: {error}",
            record.path.display()
        )
    })?;
    if new_first_line == first_line {
        return Ok(false);
    }
    let new_text = format!("{new_first_line}{line_ending}{remainder}");
    write_atomic_with_retry(&record.path, new_text.as_bytes())?;
    Ok(true)
}

fn split_first_line(text: &str) -> (&str, &str, &str) {
    if let Some(index) = text.find('\n') {
        let line_end = if index > 0 && text.as_bytes()[index - 1] == b'\r' {
            index - 1
        } else {
            index
        };
        let ending = if line_end == index { "\n" } else { "\r\n" };
        (&text[..line_end], ending, &text[index + 1..])
    } else {
        (text, "", "")
    }
}

fn write_atomic_with_retry(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Failed to determine parent for {}", path.display()))?;
    let tmp_path = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("codex-history-sync"),
        Uuid::new_v4().simple()
    ));
    let mut last_error = None;
    for attempt in 0..FILE_RETRY_LIMIT {
        let result = fs::write(&tmp_path, bytes).and_then(|_| fs::rename(&tmp_path, path));
        match result {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error.to_string());
                let _ = fs::remove_file(&tmp_path);
                if attempt + 1 < FILE_RETRY_LIMIT {
                    thread::sleep(FILE_RETRY_DELAY);
                }
            }
        }
    }
    Err(format!(
        "Failed to replace Codex file {}: {}",
        path.display(),
        last_error.unwrap_or_else(|| "unknown error".to_string())
    ))
}

fn read_session_index(path: &Path) -> Result<BTreeMap<String, SessionIndexEntry>, String> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let data = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read Codex session index: {error}"))?;
    let mut entries = BTreeMap::new();
    for line in data.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(trimmed) else {
            continue;
        };
        if entry.id.trim().is_empty() {
            continue;
        }
        entries.insert(entry.id.clone(), entry);
    }
    Ok(entries)
}

fn write_session_index(path: &Path, entries: &[SessionIndexEntry]) -> Result<(), String> {
    let mut content = String::new();
    for entry in entries {
        let serialized = serde_json::to_string(entry)
            .map_err(|error| format!("Failed to serialize Codex session index: {error}"))?;
        content.push_str(&serialized);
        content.push('\n');
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create Codex session index directory {}: {error}",
                parent.display()
            )
        })?;
    }
    write_atomic_with_retry(path, content.as_bytes())
}

fn rebuild_session_index(paths: &Paths) -> Result<IndexRebuildResult, String> {
    let columns = read_thread_columns(&paths.db_path)?;
    let thread_rows = read_visible_thread_rows(&paths.db_path, &columns)?;
    let existing = read_session_index(&paths.session_index_path)?;
    let visible_thread_ids: BTreeSet<String> =
        thread_rows.iter().map(|row| row.id.clone()).collect();
    let missing_entries_before = visible_thread_ids
        .iter()
        .filter(|id| !existing.contains_key(*id))
        .count();
    let mut entries = Vec::new();
    for thread in &thread_rows {
        let existing_entry = existing.get(&thread.id);
        let thread_name = existing_entry
            .map(|entry| entry.thread_name.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| thread.title.as_ref().map(|value| value.trim().to_string()))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| thread.id.clone());
        let updated_at = existing_entry
            .map(|entry| entry.updated_at.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| thread.updated_at.and_then(format_unix_seconds))
            .unwrap_or_default();
        entries.push(SessionIndexEntry {
            id: thread.id.clone(),
            thread_name,
            updated_at,
        });
    }
    let mut preserved_index_only_entries = 0;
    for (id, entry) in &existing {
        if visible_thread_ids.contains(id) {
            continue;
        }
        preserved_index_only_entries += 1;
        entries.push(entry.clone());
    }
    entries.sort_by(|left, right| {
        left.updated_at
            .cmp(&right.updated_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    write_session_index(&paths.session_index_path, &entries)?;
    Ok(IndexRebuildResult {
        rewritten_entries: entries.len(),
        missing_entries_before,
        preserved_index_only_entries,
    })
}

pub(crate) fn rebuild_session_index_for_db(
    codex_home: &Path,
    db_path: &Path,
) -> Result<usize, String> {
    if !db_path.exists() {
        return Ok(0);
    }

    let mut paths = resolve_paths(codex_home);
    paths.db_path = db_path.to_path_buf();
    rebuild_session_index(&paths).map(|result| result.rewritten_entries)
}

fn format_unix_seconds(value: i64) -> Option<String> {
    Utc.timestamp_opt(value, 0)
        .single()
        .map(|value| value.to_rfc3339())
}

fn make_backup(paths: &Paths, label: &str) -> Result<BackupOutcome, String> {
    ensure_environment(paths)?;
    fs::create_dir_all(&paths.backup_dir).map_err(|error| {
        format!(
            "Failed to create Codex history backup directory {}: {error}",
            paths.backup_dir.display()
        )
    })?;
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let backup_path = paths.backup_dir.join(format!(
        "state_5.sqlite.{label}.{timestamp}.{}.bak",
        Uuid::new_v4().simple()
    ));
    backup_database_to_path(&paths.db_path, &backup_path)?;
    snapshot_metadata(paths, &backup_path)?;
    Ok(BackupOutcome { path: backup_path })
}

fn backup_database_to_path(db_path: &Path, backup_path: &Path) -> Result<(), String> {
    let conn = open_read_connection(db_path)?;
    conn.backup(rusqlite::MAIN_DB, backup_path, None)
        .map_err(|error| format!("Failed to backup Codex history database: {error}"))
}

fn snapshot_metadata(paths: &Paths, backup_path: &Path) -> Result<(), String> {
    if paths.session_index_path.exists() {
        let index_text = fs::read_to_string(&paths.session_index_path)
            .map_err(|error| format!("Failed to read Codex session index for backup: {error}"))?;
        fs::write(session_index_backup_path(backup_path), index_text)
            .map_err(|error| format!("Failed to write Codex session index backup: {error}"))?;
    }
    let records = scan_session_records(paths)?;
    let entries: Vec<SessionMetaBackupEntry> = records
        .into_iter()
        .map(|record| SessionMetaBackupEntry {
            relative_path: record.relative_path,
            first_line: record.first_line,
        })
        .collect();
    let content = serde_json::to_string_pretty(&entries)
        .map_err(|error| format!("Failed to serialize Codex session metadata backup: {error}"))?;
    fs::write(session_meta_backup_path(backup_path), content)
        .map_err(|error| format!("Failed to write Codex session metadata backup: {error}"))?;
    Ok(())
}

fn session_index_backup_path(backup_path: &Path) -> PathBuf {
    backup_path.with_file_name(format!(
        "{}.session_index.jsonl",
        backup_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ))
}

fn session_meta_backup_path(backup_path: &Path) -> PathBuf {
    backup_path.with_file_name(format!(
        "{}.session_meta.json",
        backup_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ))
}

fn restore_database_from_backup(db_path: &Path, backup_path: &Path) -> Result<(), String> {
    if !backup_path.exists() {
        return Err(format!(
            "Codex history backup does not exist: {}",
            backup_path.display()
        ));
    }
    let source = open_read_connection(backup_path)?;
    source
        .backup(rusqlite::MAIN_DB, db_path, None)
        .map_err(|error| format!("Failed to restore Codex history database: {error}"))
}

fn restore_metadata_sidecars(paths: &Paths, backup_path: &Path) -> Result<(usize, usize), String> {
    let index_backup = session_index_backup_path(backup_path);
    if index_backup.exists() {
        let text = fs::read(&index_backup)
            .map_err(|error| format!("Failed to read Codex session index backup: {error}"))?;
        write_atomic_with_retry(&paths.session_index_path, &text)?;
    }
    let meta_backup = session_meta_backup_path(backup_path);
    if !meta_backup.exists() {
        return Ok((0, 0));
    }
    let content = fs::read_to_string(&meta_backup)
        .map_err(|error| format!("Failed to read Codex session metadata backup: {error}"))?;
    let entries: Vec<SessionMetaBackupEntry> = serde_json::from_str(&content)
        .map_err(|error| format!("Failed to parse Codex session metadata backup: {error}"))?;
    let mut restored = 0;
    let mut skipped = 0;
    for entry in entries {
        let Ok(path) = join_safe_relative(&paths.codex_home, &entry.relative_path) else {
            skipped += 1;
            continue;
        };
        if !path.exists() {
            skipped += 1;
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read Codex session file for restore: {error}"))?;
        let (_, line_ending, remainder) = split_first_line(&text);
        let new_text = format!("{}{}{}", entry.first_line, line_ending, remainder);
        write_atomic_with_retry(&path, new_text.as_bytes())?;
        restored += 1;
    }
    Ok((restored, skipped))
}

fn join_safe_relative(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for component in Path::new(relative_path).components() {
        match component {
            std::path::Component::Normal(value) => path.push(value),
            std::path::Component::CurDir => {}
            _ => {
                return Err(format!(
                    "Unsafe Codex backup relative path: {relative_path}"
                ))
            }
        }
    }
    Ok(path)
}

fn list_backup_files(backup_dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }
    let re = Regex::new(r"^state_5\.sqlite\..*\.bak$").unwrap();
    let mut entries = Vec::new();
    for entry in fs::read_dir(backup_dir).map_err(|error| {
        format!(
            "Failed to read Codex history backup directory {}: {error}",
            backup_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| format!("Failed to read Codex backup entry: {error}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if re.is_match(name) {
            entries.push(path);
        }
    }
    entries.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct TestCodexHistory {
        _temp_dir: TempDir,
        root: PathBuf,
    }

    impl TestCodexHistory {
        fn new() -> Self {
            let temp_dir = tempfile::tempdir().expect("tempdir");
            let root = temp_dir.path().join("codex-home");
            fs::create_dir_all(root.join("sessions/2026/05/21")).expect("create sessions");
            fs::write(
                root.join("config.toml"),
                r#"model_provider = "new-provider"
model = "gpt-new"
"#,
            )
            .expect("write config");
            Self {
                _temp_dir: temp_dir,
                root,
            }
        }

        fn create_db(&self, with_model: bool) {
            let conn = Connection::open(self.root.join("state_5.sqlite")).expect("open db");
            if with_model {
                conn.execute_batch(
                    r#"
                    CREATE TABLE threads (
                        id TEXT PRIMARY KEY,
                        model_provider TEXT,
                        model TEXT,
                        title TEXT,
                        updated_at INTEGER,
                        archived INTEGER
                    );
                    INSERT INTO threads (id, model_provider, model, title, updated_at, archived)
                    VALUES
                        ('11111111-1111-1111-1111-111111111111', 'old-provider', 'gpt-old', 'Old thread', 1700000000, 0),
                        ('22222222-2222-2222-2222-222222222222', 'new-provider', 'gpt-old', 'Model old', 1700000100, 0),
                        ('33333333-3333-3333-3333-333333333333', 'new-provider', 'gpt-new', 'Archived', 1700000200, 1),
                        ('44444444-4444-4444-4444-444444444444', 'old-provider', 'gpt-other', 'Other model', 1700000300, 0);
                    "#,
                )
                .expect("create model db");
            } else {
                conn.execute_batch(
                    r#"
                    CREATE TABLE threads (
                        id TEXT PRIMARY KEY,
                        model_provider TEXT,
                        title TEXT,
                        updated_at INTEGER
                    );
                    INSERT INTO threads (id, model_provider, title, updated_at)
                    VALUES ('11111111-1111-1111-1111-111111111111', 'old-provider', 'Old thread', 1700000000);
                    "#,
                )
                .expect("create legacy db");
            }
        }

        fn write_session(&self, id: &str, provider: &str, model: &str) -> PathBuf {
            let path = self
                .root
                .join("sessions/2026/05/21")
                .join(format!("rollout-2026-05-21T00-00-00-{id}.jsonl"));
            fs::write(
                &path,
                format!(
                    "{}\n{}\n",
                    serde_json::json!({
                        "type": "session_meta",
                        "payload": {
                            "id": id,
                            "cwd": "/tmp/project",
                            "model_provider": provider,
                            "model": model
                        }
                    }),
                    serde_json::json!({
                        "type": "response_item",
                        "payload": {"type": "message", "role": "user", "content": "hello"}
                    })
                ),
            )
            .expect("write session");
            path
        }
    }

    #[test]
    fn sync_updates_provider_only_and_preserves_existing_models() {
        let env = TestCodexHistory::new();
        env.create_db(true);
        let session_path = env.write_session(
            "11111111-1111-1111-1111-111111111111",
            "old-provider",
            "gpt-old",
        );

        let status = get_status(&env.root).expect("status");
        assert_eq!(status.model_mismatch_threads, 0);
        assert!(status.has_work);

        let result = sync(&env.root).expect("sync");

        assert_eq!(result.updated_thread_rows, 2);
        assert_eq!(result.updated_session_files, 1);
        assert!(result.backup_path.contains("pre-sync"));
        let conn = Connection::open(env.root.join("state_5.sqlite")).expect("open db");
        let rows: Vec<(String, String)> = conn
            .prepare("SELECT model_provider, model FROM threads WHERE archived = 0 ORDER BY id")
            .expect("prepare")
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query")
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            rows,
            vec![
                ("new-provider".to_string(), "gpt-old".to_string()),
                ("new-provider".to_string(), "gpt-old".to_string()),
                ("new-provider".to_string(), "gpt-other".to_string()),
            ]
        );
        let content = fs::read_to_string(session_path).expect("read session");
        let first_line = content.lines().next().unwrap();
        assert!(first_line.contains("new-provider"));
        assert!(first_line.contains("gpt-old"));
        assert!(!first_line.contains("gpt-new"));
        assert!(content.contains("hello"));
    }

    #[test]
    fn sync_legacy_schema_updates_provider_only() {
        let env = TestCodexHistory::new();
        env.create_db(false);

        let result = sync(&env.root).expect("sync legacy");

        assert_eq!(result.updated_thread_rows, 1);
        let conn = Connection::open(env.root.join("state_5.sqlite")).expect("open db");
        let provider: String = conn
            .query_row("SELECT model_provider FROM threads", [], |row| row.get(0))
            .expect("provider");
        assert_eq!(provider, "new-provider");
    }

    #[test]
    fn status_ignores_model_only_differences() {
        let env = TestCodexHistory::new();
        let conn = Connection::open(env.root.join("state_5.sqlite")).expect("open db");
        conn.execute_batch(
            r#"
            CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                model_provider TEXT,
                model TEXT,
                title TEXT,
                updated_at INTEGER,
                archived INTEGER
            );
            INSERT INTO threads (id, model_provider, model, title, updated_at, archived)
            VALUES ('22222222-2222-2222-2222-222222222222', 'new-provider', 'gpt-old', 'Model old', 1700000100, 0);
            "#,
        )
        .expect("create db");
        env.write_session(
            "22222222-2222-2222-2222-222222222222",
            "new-provider",
            "gpt-old",
        );
        fs::write(
            env.root.join("session_index.jsonl"),
            format!(
                "{}\n",
                serde_json::json!({
                    "id": "22222222-2222-2222-2222-222222222222",
                    "thread_name": "Model old",
                    "updated_at": "2026-05-21T00:00:00Z"
                })
            ),
        )
        .expect("write index");

        let status = get_status(&env.root).expect("status");

        assert_eq!(status.provider_mismatch_threads, 0);
        assert_eq!(status.model_mismatch_threads, 0);
        assert_eq!(status.session_meta_mismatch_count, 0);
        assert!(!status.has_work);
    }

    #[test]
    fn official_auth_without_config_provider_uses_official_fallback() {
        let env = TestCodexHistory::new();
        fs::write(env.root.join("config.toml"), "model = \"gpt-new\"\n").expect("write config");
        fs::write(
            env.root.join("auth.json"),
            serde_json::json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": "access-token",
                    "refresh_token": "refresh-token"
                }
            })
            .to_string(),
        )
        .expect("write auth");
        env.create_db(false);

        let status = get_status(&env.root).expect("status");

        assert_eq!(status.current_provider, OFFICIAL_MODEL_PROVIDER_ID);
        assert!(status.has_work);
    }

    #[test]
    fn api_key_auth_without_config_provider_does_not_use_official_fallback() {
        let env = TestCodexHistory::new();
        fs::write(env.root.join("config.toml"), "model = \"gpt-new\"\n").expect("write config");
        fs::write(
            env.root.join("auth.json"),
            serde_json::json!({
                "auth_mode": "apikey",
                "OPENAI_API_KEY": "sk-test"
            })
            .to_string(),
        )
        .expect("write auth");
        env.create_db(false);

        let error = get_status(&env.root).expect_err("status should fail");

        assert!(error.contains("Could not find model_provider"));
    }

    #[test]
    fn restore_latest_restores_database_and_session_metadata() {
        let env = TestCodexHistory::new();
        env.create_db(true);
        let session_path = env.write_session(
            "11111111-1111-1111-1111-111111111111",
            "old-provider",
            "gpt-old",
        );
        backup(&env.root, "manual").expect("backup");
        sync(&env.root).expect("sync");

        let restored = restore_latest(&env.root).expect("restore");

        assert!(restored.restored_backup_path.contains("pre-sync"));
        let conn = Connection::open(env.root.join("state_5.sqlite")).expect("open db");
        let provider: String = conn
            .query_row(
                "SELECT model_provider FROM threads WHERE id = '11111111-1111-1111-1111-111111111111'",
                [],
                |row| row.get(0),
            )
            .expect("provider");
        assert_eq!(provider, "old-provider");
        let first_line = fs::read_to_string(session_path)
            .expect("read session")
            .lines()
            .next()
            .unwrap()
            .to_string();
        assert!(first_line.contains("old-provider"));
        assert!(restored.safety_backup_path.contains("pre-restore"));
    }
}
