use chrono::Local;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::adapter;
use super::constants::{builtin_provider_name, is_builtin_provider, OMP_BUILTIN_PROVIDERS};
use super::types::*;
use crate::coding::db_id::db_new_id;
use crate::coding::open_code::shell_env;
use crate::coding::prompt_file::{read_prompt_content_file, write_prompt_content_file};
use crate::coding::runtime_location;
use crate::coding::skills::commands::resync_all_skills_if_tool_path_changed;
use crate::db::helpers::{
    db_delete, db_get, db_list, db_max_i64, db_patch_fields, db_put, db_update_applied_status,
};
use crate::db::schema::{DbTable, JsonFieldPath, OrderDirection, OrderField, OrderSpec};
use crate::db::SqliteDbState;
use tauri::{Emitter, Runtime};

/// OMP 思考级别白名单(OMP 支持 `auto`,比 Pi 多一个)。
const OMP_THINKING_LEVEL_KEYS: [&str; 8] =
    ["off", "minimal", "low", "medium", "high", "xhigh", "max", "auto"];
const OMP_EXTENDED_THINKING_LEVEL_KEYS: [&str; 2] = ["xhigh", "max"];
const OMP_MODEL_ROLE_KEY: &str = "default";

fn get_home_dir() -> Result<PathBuf, String> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Failed to get home directory".to_string())
}

pub fn get_omp_default_root_dir() -> Result<PathBuf, String> {
    Ok(get_home_dir()?.join(".omp").join("agent"))
}

fn get_omp_root_dir_from_shell() -> Option<PathBuf> {
    shell_env::get_env_from_shell_config(super::constants::OMP_ENV_KEY)
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
}

pub fn get_omp_root_dir_without_db() -> Result<PathBuf, String> {
    if let Ok(env_path) = std::env::var(super::constants::OMP_ENV_KEY) {
        if !env_path.trim().is_empty() {
            return Ok(PathBuf::from(env_path));
        }
    }
    if let Some(shell_path) = get_omp_root_dir_from_shell() {
        return Ok(shell_path);
    }
    get_omp_default_root_dir()
}

pub fn get_omp_root_path_info_from_db(db: &SqliteDbState) -> Result<OmpPathInfo, String> {
    let location = runtime_location::get_oh_my_pi_runtime_location_sync(db)?;
    Ok(OmpPathInfo {
        path: location.host_path.to_string_lossy().to_string(),
        source: location.source,
    })
}

pub async fn get_omp_root_path_info_from_db_async(db: &SqliteDbState) -> Result<OmpPathInfo, String> {
    let location = runtime_location::get_oh_my_pi_runtime_location_async(db).await?;
    Ok(OmpPathInfo {
        path: location.host_path.to_string_lossy().to_string(),
        source: location.source,
    })
}

pub async fn get_omp_root_dir_from_db_async(db: &SqliteDbState) -> Result<PathBuf, String> {
    Ok(runtime_location::get_oh_my_pi_runtime_location_async(db)
        .await?
        .host_path)
}

pub fn get_omp_config_path_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join(super::constants::OMP_CONFIG_FILE)
}

pub fn get_omp_models_path_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join(super::constants::OMP_MODELS_FILE)
}

pub fn get_omp_mcp_path_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join(super::constants::OMP_MCP_FILE)
}

pub fn get_omp_prompt_path_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join(super::constants::OMP_PROMPT_FILE)
}

pub async fn get_omp_config_path_async(db: &SqliteDbState) -> Result<PathBuf, String> {
    Ok(get_omp_config_path_from_root(&get_omp_root_dir_from_db_async(db).await?))
}

pub async fn get_omp_models_path_async(db: &SqliteDbState) -> Result<PathBuf, String> {
    Ok(get_omp_models_path_from_root(&get_omp_root_dir_from_db_async(db).await?))
}

pub async fn get_omp_mcp_path_async(db: &SqliteDbState) -> Result<PathBuf, String> {
    Ok(get_omp_mcp_path_from_root(&get_omp_root_dir_from_db_async(db).await?))
}

pub fn get_omp_mcp_path_from_db(db: &SqliteDbState) -> Result<PathBuf, String> {
    Ok(get_omp_mcp_path_from_root(
        &runtime_location::get_oh_my_pi_runtime_location_sync(db)?.host_path,
    ))
}

pub async fn get_omp_prompt_path_async(db: &SqliteDbState) -> Result<PathBuf, String> {
    Ok(get_omp_prompt_path_from_root(&get_omp_root_dir_from_db_async(db).await?))
}

fn read_yaml_object_or_empty(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;
    let parsed: Value = serde_json::to_value(yaml)
        .map_err(|error| format!("Failed to convert {}: {error}", path.display()))?;
    if parsed.is_object() {
        Ok(parsed)
    } else {
        Err(format!("{} must contain a YAML mapping", path.display()))
    }
}

