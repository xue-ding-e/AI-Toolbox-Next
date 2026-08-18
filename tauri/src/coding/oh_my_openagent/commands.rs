use chrono::Local;
use serde_json::Value;
use std::fs;
use std::path::Path;

use super::adapter;
use super::types::*;
use crate::coding::runtime_location;
use crate::db::helpers::{
    db_create, db_delete, db_get, db_list, db_patch_fields, db_put, db_query_by_bool,
    db_update_applied_status,
};
use crate::db::schema::{DbTable, JsonFieldPath};
use crate::db::SqliteDbState;
use tauri::Emitter;

pub const OH_MY_OPENAGENT_CONFIG_TABLE: &str = "oh_my_openagent_config";
pub const OH_MY_OPENAGENT_GLOBAL_CONFIG_TABLE: &str = "oh_my_openagent_global_config";

const OMO_LEGACY_SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/code-yeongyu/oh-my-openagent/dev/assets/oh-my-opencode.schema.json";
const OMO_UNIFIED_SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/code-yeongyu/oh-my-openagent/dev/assets/omo.schema.json";
/// Matches the upstream opencode-config unification migration id so the plugin's
/// startup migration does not re-import a stale legacy file over our values.
const OMO_UNIFIED_MIGRATION_ID: &str = "2026-07-opencode-config-unification";

/// Keys that are NOT part of the upstream `[opencode]` plugin schema
/// (`OhMyOpenCodeConfigSchema` has no catchall → any unknown key triggers a
/// doctor "Unknown config key" diagnostic). These can leak into the `[opencode]`
/// block through `other_fields` flattening or legacy file migration, so we strip
/// them in unified mode before writing. `lsp` is a known schema-eviction (still
/// legal in legacy mode, written there); `google_auth` is a stray key some legacy
/// configs carried. Add future evictions here.
const OMO_UNIFIED_UNKNOWN_KEYS: &[&str] = &["google_auth", "lsp"];

fn default_global_config() -> OhMyOpenAgentGlobalConfig {
    OhMyOpenAgentGlobalConfig {
        id: "global".to_string(),
        schema: None,
        sisyphus_agent: None,
        disabled_agents: None,
        disabled_mcps: None,
        disabled_hooks: None,
        disabled_skills: None,
        lsp: None,
        experimental: None,
        background_task: None,
        browser_automation_engine: None,
        claude_code: None,
        other_fields: None,
        updated_at: None,
    }
}

