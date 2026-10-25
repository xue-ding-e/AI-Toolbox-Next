//! Credential preparation for manual usage reads and the existing OAuth owners.
//! Network usage reads never hold the account-operation lock: switching accounts
//! remains possible, and every recovery reloads state instead of replaying a snapshot.

use super::*;
use std::path::PathBuf;
use std::sync::{Arc, Weak};

static USAGE_LOCKS: LazyLock<Mutex<HashMap<String, Weak<AsyncMutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn usage_lock(account_id: &str) -> Arc<AsyncMutex<()>> {
    let mut locks = USAGE_LOCKS.lock().unwrap_or_else(|error| error.into_inner());
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(account_id).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(AsyncMutex::new(()));
    locks.insert(account_id.to_string(), Arc::downgrade(&lock));
    lock
}

#[derive(Debug)]
pub(super) enum UsageError {
    Unauthorized,
    Other(String),
}

impl std::fmt::Display for UsageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => write!(formatter, "CODEX_REAUTH_REQUIRED: Codex usage returned HTTP 401; sign in to this account again"),
            Self::Other(message) => formatter.write_str(message),
        }
    }
}

pub(super) async fn fetch_usage(
    db: &SqliteDbState,
    access_token: &str,
    account_id: Option<&str>,
    plan_type: Option<&str>,
) -> Result<UsageSnapshot, UsageError> {
    let (client, _, usage_url) = account_http_client(db).await.map_err(UsageError::Other)?;
    let mut request = client.get(usage_url)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AI-Toolbox Codex OAuth")
        .header("Content-Type", "application/json");
    if let Some(account_id) = account_id.map(str::trim).filter(|id| !id.is_empty()) {
        request = request.header("Chatgpt-Account-Id", account_id);
    }
    let response = request.send().await
        .map_err(|error| UsageError::Other(format!("Failed to fetch usage: {error}")))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(UsageError::Unauthorized);
    }
    let response = response.error_for_status()
        .map_err(|error| UsageError::Other(format!("Codex usage request failed: {error}")))?;
    let body = response.json::<Value>().await
        .map_err(|_| UsageError::Other("Failed to parse usage response".to_string()))?;
    Ok(parse_usage_snapshot(&body, plan_type))
}

pub(super) fn access_token(auth: &Value) -> Result<&str, String> {
    auth.pointer("/tokens/access_token").and_then(Value::as_str)
        .map(str::trim).filter(|value| !value.is_empty())
        .ok_or_else(|| "Official account is missing access token; sign in again".to_string())
}

fn identity(auth: &Value) -> Option<(String, Option<String>)> {
    let claims = |kind| auth.pointer(kind).and_then(Value::as_str)
        .and_then(|token| decode_jwt_payload(token).ok());
    let id = claims("/tokens/id_token");
    // Opaque access tokens remain supported. When a JWT does carry identity
    // claims, they must agree with the ID token and explicit workspace header.
    let access = claims("/tokens/access_token");
    let mut account_id = auth.pointer("/tokens/account_id").and_then(Value::as_str)
        .map(str::trim).filter(|value| !value.is_empty()).map(str::to_string);
    let mut user_id: Option<String> = None;
    let mut subject: Option<String> = None;
    for claims in [id.as_ref(), access.as_ref()].into_iter().flatten() {
        if let Some(auth) = claims.auth.as_ref() {
            if let Some(claimed) = auth.chatgpt_account_id.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                if account_id.as_deref().is_some_and(|expected| expected != claimed) { return None }
                account_id = Some(claimed.to_string());
            }
            if let Some(claimed) = auth.chatgpt_user_id.as_ref().or(auth.user_id.as_ref())
                .filter(|value| !value.trim().is_empty()) {
                if user_id.as_ref().is_some_and(|expected| expected != claimed) { return None }
                user_id = Some(claimed.clone());
            }
        }
        if let Some(claimed) = claims.sub.as_ref().filter(|value| !value.trim().is_empty()) {
            if subject.as_ref().is_some_and(|expected| expected != claimed) { return None }
            subject = Some(claimed.clone());
        }
    }
    Some((account_id?, user_id.or(subject)))
}

