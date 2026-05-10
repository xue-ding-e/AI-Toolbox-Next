use crate::coding::db_id::db_clean_id;
use crate::db::DbState;
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

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

pub async fn get_gemini_cli_tray_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TrayProviderData, String> {
    let state = app.state::<DbState>();
    let db = state.db();
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM gemini_cli_provider")
        .await
        .map_err(|error| format!("Failed to query Gemini CLI providers: {}", error))?
        .take(0);
    let mut items = Vec::new();
    if let Ok(records) = records_result {
        for record in records {
            if let (Some(raw_id), Some(name), Some(is_applied), sort_index) = (
                record.get("id").and_then(Value::as_str),
                record.get("name").and_then(Value::as_str),
                record
                    .get("is_applied")
                    .or_else(|| record.get("isApplied"))
                    .and_then(Value::as_bool),
                record
                    .get("sort_index")
                    .or_else(|| record.get("sortIndex"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
            ) {
                let is_disabled = record
                    .get("is_disabled")
                    .or_else(|| record.get("isDisabled"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                items.push(TrayProviderItem {
                    id: db_clean_id(raw_id),
                    display_name: name.to_string(),
                    is_selected: is_applied,
                    is_disabled,
                    sort_index,
                });
            }
        }
    }
    items.sort_by_key(|item| item.sort_index);
    Ok(TrayProviderData {
        title: "──── Gemini CLI ────".to_string(),
        items,
    })
}

pub async fn apply_gemini_cli_provider<R: Runtime>(
    app: &AppHandle<R>,
    provider_id: &str,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.db();
    super::commands::apply_config_internal(&db, app, provider_id, true).await
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

pub async fn get_gemini_cli_prompt_tray_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TrayPromptData, String> {
    let configs = super::commands::list_gemini_cli_prompt_configs(app.state()).await?;
    let items: Vec<TrayPromptItem> = configs
        .into_iter()
        .filter(|config| config.id != "__local__")
        .map(|config| TrayPromptItem {
            id: config.id,
            display_name: config.name,
            is_selected: config.is_applied,
        })
        .collect();
    Ok(TrayPromptData {
        title: "全局提示词".to_string(),
        current_display: find_prompt_display_name(&items),
        items,
    })
}

pub async fn apply_gemini_cli_prompt_config<R: Runtime>(
    app: &AppHandle<R>,
    config_id: &str,
) -> Result<(), String> {
    super::commands::apply_prompt_config_internal(app.state(), app, config_id, true).await
}