fn build_local_profile_content(
    config_input: Option<OhMyOpenAgentAgentsProfileInput>,
    base_config: OhMyOpenAgentAgentsProfile,
    now: &str,
) -> OhMyOpenAgentAgentsProfileContent {
    let (name, agents, categories, other_fields) = if let Some(config) = config_input {
        (
            config.name,
            config.agents,
            config.categories,
            config.other_fields,
        )
    } else {
        (
            base_config.name,
            base_config.agents,
            base_config.categories,
            base_config.other_fields,
        )
    };

    OhMyOpenAgentAgentsProfileContent {
        name,
        is_applied: true,
        is_disabled: false,
        agents,
        categories,
        other_fields,
        sort_index: None,
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

fn build_local_global_content(
    global_input: Option<OhMyOpenAgentGlobalConfigInput>,
    base_global: Option<OhMyOpenAgentGlobalConfig>,
    now: &str,
) -> OhMyOpenAgentGlobalConfigContent {
    let (
        schema,
        sisyphus_agent,
        disabled_agents,
        disabled_mcps,
        disabled_hooks,
        disabled_skills,
        lsp,
        experimental,
        background_task,
        browser_automation_engine,
        claude_code,
        other_fields,
    ) = if let Some(global) = global_input {
        (
            global.schema,
            global.sisyphus_agent,
            global.disabled_agents,
            global.disabled_mcps,
            global.disabled_hooks,
            global.disabled_skills,
            global.lsp,
            global.experimental,
            global.background_task,
            global.browser_automation_engine,
            global.claude_code,
            global.other_fields,
        )
    } else if let Some(global) = base_global {
        (
            global.schema,
            global.sisyphus_agent,
            global.disabled_agents,
            global.disabled_mcps,
            global.disabled_hooks,
            global.disabled_skills,
            global.lsp,
            global.experimental,
            global.background_task,
            global.browser_automation_engine,
            global.claude_code,
            global.other_fields,
        )
    } else {
        (
            None, None, None, None, None, None, None, None, None, None, None, None,
        )
    };

    OhMyOpenAgentGlobalConfigContent {
        schema,
        sisyphus_agent,
        disabled_agents,
        disabled_mcps,
        disabled_hooks,
        disabled_skills,
        lsp,
        experimental,
        background_task,
        browser_automation_engine,
        claude_code,
        other_fields,
        updated_at: now.to_string(),
    }
}

fn list_configs_from_sqlite(
    sqlite_state: &SqliteDbState,
) -> Result<Vec<OhMyOpenAgentAgentsProfile>, String> {
    let mut configs = sqlite_state.with_conn(|conn| {
        db_list(conn, DbTable::OhMyOpenAgentConfig, None).map(|records| {
            records
                .into_iter()
                .map(adapter::from_db_value)
                .collect::<Vec<_>>()
        })
    })?;
    configs.sort_by(|a, b| match (a.sort_index, b.sort_index) {
        (Some(ai), Some(bi)) => ai.cmp(&bi),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.name.cmp(&b.name),
    });
    Ok(configs)
}

fn get_config_from_sqlite(
    sqlite_state: &SqliteDbState,
    config_id: &str,
) -> Result<Option<OhMyOpenAgentAgentsProfile>, String> {
    sqlite_state.with_conn(|conn| {
        db_get(conn, DbTable::OhMyOpenAgentConfig, config_id)
            .map(|record| record.map(adapter::from_db_value))
    })
}

fn get_global_config_from_sqlite(
    sqlite_state: &SqliteDbState,
) -> Result<Option<OhMyOpenAgentGlobalConfig>, String> {
    sqlite_state.with_conn(|conn| {
        db_get(conn, DbTable::OhMyOpenAgentGlobalConfig, "global")
            .map(|record| record.map(adapter::global_config_from_db_value))
    })
}

fn put_config_to_sqlite(
    sqlite_state: &SqliteDbState,
    config_id: &str,
    data: &Value,
) -> Result<(), String> {
    sqlite_state.with_conn(|conn| db_put(conn, DbTable::OhMyOpenAgentConfig, config_id, data))
}

fn put_global_config_to_sqlite(sqlite_state: &SqliteDbState, data: &Value) -> Result<(), String> {
    sqlite_state.with_conn(|conn| db_put(conn, DbTable::OhMyOpenAgentGlobalConfig, "global", data))
}

/// Normalize agent key to lowercase for backward compatibility
fn normalize_agent_key(key: &str) -> String {
    key.to_lowercase()
}

/// Normalize all keys in an agents object to lowercase
fn normalize_agents_keys(agents: &mut Value) {
    if let Some(obj) = agents.as_object_mut() {
        let keys: Vec<String> = obj.keys().cloned().collect();
        for key in keys {
            let normalized = normalize_agent_key(&key);
            if normalized != key {
                if let Some(value) = obj.remove(&key) {
                    if obj.contains_key(&normalized) {
                        log::warn!(
                            "[OhMyOpenAgent] Agent key conflict: '{}' normalized to '{}' which already exists, overwriting",
                            key,
                            normalized
                        );
                    }
                    obj.insert(normalized, value);
                }
            }
        }
    }
}

/// Flatten a unified `~/.omo/omo.jsonc` document (or a legacy flat file) into the
/// single flat view the extraction logic expects. omo-config-core control keys
/// (`profiles`, `_migrations`, `legacy_migrations`, `models`, `task`, `teams`,
/// `codegraph`, `$schema`) are not plugin config and are always stripped — even when
/// there is no `[opencode]` block yet — so they never leak into `other_fields` and
/// then get written back into `[opencode]` (which would trigger upstream Unknown key
/// diagnostics). When a `[opencode]` block exists, its keys overlay the base
/// (block wins, mirroring upstream resolution).
fn flatten_omo_config(json: &Value) -> Value {
    let Some(obj) = json.as_object() else {
        return json.clone();
    };

    let mut flattened = serde_json::Map::new();
    for (key, value) in obj {
        if matches!(
            key.as_str(),
            "[opencode]"
                | "profiles"
                | "_migrations"
                | "legacy_migrations"
                | "$schema"
                | "models"
                | "task"
                | "teams"
                | "codegraph"
        ) {
            continue;
        }
        flattened.insert(key.clone(), value.clone());
    }
    if let Some(block) = obj.get("[opencode]").and_then(Value::as_object) {
        for (key, value) in block {
            flattened.insert(key.clone(), value.clone());
        }
    }
    Value::Object(flattened)
}

/// Strip keys that are not part of the upstream `[opencode]` plugin schema from a
/// merged plugin config before writing it into the unified `omo.jsonc`. These keys
/// leak in via `other_fields` flattening or legacy file migration and would trigger
/// upstream doctor "Unknown config key" diagnostics. No-op when the value is not an
/// object. See [`OMO_UNIFIED_UNKNOWN_KEYS`].
fn strip_unified_unknown_keys(plugin_config: &mut Value) {
    let Some(obj) = plugin_config.as_object_mut() else {
        return;
    };
    for key in OMO_UNIFIED_UNKNOWN_KEYS {
        if obj.remove(*key).is_some() {
            log::warn!(
                "[OhMyOpenAgent] Stripped unknown `[opencode]` key '{}' (not in upstream plugin schema)",
                key
            );
        }
    }
}

fn get_default_oh_my_openagent_dir() -> Result<std::path::PathBuf, String> {
    let home_dir = dirs::home_dir().ok_or("Failed to get home directory")?;
    Ok(home_dir.join(".config").join("opencode"))
}

pub(crate) fn get_default_oh_my_openagent_path_candidates(
) -> Result<Vec<std::path::PathBuf>, String> {
    let default_dir = get_default_oh_my_openagent_dir()?;
    Ok(vec![
        default_dir.join("oh-my-openagent.jsonc"),
        default_dir.join("oh-my-openagent.json"),
        default_dir.join("oh-my-opencode.jsonc"),
        default_dir.join("oh-my-opencode.json"),
    ])
}

async fn get_oh_my_openagent_config_path_and_source(
    db: &crate::db::SqliteDbState,
) -> Result<(std::path::PathBuf, &'static str), String> {
    let path = runtime_location::get_omo_config_path_async(db).await?;
    let source = if !runtime_location::uses_legacy_omo_config_async(db).await {
        "unified"
    } else {
        let default_candidates = get_default_oh_my_openagent_path_candidates()?;
        if default_candidates
            .iter()
            .any(|candidate| candidate == &path)
        {
            "default"
        } else {
            "custom"
        }
    };
    Ok((path, source))
}

// ============================================================================
// Oh My OpenAgent Config Commands
// ============================================================================

/// List all Oh My OpenAgent configs ordered by name
#[tauri::command]
pub async fn list_oh_my_openagent_configs(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<OhMyOpenAgentAgentsProfile>, String> {
    let db = state.db();

    let configs = list_configs_from_sqlite(db)?;
    if configs.is_empty() {
        if let Ok(temp_config) = load_temp_config_from_file(db).await {
            return Ok(vec![temp_config]);
        }
    }
    Ok(configs)
}

/// Resolve the Oh My OpenAgent config path with legacy filename compatibility.
/// Priority: canonical .jsonc/.json -> legacy .jsonc/.json -> canonical .jsonc default
pub async fn get_oh_my_openagent_config_path(
    db: &crate::db::SqliteDbState,
) -> Result<std::path::PathBuf, String> {
    let (config_path, _) = get_oh_my_openagent_config_path_and_source(db).await?;
    Ok(config_path)
}

/// Load a temporary config from local file without writing to database
/// This is used when the database is empty and we want to show the local config
/// Returns a config with id "__local__" to indicate it's from local file
async fn load_temp_config_from_file(
    db: &crate::db::SqliteDbState,
) -> Result<OhMyOpenAgentAgentsProfile, String> {
    let config_path = get_oh_my_openagent_config_path(db)
        .await
        .map_err(|_| "Local config file not found".to_string())?;

    if !config_path.exists() {
        return Err("No config file found".to_string());
    }

    // 读取文件内容
    let file_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read local config file: {}", e))?;

    // 解析 JSON（使用 json5 支持带注释的 JSONC 格式）
    let json_value: Value = json5::from_str(&file_content)
        .map_err(|e| format!("Failed to parse local config file: {}", e))?;
    // 兼容 unified ~/.omo/omo.jsonc:把 [opencode] 块 + base 摊平成扁平视图
    let json_value = flatten_omo_config(&json_value);

    // 提取 agents 配置（并标准化键名为小写）
    let mut agents = json_value
        .get("agents")
        .and_then(|v| serde_json::from_value::<Value>(v.clone()).ok());
    if let Some(ref mut agents_value) = agents {
        normalize_agents_keys(agents_value);
    }

    let categories = json_value
        .get("categories")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    // 提取 other_fields（除了 agents 和全局配置字段之外的所有字段）
    let mut other_fields = json_value.clone();
    if let Some(obj) = other_fields.as_object_mut() {
        obj.remove("agents");
        obj.remove("categories");
        obj.remove("$schema");
        // 移除属于 Global Config 的字段
        obj.remove("sisyphus_agent");
        obj.remove("sisyphusAgent");
        obj.remove("disabled_agents");
        obj.remove("disabledAgents");
        obj.remove("disabled_mcps");
        obj.remove("disabledMcps");
        obj.remove("disabled_hooks");
        obj.remove("disabledHooks");
        obj.remove("disabled_skills");
        obj.remove("disabledSkills");
        obj.remove("lsp");
        obj.remove("experimental");
        obj.remove("background_task");
        obj.remove("backgroundTask");
        obj.remove("browser_automation_engine");
        obj.remove("browserAutomationEngine");
        obj.remove("claude_code");
        obj.remove("claudeCode");
    }

    let other_fields_value = if other_fields
        .as_object()
        .map(|o| o.is_empty())
        .unwrap_or(true)
    {
        None
    } else {
        Some(other_fields)
    };

    let now = Local::now().to_rfc3339();
    Ok(OhMyOpenAgentAgentsProfile {
        id: "__local__".to_string(), // Special ID to indicate this is from local file
        name: "default".to_string(),
        is_applied: true,
        is_disabled: false,
        agents,
        categories,
        other_fields: other_fields_value,
        sort_index: None,
        created_at: Some(now.clone()),
        updated_at: Some(now),
    })
}

/// Load a temporary global config from local file without writing to database
/// Returns a config with id "__local__" to indicate it's from local file
async fn load_temp_global_config_from_file(
    db: &crate::db::SqliteDbState,
) -> Result<OhMyOpenAgentGlobalConfig, String> {
    let config_path = get_oh_my_openagent_config_path(db)
        .await
        .map_err(|_| "Local config file not found".to_string())?;

    if !config_path.exists() {
        return Err("No config file found".to_string());
    }

    let file_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read local config file: {}", e))?;

    let json_value: Value = json5::from_str(&file_content)
        .map_err(|e| format!("Failed to parse local config file: {}", e))?;
    // 兼容 unified ~/.omo/omo.jsonc:把 [opencode] 块 + base 摊平成扁平视图
    let json_value = flatten_omo_config(&json_value);

    // 提取全局配置字段
    let schema = json_value
        .get("$schema")
        .and_then(|v| v.as_str())
        .map(String::from);

    let sisyphus_agent = json_value
        .get("sisyphus_agent")
        .or_else(|| json_value.get("sisyphusAgent"))
        .cloned();

    let disabled_agents: Option<Vec<String>> = json_value
        .get("disabled_agents")
        .or_else(|| json_value.get("disabledAgents"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let disabled_mcps: Option<Vec<String>> = json_value
        .get("disabled_mcps")
        .or_else(|| json_value.get("disabledMcps"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let disabled_hooks: Option<Vec<String>> = json_value
        .get("disabled_hooks")
        .or_else(|| json_value.get("disabledHooks"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let disabled_skills: Option<Vec<String>> = json_value
        .get("disabled_skills")
        .or_else(|| json_value.get("disabledSkills"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let lsp = json_value.get("lsp").cloned();
    let experimental = json_value.get("experimental").cloned();
    let background_task = json_value
        .get("background_task")
        .or_else(|| json_value.get("backgroundTask"))
        .cloned();
    let browser_automation_engine = json_value
        .get("browser_automation_engine")
        .or_else(|| json_value.get("browserAutomationEngine"))
        .cloned();
    let claude_code = json_value
        .get("claude_code")
        .or_else(|| json_value.get("claudeCode"))
        .cloned();

    // 提取 other_fields（除了已知字段之外的所有字段）
    let mut other_fields = json_value.clone();
    if let Some(obj) = other_fields.as_object_mut() {
        obj.remove("agents");
        obj.remove("categories");
        obj.remove("$schema");
        obj.remove("sisyphus_agent");
        obj.remove("sisyphusAgent");
        obj.remove("disabled_agents");
        obj.remove("disabledAgents");
        obj.remove("disabled_mcps");
        obj.remove("disabledMcps");
        obj.remove("disabled_hooks");
        obj.remove("disabledHooks");
        obj.remove("disabled_skills");
        obj.remove("disabledSkills");
        obj.remove("lsp");
        obj.remove("experimental");
        obj.remove("background_task");
        obj.remove("backgroundTask");
        obj.remove("browser_automation_engine");
        obj.remove("browserAutomationEngine");
        obj.remove("claude_code");
        obj.remove("claudeCode");
    }

    let other_fields_value = if other_fields
        .as_object()
        .map(|o| o.is_empty())
        .unwrap_or(true)
    {
        None
    } else {
        Some(other_fields)
    };

    let now = Local::now().to_rfc3339();
    Ok(OhMyOpenAgentGlobalConfig {
        id: "__local__".to_string(), // Special ID to indicate this is from local file
        schema,
        sisyphus_agent,
        disabled_agents,
        disabled_mcps,
        disabled_hooks,
        disabled_skills,
        lsp,
        experimental,
        background_task,
        browser_automation_engine,
        claude_code,
        other_fields: other_fields_value,
        updated_at: Some(now),
    })
}

/// Create a new Oh My OpenAgent config
#[tauri::command]
pub async fn create_oh_my_openagent_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OhMyOpenAgentAgentsProfileInput,
) -> Result<OhMyOpenAgentAgentsProfile, String> {
    let db = state.db();

    let now = Local::now().to_rfc3339();
    let content = OhMyOpenAgentAgentsProfileContent {
        name: input.name.clone(),
        is_applied: false,
        is_disabled: false,
        agents: input.agents.clone(),
        categories: input.categories.clone(),
        other_fields: input.other_fields.clone(),
        sort_index: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    let json_data = adapter::to_db_value(&content);

    let created = db.with_conn(|conn| db_create(conn, DbTable::OhMyOpenAgentConfig, &json_data))?;
    let _ = app.emit("config-changed", "window");
    Ok(adapter::from_db_value(created))
}

/// Update an existing Oh My OpenAgent config
#[tauri::command]
#[allow(unused_variables)] // app 在 Windows 平台上用于 WSL 同步
pub async fn update_oh_my_openagent_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OhMyOpenAgentAgentsProfileInput,
) -> Result<OhMyOpenAgentAgentsProfile, String> {
    let db = state.db();
    let config_id = input
        .id
        .ok_or_else(|| "ID is required for update".to_string())?;

    let existing_config = get_config_from_sqlite(db, &config_id)?
        .ok_or_else(|| format!("Oh My OpenAgent config with ID '{}' not found", config_id))?;
    let now = Local::now().to_rfc3339();
    let created_at = existing_config
        .created_at
        .clone()
        .unwrap_or_else(|| Local::now().to_rfc3339());

    let content = OhMyOpenAgentAgentsProfileContent {
        name: input.name,
        is_applied: existing_config.is_applied,
        is_disabled: existing_config.is_disabled,
        agents: input.agents,
        categories: input.categories,
        other_fields: input.other_fields,
        sort_index: existing_config.sort_index,
        created_at,
        updated_at: now,
    };

    let json_data = adapter::to_db_value(&content);
    put_config_to_sqlite(db, &config_id, &json_data)?;

    if existing_config.is_applied {
        if let Err(e) = apply_config_to_file(&db, &config_id).await {
            eprintln!("Failed to auto-apply updated config: {}", e);
        } else {
            #[cfg(target_os = "windows")]
            let _ = app.emit("wsl-sync-request-opencode", ());
        }
    }

    Ok(OhMyOpenAgentAgentsProfile {
        id: config_id,
        name: content.name,
        is_applied: existing_config.is_applied,
        is_disabled: content.is_disabled,
        agents: content.agents,
        categories: content.categories,
        other_fields: content.other_fields,
        sort_index: content.sort_index,
        created_at: Some(content.created_at),
        updated_at: Some(content.updated_at),
    })
}

/// Delete an Oh My OpenAgent config
#[tauri::command]
pub async fn delete_oh_my_openagent_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.db();
    db.with_conn(|conn| db_delete(conn, DbTable::OhMyOpenAgentConfig, &id).map(|_| ()))?;
    let _ = app.emit("config-changed", "window");
    Ok(())
}

/// Clear the currently applied runtime config file without deleting the saved profile.
#[tauri::command]
pub async fn clear_oh_my_openagent_applied_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    if config_id == "__local__" {
        return Err("Local config must be saved before clearing the runtime file".to_string());
    }

    let db = state.db();

    let config = get_config_from_sqlite(db, &config_id)?
        .ok_or_else(|| format!("Config '{}' not found", config_id))?;
    if !config.is_applied {
        return Err(format!("Config '{}' is not currently applied", config_id));
    }

    let config_path = get_oh_my_openagent_config_path(&db).await?;

    if runtime_location::uses_legacy_omo_config_async(&db).await {
        // legacy 模式：目标文件是 AI Toolbox 全权管理的扁平文件，删除本机文件前先删 WSL 目标。
        #[cfg(target_os = "windows")]
        crate::coding::wsl::remove_auto_synced_wsl_mapping_target(state.inner(), "opencode-oh-my")
            .await?;
        if config_path.exists() {
            fs::remove_file(&config_path)
                .map_err(|e| format!("Failed to remove config file: {}", e))?;
        }
    } else {
        // Unified 模式下 omo.jsonc 是共享配置文件，只移除 [opencode] 块，
        // 保留 codegraph/models/task/teams/profiles 等共享键；绝不整删 WSL 侧共享文件
        // （WSL 侧由后续 wsl-sync-request-opencode 同步去除该块）。
        remove_opencode_block(&config_path)?;
    }

    let now = Local::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        db_update_applied_status(conn, DbTable::OhMyOpenAgentConfig, None, &now)
    })?;

    let _ = app.emit("config-changed", "window");

    #[cfg(target_os = "windows")]
    let _ = app.emit("wsl-sync-request-opencode", ());

    Ok(())
}

/// 内部函数：将指定配置应用到配置文件（不改变数据库中的 is_applied 状态）
async fn apply_config_to_file(
    db: &crate::db::SqliteDbState,
    config_id: &str,
) -> Result<(), String> {
    apply_config_to_file_public(db, config_id).await
}

/// Public version of apply_config_to_file for tray module
pub async fn apply_config_to_file_public(
    db: &crate::db::SqliteDbState,
    config_id: &str,
) -> Result<(), String> {
    // Get the config from database using direct ID format (like ClaudeCode)
    let agents_profile = get_config_from_sqlite(db, config_id)?
        .ok_or_else(|| format!("Config '{}' not found", config_id))?;

    // Check if config is disabled (P0-3 fix: Architect solution C)
    if agents_profile.is_disabled {
        return Err(format!(
            "Config '{}' is disabled and cannot be applied",
            config_id
        ));
    }

    // Get config path using unified function
    let config_path = get_oh_my_openagent_config_path(db).await?;

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create opencode config directory: {}", e))?;
        }
    }

    // 获取 Global Config
    let global_config = get_global_config_from_sqlite(db)?.unwrap_or_else(default_global_config);

    // 合并配置的优先级顺序（从低到高）：
    // 1. 全局配置的明确字段（最低优先级）
    // 2. 全局配置的 other_fields
    // 3. Agents Profile 的 agents
    // 4. Agents Profile 的 categories
    // 5. Agents Profile 的 other_fields（最高优先级，可以覆盖所有）

    let legacy_mode = runtime_location::uses_legacy_omo_config_async(db).await;

    let mut plugin_config = serde_json::Map::new();

    // 1. 先设置全局配置的明确字段（优先级最低）。unified 模式剔除 lsp：
    //    lsp 已不是插件合法 schema 键（上游迁移会丢弃），写入会触发 Unknown key 诊断。
    if let Some(sisyphus) = global_config.sisyphus_agent {
        plugin_config.insert("sisyphus_agent".to_string(), sisyphus);
    }
    if let Some(disabled_agents) = global_config.disabled_agents {
        plugin_config.insert(
            "disabled_agents".to_string(),
            serde_json::json!(disabled_agents),
        );
    }
    if let Some(disabled_mcps) = global_config.disabled_mcps {
        plugin_config.insert(
            "disabled_mcps".to_string(),
            serde_json::json!(disabled_mcps),
        );
    }
    if let Some(disabled_hooks) = global_config.disabled_hooks {
        plugin_config.insert(
            "disabled_hooks".to_string(),
            serde_json::json!(disabled_hooks),
        );
    }
    if let Some(disabled_skills) = global_config.disabled_skills {
        plugin_config.insert(
            "disabled_skills".to_string(),
            serde_json::json!(disabled_skills),
        );
    }
    if legacy_mode {
        if let Some(lsp) = global_config.lsp {
            plugin_config.insert("lsp".to_string(), lsp);
        }
    }
    if let Some(experimental) = global_config.experimental {
        plugin_config.insert("experimental".to_string(), experimental);
    }
    if let Some(background_task) = global_config.background_task {
        plugin_config.insert("background_task".to_string(), background_task);
    }
    if let Some(browser_automation_engine) = global_config.browser_automation_engine {
        plugin_config.insert(
            "browser_automation_engine".to_string(),
            browser_automation_engine,
        );
    }
    if let Some(claude_code) = global_config.claude_code {
        plugin_config.insert("claude_code".to_string(), claude_code);
    }

    // 2. 然后平铺全局配置的 other_fields（会覆盖上面的明确字段）
    if let Some(global_others) = global_config.other_fields {
        if let Some(others_obj) = global_others.as_object() {
            for (key, value) in others_obj {
                plugin_config.insert(key.clone(), value.clone());
            }
        }
    }

    // 3. 设置 Agents Profile 的 agents（会覆盖前面的 agents，并标准化键名为小写）
    if let Some(mut agents) = agents_profile.agents {
        normalize_agents_keys(&mut agents);
        plugin_config.insert("agents".to_string(), agents);
    }

    // 4. 设置 Agents Profile 的 categories（会覆盖前面的 categories）
    if let Some(categories) = agents_profile.categories {
        plugin_config.insert("categories".to_string(), categories);
    }

    // 5. 最后平铺 Agents Profile 的 other_fields（最高优先级，可以覆盖所有字段）
    if let Some(profile_others) = agents_profile.other_fields {
        if let Some(others_obj) = profile_others.as_object() {
            for (key, value) in others_obj {
                plugin_config.insert(key.clone(), value.clone());
            }
        }
    }

    let mut plugin_config = Value::Object(plugin_config);

    // Unified 模式下剔除 [opencode] 块的非 schema 键（如 google_auth、lsp）。
    // 这些键会通过 other_fields 平铺或 legacy 迁移泄漏进来，触发上游 doctor
    // "Unknown config key" 诊断。legacy 模式下 lsp 仍是合法顶层键，故不过滤。
    if !legacy_mode {
        strip_unified_unknown_keys(&mut plugin_config);
    }

    // 清理空值：删除空对象和空数组
    adapter::clean_empty_values(&mut plugin_config);

    if legacy_mode {
        // Legacy 扁平格式：整体覆写 ~/.config/opencode/oh-my-openagent.jsonc
        let schema_url = global_config
            .schema
            .unwrap_or_else(|| OMO_LEGACY_SCHEMA_URL.to_string());
        let mut final_map = plugin_config.as_object().cloned().unwrap_or_default();
        final_map.insert("$schema".to_string(), serde_json::json!(schema_url));
        write_config_file(&config_path, &Value::Object(final_map))?;
    } else {
        // Unified 格式：只替换 [opencode] 块，保留 codegraph/models/task/teams/profiles 等共享键
        write_unified_omo_config(&config_path, &plugin_config)?;
    }

    Ok(())
}

/// Write a JSON document with pretty formatting to the given path.
fn write_config_file(config_path: &Path, value: &Value) -> Result<(), String> {
    let json_content = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize final config: {}", e))?;
    fs::write(config_path, json_content).map_err(|e| format!("Failed to write config file: {}", e))
}

fn migrate_legacy_local_configs_to_unified(
    legacy_candidates: &[std::path::PathBuf],
    unified_config_path: &Path,
) -> Result<bool, String> {
    let existing: Vec<std::path::PathBuf> = legacy_candidates
        .iter()
        .filter(|path| path.exists())
        .cloned()
        .collect();
    if existing.is_empty() {
        return Ok(false);
    }

    let mut merged = serde_json::Map::new();
    for path in &existing {
        let file_content = fs::read_to_string(path).map_err(|e| {
            format!(
                "Failed to read legacy Oh My OpenAgent config {}: {}",
                path.to_string_lossy(),
                e
            )
        })?;
        let json_value: Value = json5::from_str(&file_content).map_err(|e| {
            format!(
                "Failed to parse legacy Oh My OpenAgent config {}: {}",
                path.to_string_lossy(),
                e
            )
        })?;
        let flattened = flatten_omo_config(&json_value);
        let Some(object) = flattened.as_object() else {
            return Err(format!(
                "Legacy Oh My OpenAgent config {} must be a JSON object",
                path.to_string_lossy()
            ));
        };
        for (key, value) in object {
            if !merged.contains_key(key) {
                merged.insert(key.clone(), value.clone());
            }
        }
    }

    if let Some(parent) = unified_config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create unified OMO config directory {}: {}",
                parent.to_string_lossy(),
                e
            )
        })?;
    }

    write_unified_omo_config(unified_config_path, &Value::Object(merged))?;

    for path in existing {
        fs::remove_file(&path).map_err(|e| {
            format!(
                "Failed to remove migrated legacy Oh My OpenAgent config {}: {}",
                path.to_string_lossy(),
                e
            )
        })?;
    }

    Ok(true)
}