pub(super) fn same_identity(first: &Value, second: &Value) -> bool {
    matches!((identity(first), identity(second)), (Some(a), Some(b)) if a == b)
}

fn validate_account_auth(account: &CodexOfficialAccount, auth: &Value) -> Result<(), String> {
    access_token(auth)?;
    if !is_official_runtime(auth) {
        return Err("Official account snapshot is not a ChatGPT credential; sign in again".to_string());
    }
    let (id, _) = identity(auth)
        .ok_or_else(|| "Official account identity is missing or inconsistent; sign in again".to_string())?;
    // The signed-token context is authoritative. Legacy metadata can lag behind
    // a valid snapshot; it must not select a different usage account header.
    let _ = (account, id);
    Ok(())
}

fn newer_runtime_auth(stored: &Value, runtime: &Value) -> bool {
    if !is_official_runtime(runtime) || !same_identity(stored, runtime)
        || stored.get("tokens") == runtime.get("tokens") {
        return false;
    }
    let expiration = |auth: &Value| access_token(auth).ok().and_then(token_expiration_unix);
    let Some(next_expiration) = expiration(runtime) else { return false };
    if next_expiration <= chrono::Utc::now().timestamp() {
        return false;
    }
    let old_expiration = expiration(stored);
    if old_expiration.is_none_or(|old| next_expiration > old) {
        return true;
    }
    let refreshed_at = |auth: &Value| auth.get("last_refresh").and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok());
    old_expiration == Some(next_expiration)
        && matches!((refreshed_at(stored), refreshed_at(runtime)), (Some(old), Some(new)) if new > old)
}

fn is_official_runtime(auth: &Value) -> bool {
    auth.get("auth_mode").and_then(Value::as_str) == Some("chatgpt")
        && auth.get("OPENAI_API_KEY").is_none_or(|value| value.is_null() || value.as_str() == Some(""))
}

#[derive(Clone)]
pub(super) struct RuntimeSnapshot {
    path: PathBuf,
    auth: Value,
    configured_root: Value,
    applied_owners: (Vec<String>, Vec<String>),
}

fn applied_owners(conn: &rusqlite::Connection) -> Result<(Vec<String>, Vec<String>), String> {
    let applied = |table| -> Result<Vec<String>, String> {
        let mut ids: Vec<String> = db_list(conn, table, None)?.into_iter()
            .filter(|value| value.get("is_applied").and_then(Value::as_bool) == Some(true))
            .filter_map(|value| value.get("id").and_then(Value::as_str).map(str::to_string)).collect();
        ids.sort();
        Ok(ids)
    };
    Ok((applied(DbTable::CodexProvider)?, applied(DbTable::CodexOfficialAccount)?))
}

fn configured_root(db: &SqliteDbState) -> Result<Value, String> {
    db.with_conn(|conn| Ok(db_get(conn, DbTable::CodexCommonConfig, "common")?
        .and_then(|value| value.get("root_dir").cloned()).unwrap_or(Value::Null)))
}

pub(super) async fn runtime_root(db: &SqliteDbState) -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        // Runtime-location cache is global, not keyed by database. Never let a
        // unit test fall through to a cached or real host Codex profile.
        let root = configured_root(db)?.as_str().map(PathBuf::from)
            .ok_or_else(|| "Synthetic Codex root is required in tests".to_string())?;
        if !root.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("ai-toolbox-issue440-"))
            || !root.join(".issue440-synthetic").is_file() {
            return Err("Synthetic Codex root marker is required in tests".to_string());
        }
        return Ok(root);
    }
    #[cfg(not(test))]
    get_codex_root_dir_from_db_async(db).await
}

