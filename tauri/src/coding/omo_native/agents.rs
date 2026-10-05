//! Native agents/categories 的名单与模型档辅助。
//!
//! 名单常量在 [`super::constants`]，这里提供校验与读取辅助。

use serde_json::Value;

use super::constants;
use super::types::OmoNativeModelProfile;

/// 判断 agent 名是否是 Native 内建（内建可被同名覆盖，覆盖是逐字段叠加）。
pub fn is_builtin_agent(name: &str) -> bool {
    constants::OMO_NATIVE_AGENTS.contains(&name)
}

/// 判断 category 名是否是 Native 内建。
pub fn is_builtin_category(name: &str) -> bool {
    constants::OMO_NATIVE_CATEGORIES.contains(&name)
}

/// 内建 agent 名单（前端下拉用）。
#[tauri::command]
pub async fn list_omo_native_builtin_agents() -> Result<Vec<String>, String> {
    Ok(constants::OMO_NATIVE_AGENTS
        .iter()
        .map(|name| name.to_string())
        .collect())
}

/// 内建 category 名单（前端下拉用）。
#[tauri::command]
pub async fn list_omo_native_builtin_categories() -> Result<Vec<String>, String> {
    Ok(constants::OMO_NATIVE_CATEGORIES
        .iter()
        .map(|name| name.to_string())
        .collect())
}

/// 内建模型档 + 用户自定义模型档（`model_profiles` 键）。
#[tauri::command]
pub async fn list_omo_native_model_profiles(
    state: tauri::State<'_, crate::db::SqliteDbState>,
) -> Result<Vec<OmoNativeModelProfile>, String> {
    let db = state.db();
    let config_path = super::commands::get_omo_native_config_path_async(&db).await?;
    let config = if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|content| json5::from_str::<Value>(&content).ok())
            .unwrap_or_else(|| Value::Object(serde_json::Map::new()))
    } else {
        Value::Object(serde_json::Map::new())
    };

    // `model_profiles` 是共享 base 键，`[native]` 块可覆盖它。
    let profiles_value = config
        .get(constants::OMO_NATIVE_HARNESS_BLOCK)
        .and_then(|block| block.get("model_profiles"))
        .or_else(|| config.get("model_profiles"));

    let mut profiles: Vec<OmoNativeModelProfile> = constants::OMO_NATIVE_BUILTIN_MODEL_PROFILE_IDS
        .iter()
        .map(|(id, display_name)| OmoNativeModelProfile {
            name: (*id).to_string(),
            display_name: Some((*display_name).to_string()),
            family: None,
            tier: None,
            models: None,
            builtin: true,
        })
        .collect();

    if let Some(map) = profiles_value.and_then(Value::as_object) {
        for (name, entry) in map {
            let display_name = entry
                .get("display_name")
                .and_then(Value::as_str)
                .map(str::to_string);
            let family = entry.get("family").and_then(Value::as_str).map(str::to_string);
            let tier = entry.get("tier").and_then(Value::as_str).map(str::to_string);
            let models = entry.get("models").cloned();

            match profiles.iter_mut().find(|profile| profile.name == *name) {
                // 同名条目**整体替换**内建档，不是逐字段合并（上游语义）。
                Some(existing) => {
                    existing.display_name = display_name;
                    existing.family = family;
                    existing.tier = tier;
                    existing.models = models;
                }
                None => profiles.push(OmoNativeModelProfile {
                    name: name.clone(),
                    display_name,
                    family,
                    tier,
                    models,
                    builtin: false,
                }),
            }
        }
    }

    Ok(profiles)
}