pub(crate) fn migrate_default_legacy_local_configs_to_unified(
    unified_config_path: &Path,
) -> Result<bool, String> {
    let candidates = get_default_oh_my_openagent_path_candidates()?;
    migrate_legacy_local_configs_to_unified(&candidates, unified_config_path)
}

/// Write the merged plugin config into the `[opencode]` block of `~/.omo/omo.jsonc`,
/// preserving every other top-level key (codegraph/models/task/teams/profiles...).
/// Also stamps the unification migration marker so the upstream startup migration
/// does not re-import a stale legacy file over our values.
/// Write the merged plugin config into the `[opencode]` block of `~/.omo/omo.jsonc`,
/// preserving every other top-level key (codegraph/models/task/teams/profiles...) and
/// all comments verbatim (JSONC-aware in-place patch). Also stamps the unification
/// migration marker so the upstream startup migration does not re-import a stale
/// legacy file over our values. A missing/invalid file is rewritten fresh.
fn write_unified_omo_config(config_path: &Path, plugin_config: &Value) -> Result<(), String> {
    let block_json = serde_json::to_string_pretty(plugin_config)
        .map_err(|e| format!("Failed to serialize [opencode] block: {}", e))?;

    if config_path.exists() {
        let content = fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read existing omo.jsonc: {}", e))?;
        if matches!(json5::from_str::<Value>(&content), Ok(Value::Object(_))) {
            let patched = patch_unified_omo_config_text(
                &content,
                &block_json,
                OMO_UNIFIED_SCHEMA_URL,
                OMO_UNIFIED_MIGRATION_ID,
            );
            fs::write(config_path, patched)
                .map_err(|e| format!("Failed to write config file: {}", e))?;
            return Ok(());
        }
    }

    let top = serde_json::json!({
        "$schema": OMO_UNIFIED_SCHEMA_URL,
        "[opencode]": plugin_config,
        "_migrations": [OMO_UNIFIED_MIGRATION_ID],
    });
    write_config_file(config_path, &top)
}

