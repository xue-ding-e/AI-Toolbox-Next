use tauri::{AppHandle, Manager, Runtime};

use super::types::PiProviderWarning;

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

fn find_model_display_name(items: &[TrayModelItem], current: &str) -> String {
    if current.trim().is_empty() {
        return String::new();
    }
    items
        .iter()
        .find(|item| item.is_selected)
        .and_then(|item| item.display_name.split(" / ").nth(1).map(str::to_string))
        .unwrap_or_else(|| current.to_string())
}

pub async fn get_pi_tray_data<R: Runtime>(app: &AppHandle<R>) -> Result<TrayModelData, String> {
    let runtime_config = super::commands::read_pi_runtime_config(app.state()).await?;
    let current_provider = runtime_config
        .model_settings
        .provider_key
        .clone()
        .unwrap_or_default();
    let current_model = runtime_config
        .model_settings
        .model_id
        .clone()
        .unwrap_or_default();
    let mut items = Vec::new();

    for provider in runtime_config.providers {
        let is_provider_missing = provider
            .warnings
            .iter()
            .any(|warning| matches!(warning, PiProviderWarning::MissingProvider));
        for model_id in &provider.model_ids {
            let is_selected =
                provider.provider_key == current_provider && model_id == &current_model;
            items.push(TrayModelItem {
                id: format!("{}/{}", provider.provider_key, model_id),
                display_name: format!("{} / {}", provider.display_name, model_id),
                is_selected,
                is_disabled: is_provider_missing,
            });
        }

        if provider.provider_key == current_provider
            && !current_model.trim().is_empty()
            && !items.iter().any(|item| item.is_selected)
        {
            items.push(TrayModelItem {
                id: format!("{}/{}", provider.provider_key, current_model),
                display_name: format!("{} / {}", provider.display_name, current_model),
                is_selected: true,
                is_disabled: is_provider_missing,
            });
        }
    }

    items.sort_by(|left, right| {
        right
            .is_selected
            .cmp(&left.is_selected)
            .then_with(|| left.display_name.cmp(&right.display_name))
    });
    let current_display = find_model_display_name(&items, &current_model);

    Ok(TrayModelData {
        title: "默认模型".to_string(),
        current_display,
        items,
    })
}

pub async fn apply_pi_model<R: Runtime>(
    app: &AppHandle<R>,
    provider_key: &str,
    model_id: &str,
) -> Result<(), String> {
    let state = app.state::<crate::db::SqliteDbState>();
    let db = state.db();
    super::commands::apply_pi_default_model_internal(&db, app, provider_key, model_id, true).await
}

pub async fn is_enabled_for_tray<R: Runtime>(_app: &AppHandle<R>) -> bool {
    true
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

fn find_prompt_display_name(items: &[TrayPromptItem]) -> String {
    items
        .iter()
        .find(|item| item.is_selected)
        .map(|item| item.display_name.clone())
        .unwrap_or_default()
}

pub async fn get_pi_prompt_tray_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TrayPromptData, String> {
    let configs = super::commands::list_pi_prompt_configs(app.state()).await?;
    let items = configs
        .into_iter()
        .filter(|config| config.id != "__local__")
        .map(|config| TrayPromptItem {
            id: config.id,
            display_name: config.name,
            is_selected: config.is_applied,
        })
        .collect::<Vec<_>>();

    Ok(TrayPromptData {
        title: "全局提示词".to_string(),
        current_display: find_prompt_display_name(&items),
        items,
    })
}

pub async fn apply_pi_prompt_config<R: Runtime>(
    app: &AppHandle<R>,
    config_id: &str,
) -> Result<(), String> {
    super::commands::apply_pi_prompt_config_internal(app.state(), app, config_id, true).await
}
