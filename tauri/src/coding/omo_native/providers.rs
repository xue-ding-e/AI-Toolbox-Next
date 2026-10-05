//! OmO Native 的 provider 管理。
//!
//! 数据面：`<agentDir>/models.json` 的 `providers` 记录 + `<agentDir>/auth.json` 的密钥。
//!
//! `models.json` **没有对外发布的 schema**，字段形状反推自上游
//! `packages/omo-native/bin/lib/setup-opencode-providers.js` 的 `convertProvider`：
//! provider 条目含 `baseUrl` / `api` / `headers` / `models[]`，模型条目含
//! `id` / `name` / `reasoning` / `input` / `contextWindow` / `maxTokens` / `upstreamModelId`。
//! 因此写入一律按 provider key 局部更新，保留其他 provider 与未知字段。

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use tauri::Emitter;

use super::constants;
use super::types::{OmoNativeMcpServer, OmoNativeProvider, OmoNativeProviderInput, OmoNativeSkill};
use crate::db::SqliteDbState;

// ============================================================================
// 文件路径
// ============================================================================

async fn agent_dir(db: &SqliteDbState) -> Result<std::path::PathBuf, String> {
    let location = crate::coding::runtime_location::get_omo_native_runtime_location_async(db).await?;
    Ok(location.host_path.clone())
}

fn read_json_object(path: &Path) -> Result<Option<Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    let parsed: Value = json5::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
    Ok(Some(parsed))
}

fn write_json_object(path: &Path, value: &Value, private: bool) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize {}: {}", path.display(), e))?;
    fs::write(path, format!("{}\n", text))
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
    // auth.json 含密钥，收紧到 0600（与上游 auth-store.js 一致）。Windows 上该调用无实际作用。
    if private {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
    }
    Ok(())
}

/// 引擎把 `auth.json` 的值当「config value」解析：`$NAME` / `${NAME}` 插值、
/// `!cmd` 执行 shell、`$$` / `$!` 是字面量转义。写入用户密钥时必须先转义，
/// 否则含 `$` 的 key 会被引擎当变量展开。
///
/// 对应上游 `packages/omo-native/bin/lib/auth-store.js` 的 `literalConfigValue`。
pub fn escape_literal_config_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch == '$' || ch == '!' {
            out.push('$');
        }
        out.push(ch);
    }
    out
}

// ============================================================================
// 读取
// ============================================================================

