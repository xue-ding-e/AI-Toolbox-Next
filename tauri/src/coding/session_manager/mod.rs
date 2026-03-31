mod claude_code;
mod codex;
mod open_claw;
mod open_code;
mod utils;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;

use crate::coding::runtime_location::{
    build_windows_unc_path, expand_home_from_user_root, get_claude_runtime_location_async,
    get_codex_runtime_location_async, get_openclaw_runtime_location_async,
    get_opencode_runtime_location_async, RuntimeLocationInfo,
};
use crate::db::DbState;

const SESSION_CACHE_TTL: Duration = Duration::from_secs(15);
const MAX_SESSION_CACHE_ENTRIES: usize = 16;
const DEFAULT_SESSION_PATH_LIMIT: usize = 200;
const MAX_SESSION_PATH_LIMIT: usize = 500;

#[derive(Debug, Clone)]
struct SessionCacheEntry {
    created_at: Instant,
    sessions: Vec<SessionMeta>,
}

static SESSION_LIST_CACHE: LazyLock<Mutex<HashMap<String, SessionCacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub provider_id: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<i64>,
    pub source_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_command: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListPage {
    pub items: Vec<SessionMeta>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub meta: SessionMeta,
    pub messages: Vec<SessionMessage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportedSessionFile {
    version: u8,
    tool: String,
    exported_at: String,
    meta: SessionMeta,
    messages: Vec<SessionMessage>,
}

#[derive(Debug, Clone)]
enum ToolSessionContext {
    Codex {
        sessions_root: PathBuf,
    },
    ClaudeCode {
        projects_root: PathBuf,
    },
    OpenClaw {
        agents_root: PathBuf,
    },
    OpenCode {
        data_root: PathBuf,
        sqlite_db_path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy)]
enum SessionTool {
    Codex,
    ClaudeCode,
    OpenClaw,
    OpenCode,
}

impl SessionTool {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "codex" => Ok(Self::Codex),
            "claudecode" | "claude_code" => Ok(Self::ClaudeCode),
            "openclaw" | "open_claw" => Ok(Self::OpenClaw),
            "opencode" | "open_code" => Ok(Self::OpenCode),
            _ => Err(format!("Unsupported session tool: {raw}")),
        }
    }
}

impl ToolSessionContext {
    fn cache_key(&self) -> String {
        match self {
            Self::Codex { sessions_root } => format!("codex:{}", sessions_root.display()),
            Self::ClaudeCode { projects_root } => {
                format!("claudecode:{}", projects_root.display())
            }
            Self::OpenClaw { agents_root } => format!("openclaw:{}", agents_root.display()),
            Self::OpenCode {
                data_root,
                sqlite_db_path,
            } => format!(
                "opencode:{}:{}",
                data_root.display(),
                sqlite_db_path.display()
            ),
        }
    }
}

#[tauri::command]
pub async fn list_tool_sessions(
    state: tauri::State<'_, DbState>,
    tool: String,
    query: Option<String>,
    path_filter: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    force_refresh: Option<bool>,
) -> Result<SessionListPage, String> {
    let session_tool = SessionTool::parse(tool.trim())?;
    let query = normalize_query(query);
    let path_filter = normalize_query(path_filter);
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(10).clamp(1, 50);
    let force_refresh = force_refresh.unwrap_or(false);
    let context = resolve_context(&state.db(), session_tool).await?;

    tauri::async_runtime::spawn_blocking(move || {
        list_sessions_blocking(
            context,
            query,
            path_filter,
            page as usize,
            page_size as usize,
            force_refresh,
        )
    })
    .await
    .map_err(|error| format!("Failed to list sessions: {error}"))?
}

#[tauri::command]
pub async fn list_tool_session_paths(
    state: tauri::State<'_, DbState>,
    tool: String,
    limit: Option<u32>,
    force_refresh: Option<bool>,
) -> Result<Vec<String>, String> {
    let session_tool = SessionTool::parse(tool.trim())?;
    let limit = limit
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_SESSION_PATH_LIMIT)
        .clamp(1, MAX_SESSION_PATH_LIMIT);
    let force_refresh = force_refresh.unwrap_or(false);
    let context = resolve_context(&state.db(), session_tool).await?;

    tauri::async_runtime::spawn_blocking(move || {
        list_session_paths_blocking(context, limit, force_refresh)
    })
    .await
    .map_err(|error| format!("Failed to list session paths: {error}"))?
}

