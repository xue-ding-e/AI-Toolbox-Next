//! OMP subagent 集中配置(多套方案 + 一键切换)。
//!
//! 数据模型与 OhMyOpenAgent 对齐:数据库 `oh_my_pi_agents_config` 存多套
//! subagent 方案(profile),每套方案是 `agents: { name -> frontmatter 配置 }`
//! 的映射 + `other_fields`。apply 时把选中的方案整体渲染成
//! `<omp_root>/agents/*.md`(同时清理不在该方案里的自定义 agent 文件),
//! 使"目录 = 当前方案",与 OMP 本体 task-agent 发现(user `~/.omp/agent/agents`)
//! 一致。
//!
//! 文件(agents/*.md)是运行时事实源;数据库只存"方案"。删除方案不会误删
//! 正在运行的文件(clear applied 才清理目录)。通用 apply 细节见
//! [`apply_omp_agents_config_internal`]。

use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tauri::Emitter;

use super::adapter;
use super::commands::{
    get_omp_config_path_async, object_mut, read_yaml_object_or_empty, write_yaml_object,
};
use super::types::*;
use crate::coding::runtime_location;
use crate::db::helpers::{
    db_create, db_delete, db_get, db_list, db_patch_fields, db_put, db_query_by_bool,
    db_update_applied_status,
};
use crate::db::schema::{DbTable, OrderDirection, OrderField, OrderSpec, JsonFieldPath};
use crate::db::SqliteDbState;

/// 扫描目录:canonical `agents/`(落点)+ legacy `agent/`(仅读兼容)。
const AGENT_DIRECTORY_NAMES: [&str; 2] = ["agent", "agents"];

/// OMP bundled(task 子系统)内置 subagent 清单,镜像上游
/// `packages/coding-agent/src/task/agents.ts` 的 `EMBEDDED_AGENT_DEFS`。
const OMP_BUILTIN_AGENT_NAMES: [&str; 5] = [
    "task",
    "sonic",
    "scout",
    "reviewer",
    "security-reviewer",
];

/// `main` / `sub` 是 OMP 的会话 sentinel agentName,自定义 agent 不得占用。
const OMP_RESERVED_AGENT_NAMES: [&str; 2] = ["main", "sub"];