pub(super) async fn read_runtime(db: &SqliteDbState) -> Result<RuntimeSnapshot, String> {
    let root = configured_root(db)?;
    let path = runtime_root(db).await?.join("auth.json");
    let content = crate::coding::file_io::read_optional_text_file_with_timeout(path.clone(), "Codex auth.json").await?;
    let auth = content.map(|content| serde_json::from_str(&content)
        .map_err(|_| "Failed to parse Codex auth.json".to_string()))
        .transpose()?.unwrap_or_else(|| serde_json::json!({}));
    let applied_owners = db.with_conn(applied_owners)?;
    Ok(RuntimeSnapshot { path, auth, configured_root: root, applied_owners })
}

// Caller holds ACCOUNT_OPERATION_LOCK. Observed external changes are rejected;
// the final read/rename is not a cross-process filesystem compare-and-swap.
// Slow UNC I/O stays on the blocking pool, not a Tokio worker. The
// async-acquired file fence also protects newer application writers after cancellation.
async fn write_runtime_if_unchanged(
    db: &SqliteDbState,
    runtime: &RuntimeSnapshot,
    refreshed: &Value,
    account: Option<&CodexOfficialAccount>,
    provider_id: &str,
) -> Result<bool, String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct CancelWrite(Arc<AtomicBool>);
    impl Drop for CancelWrite {
        fn drop(&mut self) { self.0.store(true, Ordering::SeqCst); }
    }
    if runtime_root(db).await?.join("auth.json") != runtime.path { return Ok(false) }
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel = CancelWrite(cancelled.clone());
    let db = db.clone();
    let runtime = runtime.clone();
    let refreshed = refreshed.clone();
    let account = account.cloned();
    let provider_id = provider_id.to_string();
    let file_guard = lock_codex_auth_file().await?;
    let task = tokio::task::spawn_blocking(move || {
        let _file = file_guard;
        if cancelled.load(Ordering::SeqCst) { return Ok(false) }
        // Do not hold SQLite's global mutex while a network-path read blocks.
        let current = fs::read_to_string(&runtime.path)
            .ok().and_then(|content| serde_json::from_str::<Value>(&content).ok());
        if current.as_ref() != Some(&runtime.auth) || !is_official_runtime(&runtime.auth)
            || !same_identity(&runtime.auth, &refreshed)
            || cancelled.load(Ordering::SeqCst) { return Ok(false) }
        let merged = merge_official_runtime_auth(&runtime.auth, &refreshed);
        if merged == runtime.auth { return Ok(false) }
        let can_commit = db.with_conn(|conn| {
            let current_root = db_get(conn, DbTable::CodexCommonConfig, "common")?
                .and_then(|value| value.get("root_dir").cloned()).unwrap_or(Value::Null);
            if current_root != runtime.configured_root { return Ok(false) }
            let Some(provider) = db_get(conn, DbTable::CodexProvider, &provider_id)? else { return Ok(false) };
            let provider = adapter::from_db_value_provider(provider);
            if provider.category != "official" { return Ok(false) }
            if let Some(account) = account.as_ref() {
                if !provider.is_applied || provider.is_disabled { return Ok(false) }
                let Some(current) = db_get(conn, DbTable::CodexOfficialAccount, &account.id)? else { return Ok(false) };
                let current = adapter::from_db_value_official_account(current);
                if !current.is_applied || current.provider_id != provider_id { return Ok(false) }
            } else if applied_owners(conn)? != runtime.applied_owners { return Ok(false) }
            Ok(true)
        })?;
        if !can_commit { return Ok(false) }
        // The operation/file locks, not SQLite's connection mutex, serialize
        // this filesystem commit with other in-application auth writers.
        write_auth_json_atomic_if(&runtime.path, &merged, || !cancelled.load(Ordering::SeqCst))
    });
    tokio::time::timeout(crate::coding::file_io::DEFAULT_CONFIG_FILE_IO_TIMEOUT, task)
        .await.map_err(|_| "Timed out writing Codex credentials; the file operation may still be finishing".to_string())?
        .map_err(|_| "Failed to complete Codex credential file write".to_string())?
}