#[tauri::command]
pub async fn get_tool_session_detail(
    state: tauri::State<'_, DbState>,
    tool: String,
    source_path: String,
) -> Result<SessionDetail, String> {
    let session_tool = SessionTool::parse(tool.trim())?;
    let context = resolve_context(&state.db(), session_tool).await?;

    tauri::async_runtime::spawn_blocking(move || get_session_detail_blocking(context, source_path))
        .await
        .map_err(|error| format!("Failed to load session detail: {error}"))?
}

#[tauri::command]
pub async fn delete_tool_session(
    state: tauri::State<'_, DbState>,
    tool: String,
    source_path: String,
) -> Result<(), String> {
    let session_tool = SessionTool::parse(tool.trim())?;
    let context = resolve_context(&state.db(), session_tool).await?;

    tauri::async_runtime::spawn_blocking(move || delete_session_blocking(context, source_path))
        .await
        .map_err(|error| format!("Failed to delete session: {error}"))?
}

#[tauri::command]
pub async fn export_tool_session(
    state: tauri::State<'_, DbState>,
    tool: String,
    source_path: String,
    export_path: String,
) -> Result<(), String> {
    let session_tool = SessionTool::parse(tool.trim())?;
    let context = resolve_context(&state.db(), session_tool).await?;

    tauri::async_runtime::spawn_blocking(move || {
        export_session_blocking(context, tool, source_path, export_path)
    })
    .await
    .map_err(|error| format!("Failed to export session: {error}"))?
}

#[tauri::command]
pub async fn rename_tool_session(
    state: tauri::State<'_, DbState>,
    tool: String,
    source_path: String,
    title: String,
) -> Result<(), String> {
    let session_tool = SessionTool::parse(tool.trim())?;
    let context = resolve_context(&state.db(), session_tool).await?;

    tauri::async_runtime::spawn_blocking(move || {
        rename_session_blocking(context, tool, source_path, title)
    })
    .await
    .map_err(|error| format!("Failed to rename session: {error}"))?
}

fn list_sessions_blocking(
    context: ToolSessionContext,
    query: Option<String>,
    path_filter: Option<String>,
    page: usize,
    page_size: usize,
    force_refresh: bool,
) -> Result<SessionListPage, String> {
    let sessions = get_cached_sessions(&context, force_refresh);
    let path_filtered_sessions = if let Some(path_filter_text) = path_filter.as_deref() {
        filter_sessions_by_path(sessions, path_filter_text)
    } else {
        sessions
    };
    let filtered_sessions = if let Some(query_text) = query.as_deref() {
        filter_sessions_by_query(&context, path_filtered_sessions, query_text)
    } else {
        path_filtered_sessions
    };

    let total = filtered_sessions.len();
    let start = page.saturating_sub(1) * page_size;
    let end = (start + page_size).min(total);
    let items = if start >= total {
        Vec::new()
    } else {
        filtered_sessions[start..end].to_vec()
    };

    Ok(SessionListPage {
        items,
        page: page as u32,
        page_size: page_size as u32,
        total,
        has_more: end < total,
    })
}

fn get_session_detail_blocking(
    context: ToolSessionContext,
    source_path: String,
) -> Result<SessionDetail, String> {
    let sessions = get_cached_sessions(&context, false);
    let meta = sessions
        .into_iter()
        .find(|session| session.source_path == source_path)
        .ok_or_else(|| "Session not found".to_string())?;
    let messages = load_messages(&context, &meta.source_path)?;

    Ok(SessionDetail { meta, messages })
}

fn list_session_paths_blocking(
    context: ToolSessionContext,
    limit: usize,
    force_refresh: bool,
) -> Result<Vec<String>, String> {
    let sessions = get_cached_sessions(&context, force_refresh);
    let mut paths = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    for session in sessions {
        let Some(project_dir) = session.project_dir.as_deref() else {
            continue;
        };
        let normalized = project_dir.trim();
        if normalized.is_empty() {
            continue;
        }

        let dedupe_key = normalized.to_ascii_lowercase();
        if seen_paths.insert(dedupe_key) {
            paths.push(normalized.to_string());
        }

        if paths.len() >= limit {
            break;
        }
    }

    Ok(paths)
}

fn delete_session_blocking(context: ToolSessionContext, source_path: String) -> Result<(), String> {
    let session = get_cached_sessions(&context, true)
        .into_iter()
        .find(|item| item.source_path == source_path)
        .ok_or_else(|| "Session not found".to_string())?;

    match &context {
        ToolSessionContext::Codex { .. } => {
            codex::delete_session(Path::new(&session.source_path))?;
        }
        ToolSessionContext::ClaudeCode { .. } => {
            claude_code::delete_session(Path::new(&session.source_path))?;
        }
        ToolSessionContext::OpenClaw { .. } => {
            open_claw::delete_session(Path::new(&session.source_path))?;
        }
        ToolSessionContext::OpenCode { .. } => {
            open_code::delete_session(&session.source_path)?;
        }
    }

    invalidate_cache(&context);
    Ok(())
}