/// 单 agent 的运行时视图(前端逐 agent 编辑用)。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OmpAgent {
    pub name: String,
    /// 实际文件路径;内置 agent 无覆盖文件时为 `""`。
    pub path: String,
    pub frontmatter: String,
    pub prompt: String,
    pub raw_content: String,
    pub content_hash: String,
    pub config: Option<Value>,
    pub parse_error: Option<String>,
    pub is_builtin: bool,
    pub is_override: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveOmpAgentRequest {
    pub path: String,
    pub expected_content_hash: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOmpAgentRequest {
    pub path: String,
    pub expected_content_hash: String,
}

// ============================================================================
// 低层:文件读写 / 解析 / 校验
// ============================================================================

fn content_hash(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

/// 解析 `---` YAML frontmatter + body。
fn parse_omp_agent(content: &str) -> Result<(String, String, Value), String> {
    let normalized = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut offset = 0usize;
    let mut lines = normalized.split_inclusive('\n');
    let first_line = lines
        .next()
        .ok_or_else(|| "Agent file is empty".to_string())?;
    if first_line.trim_end_matches(['\r', '\n']) != "---" {
        return Err("Agent file must start with YAML frontmatter delimited by ---".to_string());
    }
    offset += first_line.len();
    let frontmatter_start = offset;
    let mut frontmatter_end = None;
    let mut body_start = None;

    for line in lines {
        let line_start = offset;
        offset += line.len();
        let marker = line.trim_end_matches(['\r', '\n']);
        if marker == "---" || marker == "..." {
            frontmatter_end = Some(line_start);
            body_start = Some(offset);
            break;
        }
    }

    let frontmatter_end = frontmatter_end
        .ok_or_else(|| "Agent YAML frontmatter is missing a closing --- delimiter".to_string())?;
    let body_start = body_start.unwrap_or(normalized.len());
    let frontmatter = normalized[frontmatter_start..frontmatter_end]
        .trim_end_matches(['\r', '\n'])
        .to_string();
    let prompt = normalized[body_start..]
        .trim_start_matches(['\r', '\n'])
        .to_string();
    let yaml_value = serde_yaml::from_str::<serde_yaml::Value>(&frontmatter)
        .map_err(|error| format!("Failed to parse YAML frontmatter: {error}"))?;
    let config = serde_json::to_value(yaml_value)
        .map_err(|error| format!("Failed to convert YAML frontmatter: {error}"))?;
    if !config.is_object() {
        return Err("Agent YAML frontmatter must be a mapping object".to_string());
    }

    Ok((frontmatter, prompt, config))
}

/// 镜像 OMP `parseAgentFields` 的严格子集。未知字段原样保留。
fn validate_omp_agent_config(name_from_file: &str, config: &Value) -> Result<(), String> {
    let object = config
        .as_object()
        .ok_or_else(|| "Agent YAML frontmatter must be a mapping object".to_string())?;
    let resolved_name = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(name_from_file);
    if OMP_RESERVED_AGENT_NAMES.contains(&resolved_name) {
        return Err("Agent name 'main' and 'sub' are reserved by OMP and cannot be used".to_string());
    }
    let description = object
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if description.is_empty() && !OMP_BUILTIN_AGENT_NAMES.contains(&resolved_name) {
        return Err("Custom OMP Agents require a non-empty description".to_string());
    }
    if let Some(model) = object.get("model") {
        let valid = model.is_string()
            || model
                .as_array()
                .map(|models| models.iter().all(Value::is_string))
                .unwrap_or(false);
        if !valid {
            return Err("OMP Agent model must be a string or a list of strings".to_string());
        }
    }
    if let Some(tools) = object.get("tools") {
        let valid = tools.is_string()
            || tools
                .as_array()
                .map(|items| items.iter().all(Value::is_string))
                .unwrap_or(false);
        if !valid {
            return Err("OMP Agent tools must be a string or a list of strings".to_string());
        }
    }
    if let Some(spawns) = object.get("spawns") {
        let valid = spawns.as_str().is_some()
            || spawns
                .as_array()
                .map(|items| items.iter().all(Value::is_string))
                .unwrap_or(false);
        if !valid {
            return Err("OMP Agent spawns must be a string or a list of strings".to_string());
        }
    }
    for key in ["thinkingLevel", "thinking"] {
        if let Some(value) = object.get(key) {
            if !value.is_string() {
                return Err(format!("OMP Agent {key} must be a string"));
            }
        }
    }
    for key in ["blocking", "readSummarize"] {
        if let Some(value) = object.get(key) {
            if !value.is_boolean() {
                return Err(format!("OMP Agent {key} must be a boolean"));
            }
        }
    }
    for key in ["prewalk", "advisor"] {
        if let Some(value) = object.get(key) {
            if !value.is_boolean() && !value.is_string() {
                return Err(format!("OMP Agent {key} must be a boolean or a string"));
            }
        }
    }
    if let Some(skills) = object.get("autoloadSkills") {
        let valid = skills.is_string()
            || skills
                .as_array()
                .map(|items| items.iter().all(Value::is_string))
                .unwrap_or(false);
        if !valid {
            return Err(
                "OMP Agent autoloadSkills must be a string or a list of strings".to_string(),
            );
        }
    }
    Ok(())
}

fn agent_name_from_config(name_from_file: &str, config: Option<&Value>) -> String {
    let from_config = config
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    from_config.unwrap_or_else(|| name_from_file.to_string())
}

fn read_agent_file(_root: &Path, file_path: &Path) -> Result<OmpAgent, String> {
    let raw_content = fs::read_to_string(file_path)
        .map_err(|error| format!("Failed to read {}: {error}", file_path.display()))?;
    let name_from_file = file_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string();
    let (frontmatter, prompt, parsed, parse_error) = match parse_omp_agent(&raw_content) {
        Ok((frontmatter, prompt, config)) => {
            match validate_omp_agent_config(&name_from_file, &config) {
                Ok(()) => (frontmatter, prompt, Some(config), None),
                Err(error) => (frontmatter, prompt, Some(config), Some(error)),
            }
        }
        Err(error) => (String::new(), String::new(), None, Some(error)),
    };
    let name = agent_name_from_config(&name_from_file, parsed.as_ref());
    let is_builtin = OMP_BUILTIN_AGENT_NAMES.contains(&name.as_str());
    let hash = content_hash(&raw_content);

    Ok(OmpAgent {
        name,
        path: file_path.to_string_lossy().to_string(),
        frontmatter,
        prompt,
        raw_content,
        content_hash: hash,
        config: parsed,
        parse_error,
        is_builtin,
        is_override: is_builtin,
    })
}

fn ensure_safe_agent_path(path: &Path, root: &Path) -> Result<(), String> {
    let is_markdown = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
    if !is_markdown {
        return Err("OMP Agent files must use the .md extension".to_string());
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("OMP Agent path cannot contain parent directory segments".to_string());
    }

    let allowed = AGENT_DIRECTORY_NAMES
        .iter()
        .any(|directory_name| path.starts_with(root.join(directory_name)));
    if !allowed {
        return Err(
            "The selected file is outside the configured OMP Agent directories".to_string(),
        );
    }
    Ok(())
}

async fn omp_root_dir_async(db: &SqliteDbState) -> Result<PathBuf, String> {
    Ok(runtime_location::get_oh_my_pi_runtime_location_async(db)
        .await?
        .host_path)
}

fn canonical_agents_dir(root: &Path) -> PathBuf {
    root.join("agents")
}

fn emit_omp_agents_changed<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let _ = app.emit("config-changed", "window");
    #[cfg(target_os = "windows")]
    let _ = app.emit("wsl-sync-request-omp", ());
}

// ============================================================================
// Profile 命令(数据库多套方案)
// ============================================================================

fn agents_config_order() -> Result<OrderSpec, String> {
    Ok(OrderSpec::new(vec![OrderField::json_integer(
        "sort_index",
        OrderDirection::Asc,
    )?]))
}

fn list_agents_configs_from_sqlite(db: &SqliteDbState) -> Result<Vec<OmpAgentsConfig>, String> {
    let mut configs = db.with_conn(|conn| {
        Ok(db_list(conn, DbTable::OhMyPiAgentsConfig, Some(&agents_config_order()?))
            .map(|records| {
                records
                    .into_iter()
                    .map(adapter::agents_from_db_value)
                    .collect::<Vec<_>>()
            })?)
    })?;
    configs.sort_by(|a, b| match (a.sort_index, b.sort_index) {
        (Some(ai), Some(bi)) => ai.cmp(&bi),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.name.cmp(&b.name),
    });
    Ok(configs)
}

fn get_agents_config_from_sqlite(
    db: &SqliteDbState,
    config_id: &str,
) -> Result<Option<OmpAgentsConfig>, String> {
    db.with_conn(|conn| {
        db_get(conn, DbTable::OhMyPiAgentsConfig, config_id)
            .map(|record| record.map(adapter::agents_from_db_value))
    })
}

fn put_agents_config_to_sqlite(db: &SqliteDbState, config_id: &str, data: &Value) -> Result<(), String> {
    db.with_conn(|conn| db_put(conn, DbTable::OhMyPiAgentsConfig, config_id, data))
}

/// 列出所有 OMP subagent 方案;空库时返回本地已有的 `agents/*.md` 桥接态
/// (`__local__`,只读预览)。
#[tauri::command]
pub async fn list_omp_agents_configs(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<OmpAgentsConfig>, String> {
    let db = state.db();
    let configs = list_agents_configs_from_sqlite(db)?;
    if configs.is_empty() {
        if let Ok(local) = load_local_agents_config(db).await {
            return Ok(vec![local]);
        }
    }
    Ok(configs)
}

/// 解析 role 字符串:例如 "anthropic/claude-sonnet-4-6:high" -> ("anthropic/claude-sonnet-4-6", Some("high"))
pub(crate) fn parse_role_string(raw: &str) -> (String, Option<String>) {
    let trimmed = raw.trim();
    if let Some(colon_idx) = trimmed.rfind(':') {
        let model_part = &trimmed[..colon_idx];
        let level_part = &trimmed[colon_idx + 1..];
        if !model_part.is_empty() && !level_part.is_empty() && !model_part.ends_with('/') {
            return (model_part.to_string(), Some(level_part.to_string()));
        }
    }
    (trimmed.to_string(), None)
}

/// 读取当前运行的 `agents/*.md` 目录与 `config.yml` 中的 `modelRoles`,组装为 `__local__` 桥接方案(不落库)。
async fn load_local_agents_config(db: &SqliteDbState) -> Result<OmpAgentsConfig, String> {
    let root = omp_root_dir_async(db).await?;
    let mut agents = Map::new();
    for directory_name in AGENT_DIRECTORY_NAMES {
        let directory = root.join(directory_name);
        if !directory.is_dir() {
            continue;
        }
        let mut files = fs::read_dir(&directory)
            .map_err(|error| format!("Failed to read {}: {error}", directory.display()))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_ok_and(|ft| ft.is_file()))
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            })
            .collect::<Vec<_>>();
        files.sort();
        for file in files {
            let Ok(agent) = read_agent_file(&root, &file) else {
                continue;
            };
            if let Some(config) = agent.config {
                let mut fields = config
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                fields.insert("name".to_string(), json!(agent.name));
                if !agent.prompt.trim().is_empty() {
                    fields.insert("prompt".to_string(), json!(agent.prompt));
                }
                agents.insert(agent.name.clone(), Value::Object(fields));
            }
        }
    }

    // 从 config.yml 读取当前运行时的 modelRoles 与 defaultThinkingLevel
    let mut model_roles_map = Map::new();
    if let Ok(config_path) = get_omp_config_path_async(db).await {
        if let Ok(settings) = read_yaml_object_or_empty(&config_path) {
            let default_thinking = settings
                .get("defaultThinkingLevel")
                .and_then(Value::as_str)
                .map(str::to_string);

            if let Some(roles) = settings.get("modelRoles").and_then(Value::as_object) {
                for (role, val) in roles {
                    if let Some(role_str) = val.as_str() {
                        let mut role_obj = Map::new();
                        let (model_part, thinking_part) = parse_role_string(role_str);
                        role_obj.insert("model".to_string(), json!(model_part));
                        let effective_thinking = thinking_part.or_else(|| {
                            if role == "default" {
                                default_thinking.clone()
                            } else {
                                None
                            }
                        });
                        if let Some(level) = effective_thinking {
                            role_obj.insert("thinkingLevel".to_string(), json!(level));
                        }
                        model_roles_map.insert(role.clone(), Value::Object(role_obj));
                    }
                }
            }
        }
    }

    let now = Local::now().to_rfc3339();
    Ok(OmpAgentsConfig {
        id: "__local__".to_string(),
        name: "Local agents/*.md".to_string(),
        is_applied: true,
        is_disabled: false,
        model_roles: if model_roles_map.is_empty() {
            None
        } else {
            Some(Value::Object(model_roles_map))
        },
        agents: if agents.is_empty() {
            None
        } else {
            Some(Value::Object(agents))
        },
        other_fields: None,
        sort_index: None,
        created_at: Some(now.clone()),
        updated_at: Some(now),
    })
}