/// JSONC top-level key span for in-place patching.
struct TopLevelKey {
    name: String,
    key_start: usize,   // index of the opening quote of the key
    value_start: usize, // index of the first char of the value
    value_end: usize,   // index pointing at the trailing comma (or root brace) after the value
}

/// Scan the root-level keys of a JSONC document, honoring strings and `//` / `/* */`
/// comments. Returns the keys and the index of the root object's closing brace.
fn scan_top_level_keys(text: &str) -> (Vec<TopLevelKey>, usize) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut keys = Vec::new();
    let mut root_close = n;
    let mut i = 0usize;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line = false;
    let mut in_block = false;

    while i < n {
        let c = bytes[i];
        if in_line {
            if c == b'\n' {
                in_line = false;
            }
            i += 1;
            continue;
        }
        if in_block {
            if c == b'*' && bytes.get(i + 1) == Some(&b'/') {
                in_block = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                in_line = true;
                i += 2;
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                in_block = true;
                i += 2;
                continue;
            }
            b'"' => {
                let str_start = i;
                i += 1;
                let mut str_end = i;
                let mut str_in = true;
                let mut str_esc = false;
                while str_end < n {
                    let sc = bytes[str_end];
                    if str_esc {
                        str_esc = false;
                    } else if sc == b'\\' {
                        str_esc = true;
                    } else if sc == b'"' {
                        str_in = false;
                        str_end += 1;
                        break;
                    }
                    str_end += 1;
                }
                if str_in {
                    i = str_end;
                    continue;
                }
                let name = &text[str_start + 1..str_end - 1];
                // Skip whitespace and comments to see if this string is a key (':' follows).
                let mut j = str_end;
                loop {
                    while j < n && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j + 1 < n && bytes[j] == b'/' && bytes[j + 1] == b'/' {
                        while j < n && bytes[j] != b'\n' {
                            j += 1;
                        }
                        continue;
                    }
                    if j + 1 < n && bytes[j] == b'/' && bytes[j + 1] == b'*' {
                        j += 2;
                        while j + 1 < n && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                            j += 1;
                        }
                        j += 2;
                        continue;
                    }
                    break;
                }
                if j < n && bytes[j] == b':' && depth == 1 {
                    let mut vs = j + 1;
                    while vs < n && bytes[vs].is_ascii_whitespace() {
                        vs += 1;
                    }
                    let ve = scan_value_end(text, vs);
                    keys.push(TopLevelKey {
                        name: name.to_string(),
                        key_start: str_start,
                        value_start: vs,
                        value_end: ve,
                    });
                    i = ve;
                    continue;
                }
                i = str_end;
                continue;
            }
            b'{' => {
                depth += 1;
                i += 1;
                continue;
            }
            b'}' => {
                if depth == 1 {
                    root_close = i;
                    i = n;
                    continue;
                }
                if depth > 0 {
                    depth -= 1;
                }
                i += 1;
                continue;
            }
            b'[' => {
                depth += 1;
                i += 1;
                continue;
            }
            b']' => {
                if depth > 0 {
                    depth -= 1;
                }
                i += 1;
                continue;
            }
            _ => {
                i += 1;
                continue;
            }
        }
    }
    (keys, root_close)
}

