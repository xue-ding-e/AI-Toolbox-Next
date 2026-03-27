mod claude_code;
mod codex;
mod open_claw;
mod open_code;
mod utils;

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::coding::runtime_location::{
    build_windows_unc_path, expand_home_from_user_root, get_claude_runtime_location_async,
    get_codex_runtime_location_async, get_openclaw_runtime_location_async,
    get_opencode_runtime_location_async, RuntimeLocationInfo,
};
use crate::db::DbState;

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

#[tauri::command]
pub async fn list_tool_sessions(
    state: tauri::State<'_, DbState>,
    tool: String,
    query: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<SessionListPage, String> {
    let session_tool = SessionTool::parse(tool.trim())?;
    let query = normalize_query(query);
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(10).clamp(1, 50);
    let context = resolve_context(&state.db(), session_tool).await?;

    tauri::async_runtime::spawn_blocking(move || {
        list_sessions_blocking(context, query, page as usize, page_size as usize)
    })
    .await
    .map_err(|error| format!("Failed to list sessions: {error}"))?
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

fn list_sessions_blocking(
    context: ToolSessionContext,
    query: Option<String>,
    page: usize,
    page_size: usize,
) -> Result<SessionListPage, String> {
    let sessions = scan_sessions(&context);
    let filtered_sessions = if let Some(query_text) = query.as_deref() {
        filter_sessions_by_query(&context, sessions, query_text)
    } else {
        sessions
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
    let sessions = scan_sessions(&context);
    let meta = sessions
        .into_iter()
        .find(|session| session.source_path == source_path)
        .ok_or_else(|| "Session not found".to_string())?;
    let messages = load_messages(&context, &meta.source_path)?;

    Ok(SessionDetail { meta, messages })
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

            load_messages(context, &session.source_path)
                .map(|messages| {
                    messages
                        .iter()
                        .any(|message| message.content.to_lowercase().contains(&query_lower))
                })
                .unwrap_or(false)
        })
        .collect()
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