pub(super) async fn prepare_persisted_locked(
    db: &SqliteDbState,
    provider_id: &str,
    account_id: &str,
    rejected_access: Option<&str>,
    wrote_runtime: &mut bool,
) -> Result<Value, String> {
    let provider = query_provider(db, provider_id).await?;
    if provider.category != "official" { return Err("Only official Codex providers can refresh official account usage".to_string()) }
    let mut account = load_official_account(db, account_id).await?;
    if account.provider_id != provider_id { return Err("Official account does not belong to the selected provider".to_string()) }
    let mut auth = auth_json_from_snapshot(account.auth_snapshot.as_deref()
        .ok_or_else(|| "Official account snapshot is missing".to_string())?)?;
    validate_account_auth(&account, &auth)?;
    if account.account_id.as_deref() != identity(&auth).as_ref().map(|identity| identity.0.as_str()) {
        account = persist_refreshed_account_snapshot(db, &account, &auth).await?;
    }
    let runtime = if account.is_applied && provider.is_applied && !provider.is_disabled {
        Some(read_runtime(db).await?)
    } else { None };
    if let Some(runtime) = runtime.as_ref() {
        if newer_runtime_auth(&auth, &runtime.auth) {
            account = persist_refreshed_account_snapshot(db, &account, &runtime.auth).await?;
            auth = runtime.auth.clone();
        }
    }
    // A concurrent owner may already have replaced the rejected access token.
    // Only force an exchange if the latest token is still the rejected one.
    let force = rejected_access.is_some_and(|rejected| access_token(&auth).ok() == Some(rejected));
    let refreshed = ensure_fresh_official_runtime_auth_with_force(db, &auth, force).await?;
    if !same_identity(&auth, &refreshed) { return Err("Refreshed Codex credential changed account identity; sign in again".to_string()) }
    validate_account_auth(&account, &refreshed)?;
    if refreshed != auth {
        // Never spend a rotated token on usage, or project it live, before its
        // refresh token has durably replaced the snapshot we actually loaded.
        account = persist_refreshed_account_snapshot(db, &account, &refreshed).await?;
        if let Some(runtime) = runtime.as_ref() {
            *wrote_runtime |= write_runtime_if_unchanged(db, runtime, &refreshed, Some(&account), provider_id).await?;
        }
    }
    if rejected_access.is_some_and(|rejected| access_token(&refreshed).ok() == Some(rejected)) {
        return Err("CODEX_REAUTH_REQUIRED: Codex rejected the recovered credential; sign in to this account again".to_string());
    }
    Ok(refreshed)
}

async fn fetch_prepared(db: &SqliteDbState, auth: &Value) -> Result<UsageSnapshot, UsageError> {
    let access = access_token(auth).map_err(UsageError::Other)?;
    let account_id = identity(auth).ok_or_else(|| UsageError::Other("Official account identity is missing or inconsistent".to_string()))?.0;
    let plan = usage_plan_type_from_auth(auth);
    fetch_usage(db, access, Some(&account_id), plan.as_deref()).await
}

pub(super) async fn refresh_persisted(
    db: &SqliteDbState,
    account: &CodexOfficialAccount,
    wrote_runtime: &mut bool,
) -> Result<CodexOfficialAccount, String> {
    // Order is usage -> account operation -> OAuth -> cache. Apply/background
    // never acquire a usage lock, so an in-flight HTTP read cannot block a switch.
    let lock = usage_lock(&account.id);
    let _usage = lock.lock().await;
    let result = async {
        let auth = {
            let _operation = ACCOUNT_OPERATION_LOCK.lock().await;
            prepare_persisted_locked(db, &account.provider_id, &account.id, None, wrote_runtime).await?
        };
        match fetch_prepared(db, &auth).await {
            Err(UsageError::Unauthorized) => {
                let recovered = {
                    let _operation = ACCOUNT_OPERATION_LOCK.lock().await;
                    prepare_persisted_locked(db, &account.provider_id, &account.id, Some(access_token(&auth)?), wrote_runtime).await?
                };
                fetch_prepared(db, &recovered).await.map_err(|error| error.to_string())
            }
            result => result.map_err(|error| error.to_string()),
        }
    }.await;
    persist_usage_result(db, &account.id, result).await
}