/// Scan a JSONC value starting at `start` and return the index just after the value
/// (i.e. pointing at the trailing comma or the root closing brace).
fn scan_value_end(text: &str, mut i: usize) -> usize {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line = false;
    let mut in_block = false;
    while i < n {
        let c = bytes[i];
        if in_line {
            if c == b'\n' {
                in_line = false;
            }
            i += 1;
            continue;
        }
        if in_block {
            if c == b'*' && bytes.get(i + 1) == Some(&b'/') {
                in_block = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                in_line = true;
                i += 2;
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                in_block = true;
                i += 2;
                continue;
            }
            b'"' => {
                in_string = true;
                i += 1;
                continue;
            }
            b'{' | b'[' => {
                depth += 1;
                i += 1;
                continue;
            }
            b'}' | b']' => {
                if depth == 0 {
                    return i;
                }
                depth -= 1;
                i += 1;
                continue;
            }
            b',' if depth == 0 => {
                return i;
            }
            _ => {
                i += 1;
                continue;
            }
        }
    }
    i
}

/// In-place JSONC patch: replace/insert `[opencode]`, set `$schema`, and ensure
/// `_migrations` contains `migration_id`, preserving every other top-level block and
/// all comments verbatim.
fn patch_unified_omo_config_text(
    raw: &str,
    block_json: &str,
    schema_url: &str,
    migration_id: &str,
) -> String {
    let (keys, root_close) = scan_top_level_keys(raw);
    let find = |name: &str| keys.iter().find(|k| k.name == name);

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let mut insertions: Vec<String> = Vec::new();

    if let Some(k) = find("[opencode]") {
        replacements.push((k.value_start, k.value_end, block_json.to_string()));
    } else {
        insertions.push(format!("\"[opencode]\": {}", block_json));
    }

    if let Some(k) = find("$schema") {
        replacements.push((k.value_start, k.value_end, format!("\"{}\"", schema_url)));
    } else {
        insertions.push(format!("\"$schema\": \"{}\"", schema_url));
    }

    if let Some(k) = find("_migrations") {
        let existing: Vec<String> =
            json5::from_str(&raw[k.value_start..k.value_end]).unwrap_or_default();
        let mut markers = existing;
        if !markers.iter().any(|m| m == migration_id) {
            markers.push(migration_id.to_string());
        }
        let arr = serde_json::json!(markers);
        replacements.push((
            k.value_start,
            k.value_end,
            serde_json::to_string(&arr).unwrap_or_default(),
        ));
    } else {
        insertions.push(format!("\"_migrations\": [\"{}\"]", migration_id));
    }

    replacements.sort_by_key(|(start, _, _)| *start);

    let mut out = String::with_capacity(raw.len() + 256);
    let mut cursor = 0usize;
    for (start, end, new_text) in &replacements {
        out.push_str(&raw[cursor..*start]);
        out.push_str(new_text);
        cursor = *end;
    }
    out.push_str(&raw[cursor..root_close]);

    // Insert missing keys before the root brace, comma-separated and one per line.
    if !insertions.is_empty() {
        let tail = out.trim_end();
        let needs_comma = !tail.is_empty() && !tail.ends_with(',') && !tail.ends_with('{');
        if needs_comma {
            out.push(',');
        }
        for (index, insertion) in insertions.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str("\n  ");
            out.push_str(insertion);
        }
        out.push('\n');
    }

    out.push_str(&raw[root_close..]);
    out
}