fn write_yaml_object(path: &Path, value: &Value) -> Result<(), String> {
    if !value.is_object() {
        return Err(format!(
            "{} must be written as a YAML mapping",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }
    let content = serde_yaml::to_string(value)
        .map_err(|error| format!("Failed to serialize {}: {error}", path.display()))?;
    fs::write(path, content)
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))?;
    Ok(())
}

fn object_mut(value: &mut Value) -> Result<&mut Map<String, Value>, String> {
    value
        .as_object_mut()
        .ok_or_else(|| "Expected a mapping object".to_string())
}

fn get_models_providers(models: &Value) -> Vec<(String, Value)> {
    models
        .get("providers")
        .and_then(Value::as_object)
        .map(|providers| {
            providers
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn model_ids_from_provider(provider: Option<&Value>) -> Vec<String> {
    provider
        .and_then(|value| value.get("models"))
        .and_then(Value::as_array)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| model.get("id").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn is_omp_settings_hidden_key(key: &str) -> bool {
    matches!(
        key,
        "modelRoles"
            | "defaultThinkingLevel"
            | "modelProviderOrder"
            | "modelRoleStorage"
            | "extensions"
            | "disabledExtensions"
            | "enabledModels"
            | "disabledProviders"
            | "modelTags"
            | "cycleOrder"
    )
}

fn build_other_settings(settings: &Value) -> Value {
    let mut other = settings.as_object().cloned().unwrap_or_default();
    for key in other.keys().cloned().collect::<Vec<_>>() {
        if is_omp_settings_hidden_key(&key) {
            other.remove(&key);
        }
    }
    Value::Object(other)
}

fn apply_omp_other_settings(
    settings_object: &mut Map<String, Value>,
    other_settings: &Map<String, Value>,
) {
    for key in settings_object.keys().cloned().collect::<Vec<_>>() {
        if !is_omp_settings_hidden_key(&key) {
            settings_object.remove(&key);
        }
    }
    for (key, value) in other_settings {
        if !is_omp_settings_hidden_key(key) {
            settings_object.insert(key.clone(), value.clone());
        }
    }
}

/// 将 provider 配置规范化为 OMP models.yml schema 可接受的形式。
///
/// OMP 的 schema 严格:model 的 `cost` 必须 input/output/cacheRead/cacheWrite
/// 四个字段齐全,否则整个 models.yml 校验失败、所有自定义 provider 被禁用;
/// OMP 用 `thinking` 结构表达思考级别,不识别 Pi 的 `thinkingLevelMap`,写了也会被忽略。
fn normalize_omp_provider_for_omptype(provider: &mut Value) {
    let Some(provider_obj) = provider.as_object_mut() else {
        return;
    };
    let Some(models) = provider_obj.get_mut("models").and_then(Value::as_array_mut) else {
        return;
    };
    for model in models {
        let Some(model_obj) = model.as_object_mut() else {
            continue;
        };
        if let Some(cost) = model_obj.get("cost") {
            let complete = ["input", "output", "cacheRead", "cacheWrite"]
                .iter()
                .all(|key| cost.get(*key).and_then(Value::as_f64).is_some());
            if !complete {
                model_obj.remove("cost");
            }
        }
        model_obj.remove("thinkingLevelMap");
    }
}

/// 从 config.yml 解析默认模型选择。
/// OMP 用 `modelRoles.default`(格式 `provider/modelId`)表达默认模型,
/// `defaultThinkingLevel` 表达思考级别。
fn default_selection_from_settings(settings: &Value) -> OmpDefaultSelection {
    let model_role = settings
        .get("modelRoles")
        .and_then(Value::as_object)
        .and_then(|roles| roles.get(OMP_MODEL_ROLE_KEY))
        .and_then(Value::as_str)
        .map(str::to_string);

    let (provider_key, model_id) = match model_role {
        Some(role) if !role.trim().is_empty() => split_provider_model(&role),
        _ => (None, None),
    };

    OmpDefaultSelection {
        provider_key,
        model_id,
        thinking_level: settings
            .get("defaultThinkingLevel")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// 按第一个 `/` 拆分 `provider/modelId`(与 OMP `parseModelString` 一致)。
fn split_provider_model(role: &str) -> (Option<String>, Option<String>) {
    match role.find('/') {
        Some(index) if index > 0 => {
            let provider = role[..index].to_string();
            let model = role[index + 1..].to_string();
            (
                Some(provider),
                if model.is_empty() { None } else { Some(model) },
            )
        }
        _ => (None, None),
    }
}

fn model_supports_thinking_level(model: &Value, thinking_level: &str) -> bool {
    if !OMP_THINKING_LEVEL_KEYS.contains(&thinking_level) || thinking_level == "auto" {
        // `auto` 总会生效;这里只负责非 auto 的判断。
        if thinking_level == "auto" {
            return true;
        }
        return false;
    }
    if model.get("reasoning").and_then(Value::as_bool) != Some(true) {
        return false;
    }

    match model
        .get("thinkingLevelMap")
        .and_then(Value::as_object)
        .and_then(|map| map.get(thinking_level))
    {
        Some(Value::Null) => false,
        Some(_) => true,
        None => !OMP_EXTENDED_THINKING_LEVEL_KEYS.contains(&thinking_level),
    }
}

fn credential_kind(provider: Option<&Value>, is_builtin: bool) -> OmpCredentialKind {
    let auth = provider
        .and_then(|value| value.get("auth"))
        .and_then(Value::as_str);
    if auth == Some("oauth") {
        return OmpCredentialKind::Oauth;
    }
    if provider
        .and_then(|value| value.get("apiKey"))
        .map(|value| !value.is_null())
        .unwrap_or(false)
    {
        return OmpCredentialKind::ApiKey;
    }
    if auth == Some("none") {
        return OmpCredentialKind::None;
    }
    if is_builtin {
        OmpCredentialKind::EnvPossible
    } else {
        OmpCredentialKind::None
    }
}

fn build_provider_views(settings: &Value, models: &Value) -> Vec<OmpRuntimeProviderView> {
    let default_selection = default_selection_from_settings(settings);
    let default_provider = default_selection.provider_key.clone();
    let default_model = default_selection.model_id.clone();

    let models_map: Map<String, Value> = get_models_providers(models).into_iter().collect();

    let mut keys = BTreeSet::new();
    for (key, _) in &models_map {
        keys.insert(key.clone());
    }
    if let Some(default_provider) = &default_provider {
        if !default_provider.trim().is_empty() {
            keys.insert(default_provider.clone());
        }
    }

    // 不把全部内置渠道无条件塞进列表:OMP 的 provider 事实源是 models.yml,
    // 只有配置过或设为默认的供应商才展示(内置标记仅对确实出现的渠道生效)。

    let mut views = Vec::new();
    for provider_key in keys {
        let models_provider = models_map.get(&provider_key).cloned();
        let is_builtin = is_builtin_provider(&provider_key);
        let is_default = default_provider.as_deref() == Some(provider_key.as_str());
        let is_override = is_builtin && models_provider.is_some();

        let mut sources = Vec::new();
        if is_builtin {
            sources.push(OmpProviderSource::OfficialBuiltin);
        }
        if models_provider.is_some() {
            sources.push(OmpProviderSource::ModelsYml);
        }
        if is_default {
            sources.push(OmpProviderSource::SettingsYml);
        }

        let kind = credential_kind(models_provider.as_ref(), is_builtin);
        let mut categories = Vec::new();
        match kind {
            OmpCredentialKind::ApiKey => categories.push(OmpProviderCategory::ApiKey),
            OmpCredentialKind::Oauth => categories.push(OmpProviderCategory::Subscription),
            OmpCredentialKind::EnvPossible | OmpCredentialKind::None => {}
        }
        if models_provider.is_some() {
            categories.push(OmpProviderCategory::Custom);
        }
        if categories.is_empty() && is_builtin {
            categories.push(OmpProviderCategory::ApiKey);
        }

        let model_ids = model_ids_from_provider(models_provider.as_ref());
        let mut warnings = Vec::new();
        if !is_builtin && models_provider.is_none() {
            warnings.push(OmpProviderWarning::MissingProvider);
        }
        if is_default {
            if let Some(default_model) = default_model.as_deref() {
                if !default_model.trim().is_empty()
                    && !model_ids.is_empty()
                    && !model_ids.iter().any(|id| id == default_model)
                {
                    warnings.push(OmpProviderWarning::MissingModel);
                }
            }
        }

        let mut runtime_files = Vec::new();
        if models_provider.is_some() {
            runtime_files.push(super::constants::OMP_MODELS_FILE.to_string());
        }
        if is_default {
            runtime_files.push(super::constants::OMP_CONFIG_FILE.to_string());
        }

        views.push(OmpRuntimeProviderView {
            display_name: builtin_provider_name(&provider_key)
                .map(str::to_string)
                .or_else(|| {
                    models_provider.as_ref().and_then(|value| {
                        value.get("name").and_then(Value::as_str).map(str::to_string)
                    })
                })
                .unwrap_or_else(|| provider_key.clone()),
            provider_key,
            sources,
            categories,
            credential_kind: kind,
            credential: None,
            models_provider,
            runtime_files,
            is_builtin,
            is_override,
            is_default,
            model_ids,
            warnings,
        });
    }

    views
}

fn builtin_providers() -> Vec<OmpBuiltinProvider> {
    OMP_BUILTIN_PROVIDERS
        .iter()
        .map(|(key, name)| OmpBuiltinProvider {
            key: (*key).to_string(),
            name: (*name).to_string(),
        })
        .collect()
}

fn emit_config_changed<R: Runtime>(app: &tauri::AppHandle<R>, payload: &str) {
    let _ = app.emit("config-changed", payload);
    #[cfg(target_os = "windows")]
    let _ = app.emit("wsl-sync-request-omp", ());
}

#[tauri::command]
pub async fn get_omp_root_path_info(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<OmpPathInfo, String> {
    get_omp_root_path_info_from_db_async(state.db()).await
}

#[tauri::command]
pub async fn get_omp_settings_config(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Option<OmpSettingsConfig>, String> {
    Ok(state
        .db()
        .with_conn(|conn| db_get(conn, DbTable::OhMyPiSettingsConfig, "common"))?
        .map(adapter::settings_from_db_value))
}

/// 归一化 OMP 根目录:WSL 下选中的 `.omp` 目录若非有效运行时布局,
/// 且其 `agent` 子目录是有效布局,则自动落到 `agent` 子目录。与 Pi 的 `.pi` 规则一致。
pub(crate) fn normalize_omp_root_dir(path: &str) -> String {
    if let Some(wsl_info) = runtime_location::parse_wsl_unc_path(path) {
        let linux_path = wsl_info.linux_path.trim_end_matches('/');
        if linux_path.ends_with("/.omp") && should_use_omp_agent_subdirectory(Path::new(path)) {
            let new_linux_path = format!("{}/agent", linux_path);
            return runtime_location::build_windows_unc_path(&wsl_info.distro, &new_linux_path)
                .to_string_lossy()
                .to_string();
        }
    }
    path.to_string()
}

fn should_use_omp_agent_subdirectory(selected_root: &Path) -> bool {
    !contains_omp_runtime_layout(selected_root)
        && contains_omp_runtime_layout(&selected_root.join("agent"))
}

fn contains_omp_runtime_layout(root: &Path) -> bool {
    [
        super::constants::OMP_CONFIG_FILE,
        super::constants::OMP_MODELS_FILE,
        super::constants::OMP_MCP_FILE,
        super::constants::OMP_PROMPT_FILE,
    ]
    .iter()
    .any(|file_name| root.join(file_name).is_file())
        || root.join("extensions").is_dir()
}

#[tauri::command]
pub async fn save_omp_settings_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmpSettingsConfigInput,
) -> Result<(), String> {
    let db = state.db();
    let previous_skills_path = runtime_location::get_tool_skills_path_async(&db, "oh_my_pi").await;
    let existing = get_omp_settings_config(state.clone()).await?;
    let root_dir = if input.clear_root_dir {
        None
    } else {
        input
            .root_dir
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalize_omp_root_dir)
            .or_else(|| existing.and_then(|value| value.root_dir))
    };
    let data = adapter::settings_to_db_value(root_dir.as_deref());
    db.with_conn(|conn| db_put(conn, DbTable::OhMyPiSettingsConfig, "common", &data))?;
    runtime_location::refresh_runtime_location_cache_for_module_async(&db, "oh_my_pi").await?;
    resync_all_skills_if_tool_path_changed(app.clone(), state.inner(), "oh_my_pi", previous_skills_path)
        .await;
    emit_config_changed(&app, "window");
    Ok(())
}

#[tauri::command]
pub async fn read_omp_runtime_config(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<OmpRuntimeConfig, String> {
    let db = state.db();
    let root_path_info = get_omp_root_path_info_from_db_async(&db).await?;
    let root_dir = PathBuf::from(&root_path_info.path);
    let config_path = get_omp_config_path_from_root(&root_dir);
    let models_path = get_omp_models_path_from_root(&root_dir);
    let prompt_path = get_omp_prompt_path_from_root(&root_dir);

    let settings = read_yaml_object_or_empty(&config_path)?;
    let models = read_yaml_object_or_empty(&models_path)?;

    Ok(OmpRuntimeConfig {
        root_path_info,
        config_path: config_path.to_string_lossy().to_string(),
        models_path: models_path.to_string_lossy().to_string(),
        mcp_path: get_omp_mcp_path_from_root(&root_dir)
            .to_string_lossy()
            .to_string(),
        prompt_path: prompt_path.to_string_lossy().to_string(),
        other_settings: build_other_settings(&settings),
        model_settings: default_selection_from_settings(&settings),
        providers: build_provider_views(&settings, &models),
        builtin_providers: builtin_providers(),
        settings,
        models,
    })
}

/// 更新 config.yml 中 `modelRoles.default` 与 `defaultThinkingLevel`。
async fn update_default_selection(
    db: &SqliteDbState,
    provider_key: Option<&str>,
    model_id: Option<&str>,
    thinking_level: Option<&str>,
    remove_thinking_level: bool,
) -> Result<(), String> {
    let config_path = get_omp_config_path_async(db).await?;
    let mut settings = read_yaml_object_or_empty(&config_path)?;
    let settings_object = object_mut(&mut settings)?;

    let next_provider = provider_key
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let next_model = model_id.map(str::trim).filter(|value| !value.is_empty());

    let mut roles = settings_object
        .get("modelRoles")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let (Some(provider), Some(model)) = (next_provider, next_model) {
        roles.insert(OMP_MODEL_ROLE_KEY.to_string(), Value::String(format!("{provider}/{model}")));
    } else {
        roles.remove(OMP_MODEL_ROLE_KEY);
    }
    if roles.is_empty() {
        settings_object.remove("modelRoles");
    } else {
        settings_object.insert("modelRoles".to_string(), Value::Object(roles));
    }

    match thinking_level {
        Some(value) if !value.trim().is_empty() && !remove_thinking_level => {
            settings_object.insert("defaultThinkingLevel".to_string(), json!(value.trim()));
        }
        _ => {
            if remove_thinking_level {
                settings_object.remove("defaultThinkingLevel");
            }
        }
    }

    write_yaml_object(&config_path, &settings)
}

#[tauri::command]
pub async fn save_omp_model_settings(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmpModelSettingsInput,
) -> Result<OmpRuntimeConfig, String> {
    let db = state.db();
    let current = default_selection_from_settings(
        &read_yaml_object_or_empty(&get_omp_config_path_async(&db).await?)?,
    );
    let provider_key = input
        .default_provider
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| current.provider_key.clone());
    let model_id = input
        .default_model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| current.model_id.clone());
    let thinking_level = input
        .default_thinking_level
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| current.thinking_level.clone());

    let should_remove_thinking_level = if let (Some(model), Some(level)) = (model_id.as_deref(), thinking_level.as_deref()) {
        if level == "auto" {
            false
        } else {
            let models = read_yaml_object_or_empty(&get_omp_models_path_async(&db).await?)?;
            // 找一个 provider 下匹配该模型的配置来决定思考级别支持。
            let mut supported = true;
            if let Some(provider_key) = provider_key.as_deref() {
                let provider = models
                    .get("providers")
                    .and_then(Value::as_object)
                    .and_then(|providers| providers.get(provider_key));
                if let Some(provider) = provider {
                    let model_config = provider
                        .get("models")
                        .and_then(Value::as_array)
                        .and_then(|list| {
                            list.iter().find(|m| {
                                m.get("id").and_then(Value::as_str).map(|id| id == model).unwrap_or(false)
                            })
                        });
                    if let Some(model_config) = model_config {
                        supported = model_supports_thinking_level(model_config, level);
                    }
                }
            }
            !supported
        }
    } else {
        false
    };

    update_default_selection(
        &db,
        provider_key.as_deref(),
        model_id.as_deref(),
        thinking_level.as_deref(),
        should_remove_thinking_level,
    )
    .await?;
    emit_config_changed(&app, "window");
    read_omp_runtime_config(state).await
}

pub async fn apply_omp_default_model_internal<R: Runtime>(
    db: &SqliteDbState,
    app: &tauri::AppHandle<R>,
    provider_key: &str,
    model_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    let provider_key = provider_key.trim();
    let model_id = model_id.trim();
    if provider_key.is_empty() {
        return Err("Provider key is required".to_string());
    }
    if model_id.is_empty() {
        return Err("Model id is required".to_string());
    }

    let current = default_selection_from_settings(
        &read_yaml_object_or_empty(&get_omp_config_path_async(db).await?)?,
    );
    update_default_selection(
        db,
        Some(provider_key),
        Some(model_id),
        current.thinking_level.as_deref(),
        false,
    )
    .await?;
    emit_config_changed(app, if from_tray { "tray" } else { "window" });
    Ok(())
}

#[tauri::command]
pub async fn save_omp_other_settings(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    other_settings: Value,
) -> Result<OmpRuntimeConfig, String> {
    if !other_settings.is_object() {
        return Err("OMP other settings must be a mapping object".to_string());
    }

    let db = state.db();
    let config_path = get_omp_config_path_async(&db).await?;
    let mut settings = read_yaml_object_or_empty(&config_path)?;
    let settings_object = object_mut(&mut settings)?;
    apply_omp_other_settings(settings_object, other_settings.as_object().unwrap());

    write_yaml_object(&config_path, &settings)?;
    emit_config_changed(&app, "window");
    read_omp_runtime_config(state).await
}

#[tauri::command]
pub async fn save_omp_models_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmpModelsProviderInput,
) -> Result<OmpRuntimeConfig, String> {
    let provider_key = input.provider_key.trim();
    if provider_key.is_empty() {
        return Err("Provider key is required".to_string());
    }
    if !input.provider.is_object() {
        return Err("OMP models provider config must be a mapping object".to_string());
    }

    let db = state.db();
    let models_path = get_omp_models_path_async(&db).await?;
    let mut models = read_yaml_object_or_empty(&models_path)?;
    let models_object = object_mut(&mut models)?;
    if !models_object
        .get("providers")
        .map(Value::is_object)
        .unwrap_or(false)
    {
        models_object.insert("providers".to_string(), Value::Object(Map::new()));
    }
    let mut provider = input.provider;
    normalize_omp_provider_for_omptype(&mut provider);
    models_object
        .get_mut("providers")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "models.providers must be a mapping object".to_string())?
        .insert(provider_key.to_string(), provider);

    write_yaml_object(&models_path, &models)?;
    emit_config_changed(&app, "window");
    read_omp_runtime_config(state).await
}

#[tauri::command]
pub async fn delete_omp_runtime_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    provider_key: String,
) -> Result<OmpRuntimeConfig, String> {
    let provider_key = provider_key.trim();
    if provider_key.is_empty() {
        return Err("Provider key is required".to_string());
    }
    let db = state.db();
    let models_path = get_omp_models_path_async(&db).await?;
    let mut models = read_yaml_object_or_empty(&models_path)?;
    if let Some(providers) = models.get_mut("providers").and_then(Value::as_object_mut) {
        providers.remove(provider_key);
    }
    write_yaml_object(&models_path, &models)?;

    emit_config_changed(&app, "window");
    read_omp_runtime_config(state).await
}

fn prompt_order() -> Result<OrderSpec, String> {
    Ok(OrderSpec::new(vec![OrderField::json_integer(
        "sort_index",
        OrderDirection::Asc,
    )?]))
}

fn put_omp_prompt_to_sqlite(
    db: &SqliteDbState,
    id: &str,
    content: &OmpPromptConfigContent,
) -> Result<(), String> {
    let value = adapter::prompt_to_db_value(content);
    db.with_conn(|conn| db_put(conn, DbTable::OhMyPiPromptConfig, id, &value))
}

fn get_omp_prompt_from_sqlite(
    db: &SqliteDbState,
    id: &str,
) -> Result<Option<OmpPromptConfig>, String> {
    Ok(db
        .with_conn(|conn| db_get(conn, DbTable::OhMyPiPromptConfig, id))?
        .map(adapter::prompt_from_db_value))
}

async fn get_local_prompt_config(db: &SqliteDbState) -> Result<Option<OmpPromptConfig>, String> {
    let prompt_path = get_omp_prompt_path_async(db).await?;
    if !prompt_path.exists() {
        return Ok(None);
    }
    let Some(content) = read_prompt_content_file(&prompt_path, "OMP")? else {
        return Ok(None);
    };
    Ok(Some(OmpPromptConfig {
        id: "__local__".to_string(),
        name: "Local AGENTS.md".to_string(),
        content,
        is_applied: false,
        sort_index: Some(-1),
        created_at: None,
        updated_at: None,
    }))
}

async fn write_prompt_content_to_file(
    db: &SqliteDbState,
    content: Option<&str>,
) -> Result<(), String> {
    let path = get_omp_prompt_path_async(db).await?;
    write_prompt_content_file(&path, content, "OMP")
}

#[tauri::command]
pub async fn list_omp_prompt_configs(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<OmpPromptConfig>, String> {
    let db = state.db();
    let mut prompts = db.with_conn(|conn| {
        Ok(
            db_list(conn, DbTable::OhMyPiPromptConfig, Some(&prompt_order()?))?
                .into_iter()
                .map(adapter::prompt_from_db_value)
                .collect::<Vec<_>>(),
        )
    })?;
    if !prompts.iter().any(|prompt| prompt.is_applied) {
        if let Some(local_prompt) = get_local_prompt_config(&db).await? {
            prompts.insert(0, local_prompt);
        }
    }
    Ok(prompts)
}

#[tauri::command]
pub async fn create_omp_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmpPromptConfigInput,
) -> Result<OmpPromptConfig, String> {
    let db = state.db();
    let now = Local::now().to_rfc3339();
    let next_sort_index = db.with_conn(|conn| {
        Ok(db_max_i64(
            conn,
            DbTable::OhMyPiPromptConfig,
            &JsonFieldPath::new("sort_index")?,
        )?
        .map(|value| value as i32 + 1)
        .unwrap_or(0))
    })?;
    let content = OmpPromptConfigContent {
        name: input.name,
        content: input.content,
        is_applied: false,
        sort_index: Some(next_sort_index),
        created_at: now.clone(),
        updated_at: now,
    };
    let prompt_id = db_new_id();
    put_omp_prompt_to_sqlite(&db, &prompt_id, &content)?;
    let _ = app.emit("config-changed", "window");
    Ok(adapter::prompt_from_db_value(json!({
        "id": prompt_id,
        "name": content.name,
        "content": content.content,
        "is_applied": content.is_applied,
        "sort_index": content.sort_index,
        "created_at": content.created_at,
        "updated_at": content.updated_at
    })))
}

#[tauri::command]
pub async fn update_omp_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmpPromptConfigInput,
) -> Result<OmpPromptConfig, String> {
    let config_id = input
        .id
        .ok_or_else(|| "ID is required for update".to_string())?;
    let db = state.db();
    let now = Local::now().to_rfc3339();
    let existing = get_omp_prompt_from_sqlite(&db, &config_id)?
        .ok_or_else(|| format!("Prompt config '{}' not found", config_id))?;
    let content = OmpPromptConfigContent {
        name: input.name,
        content: input.content.clone(),
        is_applied: existing.is_applied,
        sort_index: existing.sort_index,
        created_at: existing.created_at.unwrap_or_else(|| now.clone()),
        updated_at: now.clone(),
    };
    put_omp_prompt_to_sqlite(&db, &config_id, &content)?;
    if existing.is_applied {
        write_prompt_content_to_file(&db, Some(input.content.as_str())).await?;
        emit_config_changed(&app, "window");
    } else {
        let _ = app.emit("config-changed", "window");
    }
    get_omp_prompt_from_sqlite(&db, &config_id)?
        .ok_or_else(|| format!("Prompt config '{}' not found after update", config_id))
}