/// 列出 provider：内建名单 + `models.json` 自定义 provider + 密钥状态。
#[tauri::command]
pub async fn list_omo_native_providers(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<OmoNativeProvider>, String> {
    let db = state.db();
    let dir = agent_dir(&db).await?;
    let models = read_json_object(&dir.join(constants::OMO_NATIVE_MODELS_FILE))?;
    let auth = read_json_object(&dir.join(constants::OMO_NATIVE_AUTH_FILE))?;

    let custom_providers = models
        .as_ref()
        .and_then(|value| value.get("providers"))
        .and_then(Value::as_object);
    let auth_keys: Vec<String> = auth
        .as_ref()
        .and_then(Value::as_object)
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default();

    let mut providers: Vec<OmoNativeProvider> = Vec::new();

    // 内建 provider：只报名字与是否需要 OAuth，不谎称它们有 models.json 条目。
    for key in constants::OMO_NATIVE_BUILTIN_PROVIDERS {
        providers.push(OmoNativeProvider {
            key: key.to_string(),
            builtin: true,
            oauth: constants::is_oauth_provider(key),
            config: json!({}),
            has_key: auth_keys.iter().any(|auth_key| auth_key == key),
            custom: false,
        });
    }

    // 自定义 provider：`models.json` 里有条目、但不在内建名单里的。
    if let Some(map) = custom_providers {
        for (key, config) in map {
            if constants::is_builtin_provider(key) {
                // 内建 provider 在 models.json 里的覆盖条目：标记为 custom 但仍是内建。
                if let Some(existing) = providers.iter_mut().find(|p| p.key == *key) {
                    existing.config = config.clone();
                    existing.custom = true;
                }
                continue;
            }
            providers.push(OmoNativeProvider {
                key: key.clone(),
                builtin: false,
                oauth: constants::is_oauth_provider(key),
                config: config.clone(),
                has_key: auth_keys.iter().any(|auth_key| auth_key == key),
                custom: true,
            });
        }
    }

    providers.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(providers)
}

// ============================================================================
// 写入
// ============================================================================

/// 写入（或更新）一个 provider 到 `models.json`，可选同时写入 `auth.json` 的密钥。
///
/// 按 provider key 局部更新，保留其他 provider 与未知字段。
#[tauri::command]
pub async fn save_omo_native_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmoNativeProviderInput,
) -> Result<(), String> {
    let db = state.db();
    let key = input.key.trim();
    if key.is_empty() {
        return Err("Provider key cannot be empty".to_string());
    }
    if !input.config.is_object() {
        return Err("Provider config must be a JSON object".to_string());
    }

    let dir = agent_dir(&db).await?;

    // --- models.json ---
    let models_path = dir.join(constants::OMO_NATIVE_MODELS_FILE);
    let mut models = read_json_object(&models_path)?.unwrap_or_else(|| json!({}));
    if !models.is_object() {
        models = json!({});
    }
    let providers = models
        .as_object_mut()
        .expect("models is an object")
        .entry("providers".to_string())
        .or_insert_with(|| json!({}));
    if !providers.is_object() {
        *providers = json!({});
    }
    providers
        .as_object_mut()
        .expect("providers is an object")
        .insert(key.to_string(), input.config.clone());
    write_json_object(&models_path, &models, false)?;

    // --- auth.json（只在显式传入非空 key 时动） ---
    if let Some(api_key) = input.api_key.as_deref().filter(|k| !k.is_empty()) {
        let auth_path = dir.join(constants::OMO_NATIVE_AUTH_FILE);
        let mut auth = read_json_object(&auth_path)?.unwrap_or_else(|| json!({}));
        if !auth.is_object() {
            auth = json!({});
        }
        auth.as_object_mut().expect("auth is an object").insert(
            key.to_string(),
            json!({
                "type": "api_key",
                "key": escape_literal_config_value(api_key),
            }),
        );
        write_json_object(&auth_path, &auth, true)?;
    }

    let _ = app.emit("config-changed", "omo_native");
    Ok(())
}

/// 删除 `models.json` 里的 provider 条目（可选一并删除 `auth.json` 的密钥）。
#[tauri::command]
pub async fn delete_omo_native_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    provider_key: String,
    remove_key: bool,
) -> Result<(), String> {
    let db = state.db();
    let dir = agent_dir(&db).await?;

    let models_path = dir.join(constants::OMO_NATIVE_MODELS_FILE);
    if let Some(mut models) = read_json_object(&models_path)? {
        if let Some(providers) = models.get_mut("providers").and_then(Value::as_object_mut) {
            providers.remove(&provider_key);
        }
        write_json_object(&models_path, &models, false)?;
    }

    if remove_key {
        let auth_path = dir.join(constants::OMO_NATIVE_AUTH_FILE);
        if let Some(mut auth) = read_json_object(&auth_path)? {
            if let Some(map) = auth.as_object_mut() {
                map.remove(&provider_key);
            }
            write_json_object(&auth_path, &auth, true)?;
        }
    }

    let _ = app.emit("config-changed", "omo_native");
    Ok(())
}

// ============================================================================
// MCP
// ============================================================================