/// Remove a top-level key from JSONC text in place (comment-preserving), handling the
/// surrounding comma. Returns the patched text and whether the key was found.
fn remove_top_level_key(text: &str, name: &str) -> (String, bool) {
    let (keys, root_close) = scan_top_level_keys(text);
    let Some(k) = keys.iter().find(|k| k.name == name) else {
        return (text.to_string(), false);
    };
    let bytes = text.as_bytes();
    let has_comma = k.value_end < root_close && bytes[k.value_end] == b',';
    let mut out = String::with_capacity(text.len());
    if has_comma {
        // Remove [key_start, comma+1) so the preceding comma is kept and the next key stays valid.
        out.push_str(&text[..k.key_start]);
        out.push_str(&text[k.value_end + 1..]);
    } else {
        // Last key: also drop the preceding comma so no trailing comma remains before `}`.
        let mut start = k.key_start;
        let mut before = k.key_start;
        while before > 0 && bytes[before - 1].is_ascii_whitespace() {
            before -= 1;
        }
        if before > 0 && bytes[before - 1] == b',' {
            start = before - 1;
        }
        out.push_str(&text[..start]);
        out.push_str(&text[root_close..]);
    }
    (out, true)
}

/// Remove the `[opencode]` block from `~/.omo/omo.jsonc`, preserving shared keys and
/// comments. If only control keys (`$schema`/`_migrations`/`legacy_migrations`) remain,
/// the file is deleted entirely.
fn remove_opencode_block(config_path: &Path) -> Result<(), String> {
    if !config_path.exists() {
        return Ok(());
    }
    let content =
        fs::read_to_string(config_path).map_err(|e| format!("Failed to read omo.jsonc: {}", e))?;
    if !matches!(json5::from_str::<Value>(&content), Ok(Value::Object(_))) {
        return Ok(());
    }

    let (patched, found) = remove_top_level_key(&content, "[opencode]");
    if !found {
        return Ok(());
    }

    let (remaining_keys, _) = scan_top_level_keys(&patched);
    let meaningful = remaining_keys.iter().any(|k| {
        !matches!(
            k.name.as_str(),
            "$schema" | "_migrations" | "legacy_migrations"
        )
    });

    if meaningful {
        fs::write(config_path, patched).map_err(|e| format!("Failed to write omo.jsonc: {}", e))?;
    } else {
        fs::remove_file(config_path).map_err(|e| format!("Failed to remove omo.jsonc: {}", e))?;
    }
    Ok(())
}
/// Apply an Oh My OpenAgent config to the JSON file
#[tauri::command]
pub async fn apply_oh_my_openagent_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    let db = state.db();
    apply_config_internal(&db, &app, &config_id, false).await?;
    Ok(())
}

/// Internal function to apply config: writes to file and updates database
/// This is the single source of truth for applying an Oh My OpenAgent config
pub async fn apply_config_internal<R: tauri::Runtime>(
    db: &crate::db::SqliteDbState,
    app: &tauri::AppHandle<R>,
    config_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    apply_config_internal_with_events(db, app, config_id, from_tray, true).await
}

pub async fn apply_config_internal_without_events<R: tauri::Runtime>(
    db: &crate::db::SqliteDbState,
    app: &tauri::AppHandle<R>,
    config_id: &str,
) -> Result<(), String> {
    apply_config_internal_with_events(db, app, config_id, false, false).await
}

async fn apply_config_internal_with_events<R: tauri::Runtime>(
    db: &crate::db::SqliteDbState,
    app: &tauri::AppHandle<R>,
    config_id: &str,
    from_tray: bool,
    emit_events: bool,
) -> Result<(), String> {
    // 应用配置到文件
    apply_config_to_file(db, config_id).await?;

    // Update database - set all configs to not applied, then set this one to applied
    let now = Local::now().to_rfc3339();

    db.with_conn_mut(|conn| {
        db_update_applied_status(conn, DbTable::OhMyOpenAgentConfig, Some(config_id), &now)
    })?;

    if emit_events {
        // Notify based on source
        let payload = if from_tray { "tray" } else { "window" };
        let _ = app.emit("config-changed", payload);

        // Trigger WSL sync via event (Windows only)
        #[cfg(target_os = "windows")]
        let _ = app.emit("wsl-sync-request-opencode", ());
    }

    Ok(())
}

/// Reorder Oh My OpenAgent configs (by name for now)
#[tauri::command]
pub async fn reorder_oh_my_openagent_configs(
    state: tauri::State<'_, SqliteDbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.db();
    for (index, id) in ids.iter().enumerate() {
        db.with_conn(|conn| {
            db_patch_fields(
                conn,
                DbTable::OhMyOpenAgentConfig,
                id,
                &[("sort_index", serde_json::json!(index as i32))],
            )
            .map(|_| ())
        })?;
    }

    Ok(())
}

/// Toggle is_disabled status for a config
#[tauri::command]
pub async fn toggle_oh_my_openagent_config_disabled(
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
                DbTable::OhMyOpenAgentConfig,
                &config_id,
                &[
                    ("is_disabled", Value::Bool(is_disabled)),
                    ("updated_at", Value::String(now.clone())),
                ],
            )
        })?
        .ok_or_else(|| format!("Config '{}' not found", config_id))?;

    let is_applied = adapter::get_bool_compat(&config_value, "is_applied", "isApplied", false);
    if is_applied {
        apply_config_internal(&db, &app, &config_id, false).await?;
    }

    Ok(())
}