#[tauri::command]
pub async fn create_omp_agents_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmpAgentsConfigInput,
) -> Result<OmpAgentsConfig, String> {
    let db = state.db();
    let now = Local::now().to_rfc3339();
    let content = OmpAgentsConfigContent {
        name: input.name,
        is_applied: false,
        is_disabled: false,
        model_roles: input.model_roles,
        agents: input.agents,
        other_fields: input.other_fields,
        sort_index: None,
        created_at: now.clone(),
        updated_at: now,
    };
    let data = adapter::agents_to_db_value(&content);
    let created = db
        .with_conn(|conn| db_create(conn, DbTable::OhMyPiAgentsConfig, &data))?;
    let _ = app.emit("config-changed", "window");
    Ok(adapter::agents_from_db_value(created))
}

#[tauri::command]
pub async fn update_omp_agents_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmpAgentsConfigInput,
) -> Result<OmpAgentsConfig, String> {
    let db = state.db();
    let config_id = input
        .id
        .clone()
        .ok_or_else(|| "ID is required for update".to_string())?;
    let existing = get_agents_config_from_sqlite(db, &config_id)?
        .ok_or_else(|| format!("OMP Agents config '{}' not found", config_id))?;
    let now = Local::now().to_rfc3339();
    let created_at = existing
        .created_at
        .clone()
        .unwrap_or_else(|| now.clone());
    let content = OmpAgentsConfigContent {
        name: input.name,
        is_applied: existing.is_applied,
        is_disabled: existing.is_disabled,
        model_roles: input.model_roles,
        agents: input.agents,
        other_fields: input.other_fields,
        sort_index: existing.sort_index,
        created_at,
        updated_at: now,
    };
    put_agents_config_to_sqlite(db, &config_id, &adapter::agents_to_db_value(&content))?;

    if existing.is_applied {
        if let Err(error) = apply_omp_agents_config_to_dir(&db, &config_id).await {
            log::warn!("Failed to re-apply updated OMP Agents config: {error}");
        } else {
            emit_omp_agents_changed(&app);
        }
    } else {
        let _ = app.emit("config-changed", "window");
    }

    get_agents_config_from_sqlite(db, &config_id)?
        .ok_or_else(|| format!("OMP Agents config '{}' not found after update", config_id))
}