#[tauri::command]
pub async fn delete_omp_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.db();
    db.with_conn(|conn| db_delete(conn, DbTable::OhMyPiPromptConfig, &id).map(|_| ()))?;
    let _ = app.emit("config-changed", "window");
    Ok(())
}

pub async fn apply_omp_prompt_config_internal<R: Runtime>(
    state: tauri::State<'_, SqliteDbState>,
    app: &tauri::AppHandle<R>,
    config_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    let db = state.db();
    if config_id == "__local__" {
        let local_prompt = get_local_prompt_config(&db)
            .await?
            .ok_or_else(|| "Local OMP prompt not found".to_string())?;
        write_prompt_content_to_file(&db, Some(local_prompt.content.as_str())).await?;
        emit_config_changed(app, if from_tray { "tray" } else { "window" });
        return Ok(());
    }

    let prompt = get_omp_prompt_from_sqlite(&db, config_id)?
        .ok_or_else(|| format!("Prompt config '{}' not found", config_id))?;
    let now = Local::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        db_update_applied_status(conn, DbTable::OhMyPiPromptConfig, Some(config_id), &now)
    })?;
    write_prompt_content_to_file(&db, Some(prompt.content.as_str())).await?;
    emit_config_changed(app, if from_tray { "tray" } else { "window" });
    Ok(())
}