pub(super) async fn refresh_local(
    db: &SqliteDbState,
    provider_id: &str,
    wrote_runtime: &mut bool,
) -> Result<CodexOfficialAccount, String> {
    let lock = usage_lock(LOCAL_OFFICIAL_ACCOUNT_ID);
    let _usage = lock.lock().await;
    let initial = {
        let _operation = ACCOUNT_OPERATION_LOCK.lock().await;
        read_runtime(db).await?
    };
    if !is_official_runtime(&initial.auth) || !auth_has_official_runtime(&initial.auth)
        || identity(&initial.auth).is_none() {
        return Err("Current local auth.json does not contain a consistent official Codex login".to_string());
    }
    let mut rejected = None;
    for attempt in 0..2 {
        let auth = {
            let _operation = ACCOUNT_OPERATION_LOCK.lock().await;
            let current = read_runtime(db).await?;
            if current.path != initial.path || current.configured_root != initial.configured_root
                || current.applied_owners != initial.applied_owners
                || !is_official_runtime(&current.auth) || !same_identity(&initial.auth, &current.auth) {
                return Err("Local Codex account or root changed during refresh; refresh the current account again".to_string());
            }
            let force = rejected.as_deref().is_some_and(|token| access_token(&current.auth).ok() == Some(token));
            let refreshed = ensure_fresh_official_runtime_auth_with_force(db, &current.auth, force).await?;
            if !same_identity(&initial.auth, &refreshed) { return Err("Local Codex credential changed account identity".to_string()) }
            if refreshed != current.auth {
                if !write_runtime_if_unchanged(db, &current, &refreshed, None, provider_id).await? {
                    return Err("Local Codex account or root changed before credential persistence".to_string());
                }
                *wrote_runtime = true;
            }
            if rejected.as_deref().is_some_and(|token| access_token(&refreshed).ok() == Some(token)) {
                return Err(UsageError::Unauthorized.to_string());
            }
            refreshed
        };
        match fetch_prepared(db, &auth).await {
            Err(UsageError::Unauthorized) if attempt == 0 => {
                rejected = Some(access_token(&auth)?.to_string());
            }
            result => {
                let snapshot = result.map_err(|error| error.to_string())?;
                let _operation = ACCOUNT_OPERATION_LOCK.lock().await;
                let current = read_runtime(db).await?;
                let provider = query_provider(db, provider_id).await?;
                if current.path != initial.path || current.configured_root != initial.configured_root
                    || current.applied_owners != initial.applied_owners
                    || !is_official_runtime(&current.auth)
                    || !same_identity(&auth, &current.auth) || provider.category != "official" {
                    return Err("Local Codex account or root changed during usage request".to_string());
                }
                let content = build_account_content_from_auth_snapshot(provider_id, &auth, Some(&snapshot), None)?;
                let mut account = assign_provider_id(build_virtual_local_account(&auth), provider_id);
                account.limit_short_label = content.limit_short_label;
                account.limit_5h_text = content.limit_5h_text;
                account.limit_weekly_text = content.limit_weekly_text;
                account.limit_monthly_text = content.limit_monthly_text;
                account.limit_5h_reset_at = content.limit_5h_reset_at;
                account.limit_weekly_reset_at = content.limit_weekly_reset_at;
                account.limit_monthly_reset_at = content.limit_monthly_reset_at;
                account.reset_credits_available = content.reset_credits_available;
                account.credits = content.credits;
                account.last_limits_fetched_at = content.last_limits_fetched_at;
                return Ok(account);
            }
        }
    }
    unreachable!("one bounded unauthorized retry")
}