fn export_session_blocking(
    context: ToolSessionContext,
    tool: String,
    source_path: String,
    export_path: String,
) -> Result<(), String> {
    let session_detail = get_session_detail_blocking(context, source_path)?;
    let exported_file = ExportedSessionFile {
        version: 1,
        tool,
        exported_at: Utc::now().to_rfc3339(),
        meta: session_detail.meta,
        messages: session_detail.messages,
    };
    let serialized = serde_json::to_string_pretty(&exported_file)
        .map_err(|error| format!("Failed to serialize session export: {error}"))?;

    let export_path_ref = Path::new(&export_path);
    if let Some(parent_dir) = export_path_ref.parent() {
        std::fs::create_dir_all(parent_dir).map_err(|error| {
            format!(
                "Failed to create export directory {}: {error}",
                parent_dir.display()
            )
        })?;
    }

    std::fs::write(export_path_ref, serialized).map_err(|error| {
        format!(
            "Failed to write exported session file {}: {error}",
            export_path_ref.display()
        )
    })?;

    Ok(())
}

fn rename_session_blocking(
    context: ToolSessionContext,
    tool: String,
    source_path: String,
    title: String,
) -> Result<(), String> {
    if tool != "opencode" {
        return Err("Only OpenCode sessions support title editing".to_string());
    }

    match &context {
        ToolSessionContext::OpenCode { .. } => {
            open_code::rename_session(&source_path, &title)?;
            invalidate_cache(&context);
            Ok(())
        }
        _ => Err("Only OpenCode sessions support title editing".to_string()),
    }
}

fn scan_sessions(context: &ToolSessionContext) -> Vec<SessionMeta> {
    let mut sessions = match context {
        ToolSessionContext::Codex { sessions_root } => codex::scan_sessions(sessions_root),
        ToolSessionContext::ClaudeCode { projects_root } => {
            claude_code::scan_sessions(projects_root)
        }
        ToolSessionContext::OpenClaw { agents_root } => open_claw::scan_sessions(agents_root),
        ToolSessionContext::OpenCode {
            data_root,
            sqlite_db_path,
        } => open_code::scan_sessions(data_root, sqlite_db_path),
    };

    sessions.sort_by(|left, right| {
        let left_ts = left.last_active_at.or(left.created_at).unwrap_or(0);
        let right_ts = right.last_active_at.or(right.created_at).unwrap_or(0);
        right_ts.cmp(&left_ts)
    });
    sessions
}

fn load_messages(
    context: &ToolSessionContext,
    source_path: &str,
) -> Result<Vec<SessionMessage>, String> {
    match context {
        ToolSessionContext::Codex { .. } => codex::load_messages(Path::new(source_path)),
        ToolSessionContext::ClaudeCode { .. } => claude_code::load_messages(Path::new(source_path)),
        ToolSessionContext::OpenClaw { .. } => open_claw::load_messages(Path::new(source_path)),
        ToolSessionContext::OpenCode { .. } => open_code::load_messages(source_path),
    }
}

fn get_cached_sessions(context: &ToolSessionContext, force_refresh: bool) -> Vec<SessionMeta> {
    let cache_key = context.cache_key();

    if let Ok(mut cache) = SESSION_LIST_CACHE.lock() {
        if force_refresh {
            cache.remove(&cache_key);
        } else if let Some(entry) = cache.get(&cache_key) {
            if entry.created_at.elapsed() <= SESSION_CACHE_TTL {
                return entry.sessions.clone();
            }

            cache.remove(&cache_key);
        }
    }

    let sessions = scan_sessions(context);

    if let Ok(mut cache) = SESSION_LIST_CACHE.lock() {
        cache.retain(|_, entry| entry.created_at.elapsed() <= SESSION_CACHE_TTL);

        if cache.len() >= MAX_SESSION_CACHE_ENTRIES {
            let oldest_key = cache
                .iter()
                .min_by_key(|(_, entry)| entry.created_at)
                .map(|(key, _)| key.clone());
            if let Some(oldest_key) = oldest_key {
                cache.remove(&oldest_key);
            }
        }

        cache.insert(
            cache_key,
            SessionCacheEntry {
                created_at: Instant::now(),
                sessions: sessions.clone(),
            },
        );
    }

    sessions
}