pub async fn apply_omp_prompt_config_internal_without_events<R: Runtime>(
    state: tauri::State<'_, SqliteDbState>,
    app: &tauri::AppHandle<R>,
    config_id: &str,
) -> Result<(), String> {
    if config_id == "__local__" {
        let db = state.db();
        let local_prompt = get_local_prompt_config(&db)
            .await?
            .ok_or_else(|| "Local OMP prompt not found".to_string())?;
        write_prompt_content_to_file(&db, Some(local_prompt.content.as_str())).await?;
        return Ok(());
    }

    let prompt = get_omp_prompt_from_sqlite(state.db(), config_id)?
        .ok_or_else(|| format!("Prompt config '{}' not found", config_id))?;
    let now = Local::now().to_rfc3339();
    state.db().with_conn_mut(|conn| {
        db_update_applied_status(conn, DbTable::OhMyPiPromptConfig, Some(config_id), &now)
    })?;
    write_prompt_content_to_file(state.db(), Some(prompt.content.as_str())).await?;
    let _ = app;
    Ok(())
}

#[tauri::command]
pub async fn apply_omp_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    apply_omp_prompt_config_internal(state, &app, &config_id, false).await
}

#[tauri::command]
pub async fn reorder_omp_prompt_configs(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.db();
    for (index, id) in ids.iter().enumerate() {
        db.with_conn(|conn| {
            db_patch_fields(
                conn,
                DbTable::OhMyPiPromptConfig,
                id,
                &[("sort_index", json!(index as i64))],
            )
            .map(|_| ())
        })?;
    }
    let _ = app.emit("config-changed", "window");
    Ok(())
}