#[tauri::command]
pub async fn delete_omp_agents_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.db();
    db.with_conn(|conn| db_delete(conn, DbTable::OhMyPiAgentsConfig, &id).map(|_| ()))?;
    let _ = app.emit("config-changed", "window");
    Ok(())
}

#[tauri::command]
pub async fn reorder_omp_agents_configs(
    state: tauri::State<'_, SqliteDbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.db();
    for (index, id) in ids.iter().enumerate() {
        db.with_conn(|conn| {
            db_patch_fields(
                conn,
                DbTable::OhMyPiAgentsConfig,
                id,
                &[("sort_index", json!(index as i32))],
            )
            .map(|_| ())
        })?;
    }
    let _ = db;
    Ok(())
}

#[tauri::command]
pub async fn toggle_omp_agents_config_disabled(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    config_id: String,
    is_disabled: bool,
) -> Result<(), String> {
    let db = state.db();
    let now = Local::now().to_rfc3339();
    let config_value = db
        .with_conn(|conn| {
            db_patch_fields(
                conn,
                DbTable::OhMyPiAgentsConfig,
                &config_id,
                &[
                    ("is_disabled", Value::Bool(is_disabled)),
                    ("updated_at", Value::String(now.clone())),
                ],
            )
        })?
        .ok_or_else(|| format!("OMP Agents config '{}' not found", config_id))?;
    let is_applied = config_value
        .get("is_applied")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if is_applied && is_disabled {
        // 禁用已应用方案:撤回使用(清空 agents/*.md 目录并取消 applied 标记),
        // 保留数据库方案记录。与 clear applied 共用同一内部流程,避免留下
        // 「已禁用但仍应用到运行目录」的悬挂状态。
        clear_omp_agents_applied_config_internal(&db, &app).await?;
    }
    Ok(())
}

