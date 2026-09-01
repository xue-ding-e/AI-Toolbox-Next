use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use super::constants::KIMI_LOCAL_PROVIDER_ID;

#[derive(Debug, Clone)]
pub struct TrayProviderItem {
    pub id: String,
    pub display_name: String,
    pub is_selected: bool,
    pub is_disabled: bool,
    pub sort_index: i64,
}
#[derive(Debug, Clone)]
pub struct TrayProviderData {
    pub title: String,
    pub items: Vec<TrayProviderItem>,
}
#[derive(Debug, Clone)]
pub struct TrayModelItem {
    pub id: String,
    pub display_name: String,
    pub is_selected: bool,
    pub is_disabled: bool,
}
#[derive(Debug, Clone)]
pub struct TrayModelData {
    pub title: String,
    pub current_display: String,
    pub items: Vec<TrayModelItem>,
}
#[derive(Debug, Clone)]
pub struct TrayPromptItem {
    pub id: String,
    pub display_name: String,
    pub is_selected: bool,
}
#[derive(Debug, Clone)]
pub struct TrayPromptData {
    pub title: String,
    pub current_display: String,
    pub items: Vec<TrayPromptItem>,
}

pub async fn get_kimi_tray_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TrayProviderData, String> {
    let mut items = super::commands::list_kimi_providers(app.state())
        .await?
        .into_iter()
        .filter(|provider| provider.id != KIMI_LOCAL_PROVIDER_ID)
        .map(|provider| TrayProviderItem {
            id: provider.id,
            display_name: provider.name,
            is_selected: provider.is_applied,
            is_disabled: provider.is_disabled,
            sort_index: provider.sort_index.unwrap_or(0) as i64,
        })
        .collect::<Vec<_>>();
    items.sort_by_key(|item| item.sort_index);
    Ok(TrayProviderData {
        title: "Kimi".to_string(),
        items,
    })
}

pub async fn apply_kimi_provider<R: Runtime>(
    app: &AppHandle<R>,
    provider_id: &str,
) -> Result<(), String> {
    let state = app.state::<crate::db::SqliteDbState>();
    super::commands::select_kimi_provider_internal_with_sync(
        state.inner(),
        app,
        provider_id,
        true,
        true,
    )
    .await
}

fn model_items_from_provider(settings_config: &str) -> Result<TrayModelData, String> {
    let settings: Value = serde_json::from_str(settings_config)
        .map_err(|error| format!("Invalid Kimi provider settings JSON: {error}"))?;
    let default_model_key = settings
        .get("defaultModelKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    let mut items = settings
        .pointer("/modelCatalog/models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let key = model
                .get("key")
                .or_else(|| model.get("model"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            let display_name = model
                .get("displayName")
                .or_else(|| model.get("name"))
                .or_else(|| model.get("model"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(key);
            Some(TrayModelItem {
                id: key.to_string(),
                display_name: display_name.to_string(),
                is_selected: key == default_model_key,
                is_disabled: false,
            })
        })
        .collect::<Vec<_>>();
    // Official template rows keep no client-side catalog; still offer the
    // resolved default so the tray model menu is not empty.
    if items.is_empty() && !default_model_key.is_empty() {
        items.push(TrayModelItem {
            display_name: default_model_key
                .rsplit('/')
                .next()
                .unwrap_or(default_model_key)
                .to_string(),
            id: default_model_key.to_string(),
            is_selected: true,
            is_disabled: false,
        });
    }
    items.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    let current_display = items
        .iter()
        .find(|item| item.is_selected)
        .map(|item| item.display_name.clone())
        .unwrap_or_else(|| default_model_key.to_string());
    Ok(TrayModelData {
        title: "Main Model".to_string(),
        current_display,
        items,
    })
}

pub async fn get_kimi_model_tray_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TrayModelData, String> {
    let applied_provider = super::commands::list_kimi_providers_for_db(
        app.state::<crate::db::SqliteDbState>().inner(),
    )?
    .into_iter()
    .find(|provider| provider.is_applied);
    let Some(provider) = applied_provider else {
        return Ok(TrayModelData {
            title: "Main Model".to_string(),
            current_display: String::new(),
            items: Vec::new(),
        });
    };
    let mut data = model_items_from_provider(&provider.settings_config)?;
    // Model switching rewrites the live config.toml and is rejected while the
    // gateway owns it; mirror the gate in the tray by disabling the items.
    if super::commands::kimi_gateway_takeover_active(app) {
        for item in &mut data.items {
            item.is_disabled = true;
        }
    }
    Ok(data)
}

pub async fn apply_kimi_model<R: Runtime>(
    app: &AppHandle<R>,
    model_key: &str,
) -> Result<(), String> {
    let state = app.state::<crate::db::SqliteDbState>();
    super::commands::select_kimi_model_internal(state.inner(), app, model_key).await
}

pub async fn get_kimi_prompt_tray_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TrayPromptData, String> {
    let items = super::commands::list_kimi_prompt_configs(app.state())
        .await?
        .into_iter()
        // The local prompt placeholder read from the live AGENTS.md reuses the
        // "__local__" sentinel id (get_local_prompt_config), so it is excluded
        // from the tray like the __local__ provider placeholder above.
        .filter(|item| item.id != KIMI_LOCAL_PROVIDER_ID)
        .map(|item| TrayPromptItem {
            id: item.id,
            display_name: item.name,
            is_selected: item.is_applied,
        })
        .collect::<Vec<_>>();
    let current_display = items
        .iter()
        .find(|item| item.is_selected)
        .map(|item| item.display_name.clone())
        .unwrap_or_default();
    Ok(TrayPromptData {
        title: "Global Prompt".to_string(),
        current_display,
        items,
    })
}

pub async fn apply_kimi_prompt_config<R: Runtime>(
    app: &AppHandle<R>,
    config_id: &str,
) -> Result<(), String> {
    let state = app.state::<crate::db::SqliteDbState>();
    super::commands::apply_kimi_prompt_config_internal_from_tray(state.inner(), app, config_id)
        .await
}

pub async fn is_enabled_for_tray<R: Runtime>(_app: &AppHandle<R>) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_items_use_applied_default_and_preserve_display_names() {
        let data = model_items_from_provider(
            &serde_json::json!({
                "defaultModelKey": "kimi-code/k3",
                "modelCatalog": { "models": [
                    { "key": "kimi-code/k3", "model": "k3", "displayName": "K3" },
                    { "key": "kimi-code/kimi-for-coding", "model": "kimi-for-coding", "displayName": "Kimi For Coding" }
                ]}
            })
            .to_string(),
        )
        .expect("build model tray data");

        assert_eq!(data.current_display, "K3");
        assert_eq!(data.items.len(), 2);
        assert!(data
            .items
            .iter()
            .any(|item| item.id == "kimi-code/k3" && item.is_selected));
    }
}