#[tauri::command]
pub async fn save_omp_local_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmpPromptConfigInput,
) -> Result<OmpPromptConfig, String> {
    let db = state.db();
    let content = if input.content.trim().is_empty() {
        get_local_prompt_config(&db)
            .await?
            .map(|prompt| prompt.content)
            .unwrap_or_default()
    } else {
        input.content
    };
    let created = create_omp_prompt_config(
        state.clone(),
        app.clone(),
        OmpPromptConfigInput {
            id: None,
            name: input.name,
            content,
        },
    )
    .await?;
    apply_omp_prompt_config_internal(state.clone(), &app, &created.id, false).await?;
    Ok(get_omp_prompt_from_sqlite(state.db(), &created.id)?.unwrap_or(created))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_omptype_drops_incomplete_cost_and_thinking_level_map() {
        let mut provider = json!({
            "api": "openai-responses",
            "baseUrl": "http://127.0.0.1:8787/v1",
            "apiKey": "sk-test",
            "models": [
                { "id": "a", "cost": { "input": 0.1 } },
                {
                    "id": "b",
                    "cost": { "input": 0.1, "output": 0.3, "cacheRead": 0.1, "cacheWrite": 0.2 },
                    "thinkingLevelMap": { "high": "high" }
                }
            ]
        });

        normalize_omp_provider_for_omptype(&mut provider);

        assert_eq!(
            provider["models"][0],
            json!({ "id": "a" })
        );
        assert_eq!(
            provider["models"][1],
            json!({
                "id": "b",
                "cost": { "input": 0.1, "output": 0.3, "cacheRead": 0.1, "cacheWrite": 0.2 }
            })
        );
    }

    #[test]
    fn thinking_level_map_only_defaults_standard_omitted_levels_to_supported() {
        let model = json!({
            "id": "deepseek-v4-pro",
            "reasoning": true,
            "thinkingLevelMap": {
                "minimal": null,
                "low": null,
                "medium": null,
                "high": "high",
                "xhigh": "max"
            }
        });

        assert!(model_supports_thinking_level(&model, "off"));
        assert!(model_supports_thinking_level(&model, "high"));
        assert!(model_supports_thinking_level(&model, "xhigh"));
        assert!(model_supports_thinking_level(&model, "auto"));
        assert!(!model_supports_thinking_level(&model, "max"));
        assert!(!model_supports_thinking_level(&model, "minimal"));
        assert!(!model_supports_thinking_level(&model, "unknown"));
    }

    #[test]
    fn split_provider_model_handles_slashed_model_ids() {
        let (provider, model) = split_provider_model("openrouter/openai/gpt-5");
        assert_eq!(provider.as_deref(), Some("openrouter"));
        assert_eq!(model.as_deref(), Some("openai/gpt-5"));

        let (provider, model) = split_provider_model("anthropic/claude-sonnet-4");
        assert_eq!(provider.as_deref(), Some("anthropic"));
        assert_eq!(model.as_deref(), Some("claude-sonnet-4"));

        let (provider, model) = split_provider_model("bare");
        assert_eq!(provider, None);
        assert_eq!(model, None);
    }

    #[test]
    fn default_selection_reads_model_roles_and_thinking() {
        let settings = json!({
            "modelRoles": {
                "default": "anthropic/claude-sonnet-4",
                "smol": "openai/gpt-5-mini"
            },
            "defaultThinkingLevel": "medium"
        });
        let selection = default_selection_from_settings(&settings);
        assert_eq!(selection.provider_key.as_deref(), Some("anthropic"));
        assert_eq!(selection.model_id.as_deref(), Some("claude-sonnet-4"));
        assert_eq!(selection.thinking_level.as_deref(), Some("medium"));
    }

    #[test]
    fn build_other_settings_excludes_managed_model_keys() {
        let settings = json!({
            "modelRoles": {"default": "anthropic/claude-sonnet-4"},
            "defaultThinkingLevel": "high",
            "extensions": ["./extensions"],
            "theme": {"dark": true},
            "compaction.enabled": false
        });

        let other = build_other_settings(&settings);
        assert_eq!(other, json!({
            "theme": {"dark": true},
            "compaction.enabled": false
        }));
    }

    #[test]
    fn apply_omp_other_settings_preserves_managed_keys() {
        let mut settings = json!({
            "modelRoles": {"default": "anthropic/claude-sonnet-4"},
            "defaultThinkingLevel": "high",
            "extensions": ["./extensions"],
            "theme": {"dark": true}
        });
        let other_settings = json!({
            "theme": {"dark": false},
            "compaction.enabled": true
        });

        apply_omp_other_settings(
            settings.as_object_mut().expect("settings object"),
            other_settings.as_object().expect("other settings object"),
        );

        assert_eq!(
            settings,
            json!({
                "modelRoles": {"default": "anthropic/claude-sonnet-4"},
                "defaultThinkingLevel": "high",
                "extensions": ["./extensions"],
                "theme": {"dark": false},
                "compaction.enabled": true
            })
        );
    }

    #[test]
    fn yaml_round_trip_preserves_unknown_fields() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir
            .path()
            .join(crate::coding::oh_my_pi::constants::OMP_MODELS_FILE);
        let value = json!({
            "providers": {
                "gateway": {
                    "baseUrl": "https://example.com/v1",
                    "api": "openai-responses",
                    "customField": { "enabled": true },
                    "models": [{ "id": "gpt-test", "unknown": 42 }]
                }
            },
            "futureRootField": true
        });
        write_yaml_object(&path, &value).expect("write yaml");
        let read_back = read_yaml_object_or_empty(&path).expect("read yaml");
        let _ = fs::remove_file(&path);
        assert_eq!(read_back, value);
    }

    #[test]
    fn normalize_omp_root_dir_preserves_non_wsl_path() {
        let path = r"C:\Users\tester\.omp";
        assert_eq!(normalize_omp_root_dir(path), path);
    }

    #[test]
    fn normalize_omp_root_dir_preserves_wsl_path_not_ending_with_dot_omp() {
        let path = r"\\wsl.localhost\Ubuntu\home\tester\custom-agent";
        assert_eq!(normalize_omp_root_dir(path), path);
    }
}