/// 将方案的 model_roles 写入运行时 config.yml。
pub(crate) async fn apply_model_roles_to_settings(
    db: &SqliteDbState,
    model_roles: Option<&Value>,
) -> Result<(), String> {
    let config_path = get_omp_config_path_async(db).await?;
    let mut settings = read_yaml_object_or_empty(&config_path)?;
    let settings_object = object_mut(&mut settings)?;

    let mut next_roles = Map::new();
    let mut next_default_thinking: Option<String> = None;

    if let Some(roles_obj) = model_roles.and_then(Value::as_object) {
        for (role, role_val) in roles_obj {
            let (model_id, thinking_level) = match role_val {
                Value::String(s) => parse_role_string(s),
                Value::Object(map) => {
                    let m = map
                        .get("model")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    let t = map
                        .get("thinkingLevel")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(str::to_string);
                    (m, t)
                }
                _ => continue,
            };

            if model_id.is_empty() {
                continue;
            }

            let role_val_str = match &thinking_level {
                Some(level) if !level.is_empty() => format!("{model_id}:{level}"),
                _ => model_id.clone(),
            };

            next_roles.insert(role.clone(), Value::String(role_val_str));

            if role == "default" {
                next_default_thinking = thinking_level;
            }
        }
    }

    if next_roles.is_empty() {
        settings_object.remove("modelRoles");
    } else {
        settings_object.insert("modelRoles".to_string(), Value::Object(next_roles));
    }

    if let Some(level) = next_default_thinking {
        settings_object.insert("defaultThinkingLevel".to_string(), json!(level));
    }

    write_yaml_object(&config_path, &settings)?;
    Ok(())
}

/// 清空当前应用的 OMP subagent 方案:删除 `agents/*.md` 下所有可管理文件
/// 并清空 `config.yml` 中的 `modelRoles`,取消 applied 标记。保留数据库里的方案记录。
#[tauri::command]
pub async fn clear_omp_agents_applied_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    if config_id == "__local__" {
        return Err("Local config cannot be cleared; save it as a managed config first".to_string());
    }
    clear_omp_agents_applied_config_internal(state.db(), &app).await
}

async fn clear_omp_agents_applied_config_internal<R: tauri::Runtime>(
    db: &SqliteDbState,
    app: &tauri::AppHandle<R>,
) -> Result<(), String> {
    let root = omp_root_dir_async(db).await?;
    remove_managed_agent_files(&root)?;
    apply_model_roles_to_settings(db, None).await?;
    let now = Local::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        db_update_applied_status(conn, DbTable::OhMyPiAgentsConfig, None, &now)
    })?;
    emit_omp_agents_changed(app);
    Ok(())
}

/// 删除 `agents/`(及 legacy `agent/`)下所有可管理的 .md 文件——这些文件都由
/// apply 渲染产生。内置 agent 覆盖文件也属于可管理文件(恢复默认=删除覆盖)。
fn remove_managed_agent_files(root: &Path) -> Result<(), String> {
    for directory_name in AGENT_DIRECTORY_NAMES {
        let directory = root.join(directory_name);
        if !directory.is_dir() {
            continue;
        }
        let files = fs::read_dir(&directory)
            .map_err(|error| format!("Failed to read {}: {error}", directory.display()))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_ok_and(|ft| ft.is_file()))
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            })
            .collect::<Vec<_>>();
        for file in files {
            if let Err(error) = fs::remove_file(&file) {
                log::warn!("Failed to remove OMP agent file {}: {error}", file.display());
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn apply_omp_agents_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    apply_omp_agents_config_internal(state.db(), &app, &config_id, false).await
}

/// 将一个 subagent 方案渲染为 `agents/*.md` 目录并更新 `config.yml` 中的 `modelRoles`,并更新
/// applied 标记。`__local__` 直接把当前目录当作已应用(写前先清空)。
pub async fn apply_omp_agents_config_internal<R: tauri::Runtime>(
    db: &SqliteDbState,
    app: &tauri::AppHandle<R>,
    config_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    if config_id == "__local__" {
        let root = omp_root_dir_async(db).await?;
        remove_managed_agent_files(&root)?;
        apply_model_roles_to_settings(db, None).await?;
        let now = Local::now().to_rfc3339();
        db.with_conn_mut(|conn| {
            db_update_applied_status(conn, DbTable::OhMyPiAgentsConfig, None, &now)
        })?;
        let payload = if from_tray { "tray" } else { "window" };
        let _ = app.emit("config-changed", payload);
        return Ok(());
    }

    apply_omp_agents_config_to_dir(db, config_id).await?;
    let now = Local::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        db_update_applied_status(conn, DbTable::OhMyPiAgentsConfig, Some(config_id), &now)
    })?;
    let payload = if from_tray { "tray" } else { "window" };
    let _ = app.emit("config-changed", payload);
    #[cfg(target_os = "windows")]
    let _ = app.emit("wsl-sync-request-omp", ());
    Ok(())
}

