//! OpenCode Tray Support Module
//!
//! Provides standardized API for tray menu integration.
//! This module handles all data fetching and processing for tray menu display.

use crate::coding::open_code::free_models;
use crate::coding::open_code::types::ReadConfigResult;
use crate::coding::open_code::{read_opencode_config, OpenCodeConfig};
use tauri::{AppHandle, Manager, Runtime};

/// Helper to extract OpenCodeConfig from ReadConfigResult, returning default config for non-success cases
fn extract_config_or_default(result: ReadConfigResult) -> OpenCodeConfig {
    match result {
        ReadConfigResult::Success { config } => config,
        _ => OpenCodeConfig {
            schema: None,
            provider: Some(std::collections::HashMap::new()),
            model: None,
            small_model: None,
            plugin: None,
            other: serde_json::Map::new(),
        },
    }
}

/// Item for model selection in tray menu
#[derive(Debug, Clone)]
pub struct TrayModelItem {
    /// Unique identifier for the model (used in event handling)
    pub id: String,
    /// Display name in menu (format: "provider_name / model_name")
    pub display_name: String,
    /// Whether this model is currently selected
    pub is_selected: bool,
}

/// Data for a model submenu
#[derive(Debug, Clone)]
pub struct TrayModelData {
    /// Title of the submenu (e.g., "主模型")
    pub title: String,
    /// Currently selected model display name (shown in parentheses)
    pub current_display: String,
    /// List of available models
    pub items: Vec<TrayModelItem>,
}

/// Get tray model data for both main and small models
pub async fn get_opencode_tray_model_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(TrayModelData, TrayModelData), String> {
    let result = read_opencode_config(app.state()).await?;
    let config = extract_config_or_default(result);

    let current_main = config.model.as_ref().map(|s: &String| s.as_str()).unwrap_or("");
    let current_small = config.small_model.as_ref().map(|s: &String| s.as_str()).unwrap_or("");

    // Build items list
    let mut items: Vec<TrayModelItem> = Vec::new();
    if let Some(providers) = config.provider {
        for (provider_id, provider) in providers {
            let provider_name = provider.name.as_deref().unwrap_or(&provider_id);
            for (model_id, model) in provider.models.iter() {
                let model_name = model.name.as_deref().unwrap_or(&model_id);
                let item_id = format!("{}/{}", provider_id, model_id);
                let display_name = format!("{} / {}", provider_name, model_name);

                items.push(TrayModelItem {
                    id: item_id,
                    display_name,
                    is_selected: false,
                });
            }
        }
    }

    // Add free models from opencode channel
    match free_models::get_free_models(&*app.state(), false).await {
        Ok((free_models, _, _)) => {
            for free_model in free_models {
                let item_id = format!("{}/{}", free_model.provider_id, free_model.id);
                let display_name = format!("{} / {} (Free)", free_model.provider_name, free_model.name);

                items.push(TrayModelItem {
                    id: item_id,
                    display_name,
                    is_selected: false,
                });
            }
        }
        Err(e) => {
            eprintln!("Failed to load free models for tray: {}", e);
        }
    }

    // Sort by display name
    items.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    // Remove duplicates
    items.dedup_by(|a, b| a.id == b.id);

    // Find current selections - create separate clones for each model type
    let main_items: Vec<TrayModelItem> = items
        .iter()
        .map(|item| TrayModelItem {
            id: item.id.clone(),
            display_name: item.display_name.clone(),
            is_selected: current_main == item.id,
        })
        .collect();

    let small_items: Vec<TrayModelItem> = items
        .iter()
        .map(|item| TrayModelItem {
            id: item.id.clone(),
            display_name: item.display_name.clone(),
            is_selected: current_small == item.id,
        })
        .collect();

    // Extract current display names
    let main_display = find_model_display_name(&main_items, current_main);
    let small_display = find_model_display_name(&small_items, current_small);

    let main_data = TrayModelData {
        title: "主模型".to_string(),
        current_display: main_display,
        items: main_items,
    };

    let small_data = TrayModelData {
        title: "小模型".to_string(),
        current_display: small_display,
        items: small_items,
    };

    Ok((main_data, small_data))
}

/// Helper to find display name for current selection
fn find_model_display_name(items: &[TrayModelItem], current: &str) -> String {
    if current.is_empty() {
        return String::new();
    }

    // item.id format: "provider_id/model_id"
    // current format: "provider_id/model_id" (from config)
    for item in items {
        if item.id == current {
            // Extract just the model name from display_name (format: "provider_name / model_name")
            if let Some(model_name) = item.display_name.split(" / ").nth(1) {
                return model_name.to_string();
            }
            return item.display_name.clone();
        }
    }

    // Fallback: if not found, extract model_id from current value
    if let Some(slash_pos) = current.rfind('/') {
        let model_part = &current[slash_pos + 1..];
        return model_part.to_string();
    }

    current.to_string()
}

/// Apply model selection from tray menu
pub async fn apply_opencode_model<R: Runtime>(
    app: &AppHandle<R>,
    model_type: &str, // "main" or "small"
    item_id: &str,    // Format: "provider/model"
) -> Result<(), String> {
    // Parse item_id to get provider_id and model_id
    let parts: Vec<&str> = item_id.split('/').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid model ID format: {}", item_id));
    }

    let provider_id = parts[0];
    let model_id = parts[1];

    // Read current config
    let result = read_opencode_config(app.state()).await?;
    let mut config = extract_config_or_default(result);

    // Build new config value: "provider_id/model_id" format
    let new_model_value = format!("{}/{}", provider_id, model_id);

    // Update config
    if model_type == "main" {
        config.model = Some(new_model_value);
    } else if model_type == "small" {
        config.small_model = Some(new_model_value);
    } else {
        return Err(format!("Invalid model type: {}", model_type));
    }

    // Save config from tray (will emit "tray" event)
    super::commands::apply_config_internal(app.state(), app, config, true).await?;

    Ok(())
}

/// Check if OpenCode models should be shown in tray menu
/// Returns true - OpenCode models are always visible as a core feature
pub async fn is_enabled_for_tray<R: Runtime>(_app: &AppHandle<R>) -> bool {
    true
}