/// 列出 `<agentDir>/mcp.json` 的 `mcpServers`。
#[tauri::command]
pub async fn list_omo_native_mcp_servers(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<OmoNativeMcpServer>, String> {
    let db = state.db();
    let dir = agent_dir(&db).await?;
    let mcp = read_json_object(&dir.join(constants::OMO_NATIVE_MCP_FILE))?;
    let servers = mcp
        .as_ref()
        .and_then(|value| value.get("mcpServers"))
        .and_then(Value::as_object);

    let mut result: Vec<OmoNativeMcpServer> = servers
        .map(|map| {
            map.iter()
                .map(|(name, config)| OmoNativeMcpServer {
                    name: name.clone(),
                    config: config.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

/// 写入一个 MCP server 到 `<agentDir>/mcp.json`，保留其他 server 与未知字段。
#[tauri::command]
pub async fn save_omo_native_mcp_server(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    name: String,
    config: Value,
) -> Result<(), String> {
    let db = state.db();
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("MCP server name cannot be empty".to_string());
    }
    if !config.is_object() {
        return Err("MCP server config must be a JSON object".to_string());
    }

    let dir = agent_dir(&db).await?;
    let mcp_path = dir.join(constants::OMO_NATIVE_MCP_FILE);
    let mut mcp = read_json_object(&mcp_path)?.unwrap_or_else(|| json!({}));
    if !mcp.is_object() {
        mcp = json!({});
    }
    let servers = mcp
        .as_object_mut()
        .expect("mcp is an object")
        .entry("mcpServers".to_string())
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    servers
        .as_object_mut()
        .expect("servers is an object")
        .insert(trimmed.to_string(), config);

    // mcp.json 的 env/headers 可能含 token，与上游一致收紧权限。
    write_json_object(&mcp_path, &mcp, true)?;
    let _ = app.emit("config-changed", "omo_native");
    Ok(())
}

#[tauri::command]
pub async fn delete_omo_native_mcp_server(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    name: String,
) -> Result<(), String> {
    let db = state.db();
    let dir = agent_dir(&db).await?;
    let mcp_path = dir.join(constants::OMO_NATIVE_MCP_FILE);
    if let Some(mut mcp) = read_json_object(&mcp_path)? {
        if let Some(servers) = mcp.get_mut("mcpServers").and_then(Value::as_object_mut) {
            servers.remove(&name);
        }
        write_json_object(&mcp_path, &mcp, true)?;
    }
    let _ = app.emit("config-changed", "omo_native");
    Ok(())
}

// ============================================================================
// Skills
// ============================================================================

/// 列出 `<agentDir>/skills/` 下的 skill 目录。
#[tauri::command]
pub async fn list_omo_native_skills(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<OmoNativeSkill>, String> {
    let db = state.db();
    let dir = agent_dir(&db).await?.join(constants::OMO_NATIVE_SKILLS_DIR);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let entries =
        fs::read_dir(&dir).map_err(|e| format!("Failed to read skills directory: {}", e))?;
    let mut skills: Vec<OmoNativeSkill> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        skills.push(OmoNativeSkill {
            description: read_skill_description(&path),
            name,
            path: path.to_string_lossy().to_string(),
        });
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

/// 从 `SKILL.md` 的 YAML frontmatter 读 description（失败返回 None，不影响列表）。
fn read_skill_description(skill_dir: &Path) -> Option<String> {
    let content = fs::read_to_string(skill_dir.join("SKILL.md")).ok()?;
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix("description:") {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_doubles_dollar_and_bang() {
        assert_eq!(escape_literal_config_value("sk-abc"), "sk-abc");
        assert_eq!(escape_literal_config_value("a$b"), "a$$b");
        assert_eq!(escape_literal_config_value("a!b"), "a$!b");
        assert_eq!(escape_literal_config_value("${ENV}"), "$${ENV}");
        // 幂等性检查：转义后的字符串再转义会继续翻倍——所以只能转义一次。
        assert_eq!(escape_literal_config_value("a$$b"), "a$$$$b");
    }

    #[test]
    fn escape_leaves_other_chars_untouched() {
        assert_eq!(
            escape_literal_config_value("key-with_underscore.and/slash:colon"),
            "key-with_underscore.and/slash:colon"
        );
    }
}