/// 把指定方案的 model_roles 写入 config.yml,并将自定义 agents 映射渲染为文件(校验 + 清理 + 全量写)。
async fn apply_omp_agents_config_to_dir(
    db: &SqliteDbState,
    config_id: &str,
) -> Result<(), String> {
    let config = get_agents_config_from_sqlite(db, config_id)?
        .ok_or_else(|| format!("OMP Agents config '{}' not found", config_id))?;
    if config.is_disabled {
        return Err(format!(
            "OMP Agents config '{}' is disabled and cannot be applied",
            config_id
        ));
    }

    // 1. 核心 model_roles 写入 config.yml
    apply_model_roles_to_settings(db, config.model_roles.as_ref()).await?;

    // 2. 自定义 agents 写入 agents/*.md
    let root = omp_root_dir_async(db).await?;
    let directory = canonical_agents_dir(&root);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed to create {}: {error}", directory.display()))?;

    // 先校验整份方案的每个 agent,任一非法则整体失败(避免写到一半留下脏目录)。
    let mut rendered = Vec::<(String, String)>::new(); // (file_name, content)
    if let Some(agents) = config.agents.as_ref().and_then(Value::as_object) {
        for (name, agent_config) in agents {
            render_agent_file(name, agent_config, &mut rendered)?;
        }
    }

    remove_managed_agent_files(&root)?;
    for (file_name, content) in rendered {
        let file_path = directory.join(file_name);
        fs::write(&file_path, content)
            .map_err(|error| format!("Failed to write {}: {error}", file_path.display()))?;
    }
    Ok(())
}

/// 将单个 agent 的 frontmatter 配置渲染为 `.md` 文件内容。
fn render_agent_file(
    name: &str,
    agent_config: &Value,
    rendered: &mut Vec<(String, String)>,
) -> Result<(), String> {
    if name.trim().is_empty() || !is_valid_agent_file_name(name) {
        return Err(format!("Invalid OMP agent name: {name:?}"));
    }
    if OMP_RESERVED_AGENT_NAMES.contains(&name) {
        return Err(format!("Agent name '{name}' is reserved by OMP and cannot be used"));
    }
    let object = agent_config
        .as_object()
        .ok_or_else(|| format!("Agent '{name}' configuration must be a mapping object"))?;
    let mut frontmatter = object.clone();
    // filename 作为 name;若 frontmatter 未显式声明 name,渲染时补上
    // (OMP parseAgentFields 要求 name 必填)。
    let declared_name = frontmatter
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if declared_name.is_none() {
        frontmatter.insert("name".to_string(), json!(name));
    }
    let prompt = frontmatter
        .remove("prompt")
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();

    let yaml = serde_yaml::to_string(&serde_json::to_value(&frontmatter).unwrap_or(json!({})))
        .map_err(|error| format!("Failed to serialize agent frontmatter: {error}"))?;
    let content = format!("---\n{yaml}---\n\n{prompt}");
    rendered.push((format!("{name}.md"), content));
    Ok(())
}

fn is_valid_agent_file_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        && name != "."
        && name != ".."
}

// ============================================================================
// 低层命令:列出现有文件 + 保存/删除单个文件(编辑弹窗直接落盘,不经 DB)
// ============================================================================