/// Get Oh My OpenAgent config file path info
#[tauri::command]
pub async fn get_oh_my_openagent_config_path_info(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<ConfigPathInfo, String> {
    let db = state.db();
    let (config_path, source) = get_oh_my_openagent_config_path_and_source(&db).await?;
    let path = config_path.to_string_lossy().to_string();

    Ok(ConfigPathInfo {
        path,
        source: source.to_string(),
    })
}

/// Check if a local Oh My OpenAgent config file exists
#[tauri::command]
pub async fn check_oh_my_openagent_config_exists(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<bool, String> {
    let db = state.db();
    let config_path = get_oh_my_openagent_config_path(&db).await?;
    Ok(config_path.exists())
}

// ============================================================================
// Oh My OpenAgent Global Config Commands
// ============================================================================

/// Get Oh My OpenAgent global config (固定 ID 为 "global")
#[tauri::command]
pub async fn get_oh_my_openagent_global_config(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<OhMyOpenAgentGlobalConfig, String> {
    let db = state.db();
    if let Some(config) = get_global_config_from_sqlite(db)? {
        return Ok(config);
    }
    if let Ok(temp_config) = load_temp_global_config_from_file(db).await {
        return Ok(temp_config);
    }
    Ok(default_global_config())
}

/// Save Oh My OpenAgent global config
#[tauri::command]
#[allow(unused_variables)] // app 在 Windows 平台上用于 WSL 同步
pub async fn save_oh_my_openagent_global_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OhMyOpenAgentGlobalConfigInput,
) -> Result<OhMyOpenAgentGlobalConfig, String> {
    let db = state.db();

    let now = Local::now().to_rfc3339();
    let content = OhMyOpenAgentGlobalConfigContent {
        schema: input.schema,
        sisyphus_agent: input.sisyphus_agent,
        disabled_agents: input.disabled_agents,
        disabled_mcps: input.disabled_mcps,
        disabled_hooks: input.disabled_hooks,
        disabled_skills: input.disabled_skills,
        lsp: input.lsp,
        experimental: input.experimental,
        background_task: input.background_task,
        browser_automation_engine: input.browser_automation_engine,
        claude_code: input.claude_code,
        other_fields: input.other_fields,
        updated_at: now.clone(),
    };

    let json_data = adapter::global_config_to_db_value(&content);

    put_global_config_to_sqlite(db, &json_data)?;

    let applied_configs = db.with_conn(|conn| {
        db_query_by_bool(
            conn,
            DbTable::OhMyOpenAgentConfig,
            &JsonFieldPath::new("is_applied")?,
            true,
            None,
            Some(1),
        )
    })?;

    if let Some(record) = applied_configs.first() {
        let applied_config = adapter::from_db_value(record.clone());
        if apply_config_to_file(&db, &applied_config.id).await.is_ok() {
            #[cfg(target_os = "windows")]
            let _ = app.emit("wsl-sync-request-opencode", ());
        }
    }

    get_global_config_from_sqlite(db)?
        .ok_or_else(|| "Failed to retrieve saved global config".to_string())
}

/// Save local config (both Agents Profile and Global Config) into database
/// This is used when saving __local__ temporary config to database
/// Input can include config and/or globalConfig; missing parts will be loaded from local files
#[tauri::command]
pub async fn save_oh_my_openagent_local_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OhMyOpenAgentLocalConfigInput,
) -> Result<(), String> {
    let db = state.db();

    // Load base config from local files
    let base_config = load_temp_config_from_file(&db).await?;
    let base_global = load_temp_global_config_from_file(&db).await.ok();

    let now = Local::now().to_rfc3339();

    let config_content = build_local_profile_content(input.config, base_config, &now);

    let config_json = adapter::to_db_value(&config_content);
    let created_config_value =
        db.with_conn(|conn| db_create(conn, DbTable::OhMyOpenAgentConfig, &config_json))?;
    let created_config = adapter::from_db_value(created_config_value);

    let global_content = build_local_global_content(input.global_config, base_global, &now);

    let global_json = adapter::global_config_to_db_value(&global_content);
    put_global_config_to_sqlite(db, &global_json)?;

    if let Err(e) = apply_config_to_file(&db, &created_config.id).await {
        eprintln!("Failed to apply config after local save: {}", e);
    } else {
        #[cfg(target_os = "windows")]
        let _ = app.emit("wsl-sync-request-opencode", ());
    }

    let _ = app.emit("config-changed", "window");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn local_profile_with_other_fields() -> OhMyOpenAgentAgentsProfile {
        OhMyOpenAgentAgentsProfile {
            id: "__local__".to_string(),
            name: "Local Profile".to_string(),
            is_applied: false,
            is_disabled: false,
            agents: Some(json!({ "coder": { "model": "old-model" } })),
            categories: Some(json!({ "coding": { "model": "old-category" } })),
            other_fields: Some(json!({ "background_task": { "defaultConcurrency": 3 } })),
            sort_index: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn local_global_with_other_fields() -> OhMyOpenAgentGlobalConfig {
        OhMyOpenAgentGlobalConfig {
            id: "global".to_string(),
            schema: Some("old-schema".to_string()),
            sisyphus_agent: Some(json!({ "model": "old-sisyphus" })),
            disabled_agents: Some(vec!["coder".to_string()]),
            disabled_mcps: Some(vec!["filesystem".to_string()]),
            disabled_hooks: Some(vec!["pre-run".to_string()]),
            disabled_skills: Some(vec!["review".to_string()]),
            lsp: Some(json!({ "rust": { "enabled": true } })),
            experimental: Some(json!({ "feature": true })),
            background_task: Some(json!({ "defaultConcurrency": 3 })),
            browser_automation_engine: Some(json!({ "provider": "playwright" })),
            claude_code: Some(json!({ "enabled": true })),
            other_fields: Some(json!({ "custom": "old" })),
            updated_at: None,
        }
    }

    #[test]
    fn local_profile_input_clears_optional_fields_instead_of_reusing_local_file() {
        let content = build_local_profile_content(
            Some(OhMyOpenAgentAgentsProfileInput {
                id: None,
                name: "Edited Profile".to_string(),
                agents: None,
                categories: None,
                other_fields: None,
            }),
            local_profile_with_other_fields(),
            "2026-05-24T00:00:00+08:00",
        );

        assert_eq!(content.name, "Edited Profile");
        assert_eq!(content.agents, None);
        assert_eq!(content.categories, None);
        assert_eq!(content.other_fields, None);
        assert!(content.is_applied);
    }

    #[test]
    fn local_profile_without_input_reuses_local_file_fields() {
        let content = build_local_profile_content(
            None,
            local_profile_with_other_fields(),
            "2026-05-24T00:00:00+08:00",
        );

        assert_eq!(
            content.agents,
            Some(json!({ "coder": { "model": "old-model" } }))
        );
        assert_eq!(
            content.other_fields,
            Some(json!({ "background_task": { "defaultConcurrency": 3 } }))
        );
    }

    #[test]
    fn local_global_input_clears_optional_fields_instead_of_reusing_local_file() {
        let content = build_local_global_content(
            Some(OhMyOpenAgentGlobalConfigInput {
                schema: None,
                sisyphus_agent: None,
                disabled_agents: None,
                disabled_mcps: None,
                disabled_hooks: None,
                disabled_skills: None,
                lsp: None,
                experimental: None,
                background_task: None,
                browser_automation_engine: None,
                claude_code: None,
                other_fields: None,
            }),
            Some(local_global_with_other_fields()),
            "2026-05-24T00:00:00+08:00",
        );

        assert_eq!(content.schema, None);
        assert_eq!(content.sisyphus_agent, None);
        assert_eq!(content.disabled_agents, None);
        assert_eq!(content.other_fields, None);
    }

    #[test]
    fn local_global_without_input_reuses_local_file_fields() {
        let content = build_local_global_content(
            None,
            Some(local_global_with_other_fields()),
            "2026-05-24T00:00:00+08:00",
        );

        assert_eq!(content.schema, Some("old-schema".to_string()));
        assert_eq!(content.disabled_agents, Some(vec!["coder".to_string()]));
        assert_eq!(content.other_fields, Some(json!({ "custom": "old" })));
    }

    #[test]
    fn flatten_omo_config_merges_base_agents_with_opencode_block() {
        let input = json!({
            "$schema": "https://example/omo.schema.json",
            "codegraph": { "daemon": true },
            "models": { "opus": { "model": "x/y" } },
            "task": { "default_concurrency": 5 },
            "agents": { "baseAgent": { "model": "a/b" } },
            "[opencode]": {
                "agents": {
                    "sisyphus": { "model": "c/d" },
                    "baseAgent": { "model": "overridden" }
                },
                "background_task": { "defaultConcurrency": 4 }
            }
        });

        let flat = flatten_omo_config(&input);
        let obj = flat.as_object().unwrap();

        // [opencode] block wins over base for the same key
        assert_eq!(obj["agents"]["baseAgent"]["model"], "overridden");
        // block-only agent keys survive
        assert_eq!(obj["agents"]["sisyphus"]["model"], "c/d");
        // block top-level plugin keys survive
        assert!(obj.contains_key("background_task"));
        // omo-config-core control keys and $schema are stripped
        assert!(!obj.contains_key("codegraph"));
        assert!(!obj.contains_key("models"));
        assert!(!obj.contains_key("task"));
        assert!(!obj.contains_key("$schema"));
    }

    #[test]
    fn flatten_omo_config_returns_unchanged_when_no_block() {
        let input = json!({ "agents": { "a": { "model": "x" } } });
        assert_eq!(flatten_omo_config(&input), input);
    }

    #[test]
    fn write_unified_omo_config_preserves_shared_keys_and_stamps_migration_marker() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("omo.jsonc");
        std::fs::write(
            &path,
            r#"{
  "codegraph": { "daemon": true },
  "models": { "opus": { "model": "x/y" } }
}"#,
        )
        .unwrap();

        let plugin = json!({ "agents": { "sisyphus": { "model": "c/d" } } });
        write_unified_omo_config(&path, &plugin).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let obj: Value = serde_json::from_str(&content).unwrap();
        assert!(obj.as_object().unwrap().contains_key("codegraph"));
        assert!(obj.as_object().unwrap().contains_key("models"));
        assert_eq!(obj["[opencode]"], plugin);
        assert_eq!(obj["$schema"], OMO_UNIFIED_SCHEMA_URL);
        let markers = obj["_migrations"].as_array().unwrap();
        assert!(markers
            .iter()
            .any(|m| m.as_str() == Some(OMO_UNIFIED_MIGRATION_ID)));
    }

    #[test]
    fn remove_opencode_block_removes_block_but_preserves_shared_keys() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("omo.jsonc");
        std::fs::write(
            &path,
            r#"{ "codegraph": { "daemon": true }, "[opencode]": { "agents": {} } }"#,
        )
        .unwrap();

        remove_opencode_block(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let obj: Value = serde_json::from_str(&content).unwrap();
        assert!(obj.as_object().unwrap().contains_key("codegraph"));
        assert!(!obj.as_object().unwrap().contains_key("[opencode]"));
    }

    #[test]
    fn remove_opencode_block_deletes_file_when_only_control_keys_remain() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("omo.jsonc");
        std::fs::write(
            &path,
            r#"{ "$schema": "x", "_migrations": [], "[opencode]": { "agents": {} } }"#,
        )
        .unwrap();

        remove_opencode_block(&path).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn write_unified_omo_config_preserves_comments_in_shared_blocks() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("omo.jsonc");
        std::fs::write(
            &path,
            r#"{
  // shared codegraph settings
  "codegraph": { "daemon": true /* keep daemon */ },
  "models": {
    "opus": { "model": "x/y" } // my favorite model
  }
}"#,
        )
        .unwrap();

        let plugin = json!({ "agents": { "s": { "model": "a/b" } } });
        write_unified_omo_config(&path, &plugin).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        // comments in shared blocks survive the in-place patch
        assert!(content.contains("// shared codegraph settings"));
        assert!(content.contains("/* keep daemon */"));
        assert!(content.contains("// my favorite model"));
        // still valid JSONC and all keys present
        let obj: Value = json5::from_str(&content).unwrap();
        assert!(obj.as_object().unwrap().contains_key("codegraph"));
        assert!(obj.as_object().unwrap().contains_key("models"));
        assert_eq!(obj["[opencode]"], plugin);
        assert_eq!(obj["$schema"], OMO_UNIFIED_SCHEMA_URL);
    }

    #[test]
    fn migrate_legacy_local_configs_to_unified_moves_flat_config_into_opencode_block() {
        let temp = tempfile::tempdir().unwrap();
        let legacy = temp.path().join("oh-my-openagent.jsonc");
        let unified = temp.path().join(".omo").join("omo.jsonc");
        std::fs::write(
            &legacy,
            r#"{
  "$schema": "legacy",
  "agents": { "Sisyphus": { "model": "a/b" } },
  "codegraph": { "daemon": true }
}"#,
        )
        .unwrap();

        let changed = migrate_legacy_local_configs_to_unified(&[legacy.clone()], &unified).unwrap();

        assert!(changed);
        assert!(!legacy.exists());
        let content = std::fs::read_to_string(&unified).unwrap();
        let obj: Value = json5::from_str(&content).unwrap();
        assert_eq!(obj["[opencode]"]["agents"]["Sisyphus"]["model"], "a/b");
        assert!(obj["[opencode]"].get("codegraph").is_none());
        assert_eq!(obj["$schema"], OMO_UNIFIED_SCHEMA_URL);
    }

    #[test]
    fn migrate_legacy_local_configs_to_unified_preserves_existing_shared_keys() {
        let temp = tempfile::tempdir().unwrap();
        let legacy = temp.path().join("oh-my-openagent.jsonc");
        let unified = temp.path().join(".omo").join("omo.jsonc");
        std::fs::create_dir_all(unified.parent().unwrap()).unwrap();
        std::fs::write(
            &unified,
            r#"{
  "codegraph": { "daemon": true },
  "models": { "opus": { "model": "x/y" } }
}"#,
        )
        .unwrap();
        std::fs::write(
            &legacy,
            r#"{
  "agents": { "sisyphus": { "model": "a/b" } }
}"#,
        )
        .unwrap();

        let changed = migrate_legacy_local_configs_to_unified(&[legacy.clone()], &unified).unwrap();

        assert!(changed);
        assert!(!legacy.exists());
        let content = std::fs::read_to_string(&unified).unwrap();
        assert!(content.contains("\"codegraph\""));
        let obj: Value = json5::from_str(&content).unwrap();
        assert_eq!(obj["codegraph"]["daemon"], true);
        assert_eq!(obj["models"]["opus"]["model"], "x/y");
        assert_eq!(obj["[opencode]"]["agents"]["sisyphus"]["model"], "a/b");
    }

    #[test]
    fn patch_unified_omo_config_text_merges_existing_migration_markers() {
        let raw = r#"{
  "codegraph": { "daemon": true },
  "[opencode]": { "agents": {} },
  "_migrations": ["2026-08-reasoning-unification"]
}"#;
        let patched = patch_unified_omo_config_text(
            raw,
            "{ }",
            "https://example/omo.schema.json",
            "2026-07-opencode-config-unification",
        );
        let obj: Value = json5::from_str(&patched).unwrap();
        let names: Vec<&str> = obj["_migrations"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m.as_str())
            .collect();
        assert!(names.contains(&"2026-08-reasoning-unification"));
        assert!(names.contains(&"2026-07-opencode-config-unification"));
    }

    #[test]
    fn remove_top_level_key_handles_middle_and_last_keys() {
        let (middle, found) = remove_top_level_key(r#"{ "a": 1, "b": 2, "c": 3 }"#, "b");
        assert!(found);
        let obj: Value = json5::from_str(&middle).unwrap();
        assert!(obj.get("a").is_some() && obj.get("c").is_some() && obj.get("b").is_none());

        let (last, found) = remove_top_level_key(r#"{ "a": 1, "b": 2 }"#, "b");
        assert!(found);
        let obj: Value = json5::from_str(&last).unwrap();
        assert!(obj.get("a").is_some() && obj.get("b").is_none());
    }

    #[test]
    fn remove_opencode_block_preserves_comments_in_shared_blocks() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("omo.jsonc");
        std::fs::write(
            &path,
            r#"{
  "codegraph": { "daemon": true }, // keep codegraph
  "[opencode]": { "agents": {} }
}"#,
        )
        .unwrap();

        remove_opencode_block(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// keep codegraph"));
        assert!(content.contains("\"codegraph\""));
        assert!(!content.contains("[opencode]"));
    }

    #[test]
    fn strip_unified_unknown_keys_removes_google_auth_and_lsp() {
        let mut config = json!({
            "agents": { "sisyphus": { "model": "a/b" } },
            "google_auth": false,
            "lsp": { "gopls": { "command": ["gopls"] } },
            "background_task": { "defaultConcurrency": 4 }
        });
        strip_unified_unknown_keys(&mut config);
        let obj = config.as_object().unwrap();
        // unknown keys stripped
        assert!(!obj.contains_key("google_auth"));
        assert!(!obj.contains_key("lsp"));
        // legit plugin keys preserved
        assert!(obj.contains_key("agents"));
        assert!(obj.contains_key("background_task"));
    }

    #[test]
    fn strip_unified_unknown_keys_leaves_non_object_unchanged() {
        let mut config = json!([1, 2, 3]);
        strip_unified_unknown_keys(&mut config);
        assert_eq!(config, json!([1, 2, 3]));
    }

    #[test]
    fn strip_unified_unknown_keys_preserves_keys_not_in_blocklist() {
        let mut config = json!({
            "claude_code": { "agents": false },
            "codegraph": { "daemon": true }
        });
        strip_unified_unknown_keys(&mut config);
        let obj = config.as_object().unwrap();
        assert!(obj.contains_key("claude_code"));
        assert!(obj.contains_key("codegraph"));
    }
}