fn invalidate_cache(context: &ToolSessionContext) {
    let cache_key = context.cache_key();
    if let Ok(mut cache) = SESSION_LIST_CACHE.lock() {
        cache.remove(&cache_key);
    }
}

fn filter_sessions_by_query(
    context: &ToolSessionContext,
    sessions: Vec<SessionMeta>,
    query: &str,
) -> Vec<SessionMeta> {
    let query_lower = query.to_lowercase();

    sessions
        .into_iter()
        .filter(|session| {
            if meta_matches_query(session, &query_lower) {
                return true;
            }

            scan_session_content_for_query(context, &session.source_path, &query_lower)
                .unwrap_or(false)
        })
        .collect()
}

fn filter_sessions_by_path(sessions: Vec<SessionMeta>, path_filter: &str) -> Vec<SessionMeta> {
    let path_filter_lower = path_filter.to_ascii_lowercase();

    sessions
        .into_iter()
        .filter(|session| {
            session
                .project_dir
                .as_deref()
                .map(|value| contains_query(value, &path_filter_lower))
                .unwrap_or(false)
        })
        .collect()
}

fn scan_session_content_for_query(
    context: &ToolSessionContext,
    source_path: &str,
    query_lower: &str,
) -> Result<bool, String> {
    match context {
        ToolSessionContext::Codex { .. } => {
            codex::scan_messages_for_query(Path::new(source_path), query_lower)
        }
        ToolSessionContext::ClaudeCode { .. } => {
            claude_code::scan_messages_for_query(Path::new(source_path), query_lower)
        }
        ToolSessionContext::OpenClaw { .. } => {
            open_claw::scan_messages_for_query(Path::new(source_path), query_lower)
        }
        ToolSessionContext::OpenCode { .. } => {
            open_code::scan_messages_for_query(source_path, query_lower)
        }
    }
}

fn meta_matches_query(session: &SessionMeta, query_lower: &str) -> bool {
    contains_query(&session.session_id, query_lower)
        || session
            .title
            .as_deref()
            .map(|value| contains_query(value, query_lower))
            .unwrap_or(false)
        || session
            .summary
            .as_deref()
            .map(|value| contains_query(value, query_lower))
            .unwrap_or(false)
        || session
            .project_dir
            .as_deref()
            .map(|value| contains_query(value, query_lower))
            .unwrap_or(false)
}

fn contains_query(value: &str, query_lower: &str) -> bool {
    value.to_lowercase().contains(query_lower)
}

fn normalize_query(query: Option<String>) -> Option<String> {
    query
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn resolve_context(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    tool: SessionTool,
) -> Result<ToolSessionContext, String> {
    match tool {
        SessionTool::Codex => {
            let runtime_location = get_codex_runtime_location_async(db).await?;
            Ok(ToolSessionContext::Codex {
                sessions_root: runtime_location.host_path.join("sessions"),
            })
        }
        SessionTool::ClaudeCode => {
            let runtime_location = get_claude_runtime_location_async(db).await?;
            Ok(ToolSessionContext::ClaudeCode {
                projects_root: runtime_location.host_path.join("projects"),
            })
        }
        SessionTool::OpenClaw => {
            let runtime_location = get_openclaw_runtime_location_async(db).await?;
            let config_dir = runtime_location
                .host_path
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| "Failed to determine OpenClaw config directory".to_string())?;
            Ok(ToolSessionContext::OpenClaw {
                agents_root: config_dir.join("agents"),
            })
        }
        SessionTool::OpenCode => {
            let runtime_location = get_opencode_runtime_location_async(db).await?;
            let data_root = resolve_opencode_data_root(&runtime_location)?;
            Ok(ToolSessionContext::OpenCode {
                sqlite_db_path: data_root.join("opencode.db"),
                data_root,
            })
        }
    }
}

fn resolve_opencode_data_root(location: &RuntimeLocationInfo) -> Result<PathBuf, String> {
    if let Some(wsl) = &location.wsl {
        let linux_path =
            expand_home_from_user_root(wsl.linux_user_root.as_deref(), "~/.local/share/opencode");
        return Ok(build_windows_unc_path(&wsl.distro, &linux_path));
    }

    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        if !data_home.trim().is_empty() {
            return Ok(PathBuf::from(data_home).join("opencode"));
        }
    }

    Ok(get_home_dir()?
        .join(".local")
        .join("share")
        .join("opencode"))
}

fn get_home_dir() -> Result<PathBuf, String> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Failed to get home directory".to_string())
}