/// 列出现有 user 级 agent 文件(含内置补齐占位)。供编辑弹窗在 apply 前预览
/// 当前目录;也用于方案编辑时的"已有快照"。
#[tauri::command]
pub async fn list_omp_agents(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<OmpAgent>, String> {
    let root = omp_root_dir_async(state.db()).await?;
    let mut agents = Vec::<OmpAgent>::new();

    for directory_name in AGENT_DIRECTORY_NAMES {
        let directory = root.join(directory_name);
        if !directory.is_dir() {
            continue;
        }
        let mut files = match fs::read_dir(&directory) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_ok_and(|ft| ft.is_file()))
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
                })
                .collect::<Vec<_>>(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                log::warn!(
                    "Failed to read OMP Agent directory {}: {error}",
                    directory.display()
                );
                Vec::new()
            }
        };
        files.sort();
        for file in files {
            if let Ok(agent) = read_agent_file(&root, &file) {
                agents.push(agent);
            }
        }
    }

    let existing_names: std::collections::HashSet<String> = agents
        .iter()
        .map(|agent| agent.name.clone())
        .collect();
    for builtin in OMP_BUILTIN_AGENT_NAMES {
        if existing_names.contains(builtin) {
            continue;
        }
        agents.push(OmpAgent {
            name: builtin.to_string(),
            path: String::new(),
            frontmatter: String::new(),
            prompt: String::new(),
            raw_content: String::new(),
            content_hash: String::new(),
            config: None,
            parse_error: None,
            is_builtin: true,
            is_override: false,
        });
    }

    agents.sort_by(|left, right| {
        let builtin_rank = |name: &str| {
            OMP_BUILTIN_AGENT_NAMES
                .iter()
                .position(|builtin| *builtin == name)
                .map(|index| index as i32)
                .unwrap_or(i32::MAX)
        };
        left.name
            .cmp(&right.name)
            .then_with(|| left.is_builtin.cmp(&right.is_builtin))
            .then_with(|| builtin_rank(&left.name).cmp(&builtin_rank(&right.name)))
    });

    Ok(agents)
}

/// 保存(新建/覆盖)单个 agent 文件。
#[tauri::command]
pub async fn save_omp_agent<R: tauri::Runtime>(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle<R>,
    request: SaveOmpAgentRequest,
) -> Result<OmpAgent, String> {
    let path = PathBuf::from(&request.path);
    let root = omp_root_dir_async(state.db()).await?;
    ensure_safe_agent_path(&path, &root)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let (_, _, parsed) = parse_omp_agent(&request.content)?;
    let name_from_file = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string();
    validate_omp_agent_config(&name_from_file, &parsed)?;

    let current_content = fs::read_to_string(&path).ok();
    if let Some(current_content) = current_content {
        if content_hash(&current_content) != request.expected_content_hash {
            return Err(
                "OMP Agent file changed outside AI Toolbox. Reload before saving.".to_string(),
            );
        }
    } else if !request.expected_content_hash.is_empty() {
        return Err("OMP Agent file does not exist. Reload before saving.".to_string());
    }

    fs::write(&path, &request.content)
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))?;
    emit_omp_agents_changed(&app);
    read_agent_file(&root, &path)
}

/// 删除单个 agent 文件(自定义 agent / 内置覆盖文件)。
#[tauri::command]
pub async fn delete_omp_agent<R: tauri::Runtime>(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle<R>,
    request: DeleteOmpAgentRequest,
) -> Result<(), String> {
    let path = PathBuf::from(&request.path);
    let root = omp_root_dir_async(state.db()).await?;
    ensure_safe_agent_path(&path, &root)?;

    let current_content = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    if content_hash(&current_content) != request.expected_content_hash {
        return Err(
            "OMP Agent file changed outside AI Toolbox. Reload before deleting.".to_string(),
        );
    }
    fs::remove_file(&path)
        .map_err(|error| format!("Failed to delete {}: {error}", path.display()))?;

    emit_omp_agents_changed(&app);
    Ok(())
}

/// 内置(未覆盖)agent 的展示描述。
pub fn builtin_agent_description(name: &str) -> Option<&'static str> {
    match name {
        "task" => Some(
            "General-purpose subagent with full capabilities for delegated multi-step tasks",
        ),
        "sonic" => Some(
            "Low-reasoning agent for strictly mechanical updates or data collection only",
        ),
        "scout" => Some("Read-only retrieval of external docs and dependency sources"),
        "reviewer" => Some("Review a change for concrete correctness findings"),
        "security-reviewer" => Some("Review a change for security issues"),
        _ => None,
    }
}

/// 供 tray 复用:读取当前 applied 方案 id(没有则 None)。
pub async fn get_applied_omp_agents_config_id(db: &SqliteDbState) -> Result<Option<String>, String> {
    let records =
        db.with_conn(|conn| db_query_by_bool(conn, DbTable::OhMyPiAgentsConfig, &JsonFieldPath::new("is_applied")?, true, None, Some(1)))?;
    Ok(records
        .first()
        .map(|record| crate::coding::db_id::db_extract_id(record)))
}

#[cfg(test)]
mod tests {
    use super::{
        is_valid_agent_file_name, parse_omp_agent, render_agent_file, validate_omp_agent_config,
        OMP_BUILTIN_AGENT_NAMES, OMP_RESERVED_AGENT_NAMES,
    };
    use serde_json::json;

    #[test]
    fn parses_frontmatter_and_prompt() {
        let content = "---\nname: reviewer\ndescription: Reviews code\nmodel: \"@smol\"\n---\n\nReview carefully.\n";
        let (frontmatter, prompt, config) = parse_omp_agent(content).unwrap();
        assert!(frontmatter.contains("description: Reviews code"));
        assert_eq!(prompt, "Review carefully.\n");
        assert_eq!(config["name"], "reviewer");
        assert_eq!(config["model"], "@smol");
    }

    #[test]
    fn accepts_bom_and_closing_dotdotdot() {
        let content = "\u{feff}---\nname: scout\ndescription: Read-only\n...\nbody";
        let (_, prompt, config) = parse_omp_agent(content).unwrap();
        assert_eq!(config["name"], "scout");
        assert_eq!(prompt, "body");
    }

    #[test]
    fn rejects_empty_and_non_marker_files() {
        assert!(parse_omp_agent("").is_err());
        assert!(parse_omp_agent("no frontmatter here").is_err());
    }

    #[test]
    fn requires_description_for_custom() {
        let config = json!({ "description": "ok" });
        assert!(validate_omp_agent_config("custom", &config).is_ok());
        let without_description = json!({ "name": "custom" });
        assert!(validate_omp_agent_config("custom", &without_description).is_err());
    }

    #[test]
    fn builtin_agents_skip_description_requirement() {
        let config = json!({ "name": "task" });
        assert!(validate_omp_agent_config("task", &config).is_ok());
    }

    #[test]
    fn rejects_reserved_names() {
        for reserved in OMP_RESERVED_AGENT_NAMES {
            let config = json!({ "name": reserved, "description": "x" });
            assert!(validate_omp_agent_config("whatever", &config).is_err());
        }
    }

    #[test]
    fn accepts_list_model_and_csv_tools() {
        let config = json!({
            "name": "x",
            "description": "d",
            "model": ["@smol", "openai/gpt-5-mini"],
            "tools": "read, grep",
            "spawns": "*",
            "blocking": true,
            "prewalk": "@smol",
            "advisor": true
        });
        assert!(validate_omp_agent_config("x", &config).is_ok());
    }

    #[test]
    fn rejects_invalid_field_types() {
        assert!(validate_omp_agent_config(
            "x",
            &json!({ "name": "x", "description": "d", "model": 42 })
        )
        .is_err());
        assert!(validate_omp_agent_config(
            "x",
            &json!({ "name": "x", "description": "d", "blocking": "yes" })
        )
        .is_err());
        assert!(validate_omp_agent_config(
            "x",
            &json!({ "name": "x", "description": "d", "thinkingLevel": 3 })
        )
        .is_err());
        assert!(validate_omp_agent_config(
            "x",
            &json!({ "name": "x", "description": "d", "autoloadSkills": [1, 2] })
        )
        .is_err());
    }

    #[test]
    fn render_agent_adds_name_and_prompt() {
        let mut rendered = Vec::new();
        render_agent_file(
            "custom",
            &json!({ "description": "d", "model": "@smol", "prompt": "Do work" }),
            &mut rendered,
        )
        .unwrap();
        assert_eq!(rendered.len(), 1);
        let (file_name, content) = &rendered[0];
        assert_eq!(file_name, "custom.md");
        let (_, prompt, config) = parse_omp_agent(content).unwrap();
        assert_eq!(config["name"], "custom");
        assert_eq!(prompt, "Do work");
    }

    #[test]
    fn render_rejects_invalid_names() {
        let mut rendered = Vec::new();
        assert!(render_agent_file("../evil", &json!({}), &mut rendered).is_err());
        assert!(render_agent_file("a/b", &json!({}), &mut rendered).is_err());
    }

    #[test]
    fn valid_file_names_pass() {
        assert!(is_valid_agent_file_name("reviewer"));
        assert!(is_valid_agent_file_name("security-reviewer"));
        assert!(is_valid_agent_file_name("my_agent"));
        assert!(!is_valid_agent_file_name("../evil"));
        assert!(!is_valid_agent_file_name("a/b"));
        assert!(!is_valid_agent_file_name(""));
    }

    #[test]
    fn builtin_list_covers_known_bundled_agents() {
        for name in ["task", "sonic", "scout", "reviewer", "security-reviewer"] {
            assert!(OMP_BUILTIN_AGENT_NAMES.contains(&name));
        }
    }

    #[test]
    fn parses_role_strings_with_and_without_thinking_suffix() {
        use super::parse_role_string;
        assert_eq!(
            parse_role_string("anthropic/claude-sonnet-4-6:high"),
            ("anthropic/claude-sonnet-4-6".to_string(), Some("high".to_string()))
        );
        assert_eq!(
            parse_role_string("openai/gpt-5-turbo"),
            ("openai/gpt-5-turbo".to_string(), None)
        );
        assert_eq!(
            parse_role_string("@task:auto"),
            ("@task".to_string(), Some("auto".to_string()))
        );
    }
}