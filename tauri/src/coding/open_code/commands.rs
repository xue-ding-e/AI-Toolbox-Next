use std::fs;
use std::path::Path;

use super::types::*;

// ============================================================================
// OpenCode Commands
// ============================================================================

/// Get OpenCode config file path
/// Priority: ~/.config/opencode/opencode.json(c)
#[tauri::command]
pub fn get_opencode_config_path() -> Result<String, String> {
    let home_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "Failed to get home directory".to_string())?;

    let config_dir = Path::new(&home_dir).join(".config").join("opencode");

    // Check for .json first, then .jsonc
    let json_path = config_dir.join("opencode.json");
    let jsonc_path = config_dir.join("opencode.jsonc");

    if json_path.exists() {
        Ok(json_path.to_string_lossy().to_string())
    } else if jsonc_path.exists() {
        Ok(jsonc_path.to_string_lossy().to_string())
    } else {
        // Return default path for new file
        Ok(json_path.to_string_lossy().to_string())
    }
}

/// Read OpenCode configuration file
#[tauri::command]
pub async fn read_opencode_config() -> Result<Option<OpenCodeConfig>, String> {
    let config_path_str = get_opencode_config_path()?;
    let config_path = Path::new(&config_path_str);

    if !config_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let mut config: OpenCodeConfig = json5::from_str(&content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;

    // Fill missing name fields with provider key
    // Fill missing npm fields with smart default based on provider key/name
    for (key, provider) in config.provider.iter_mut() {
        if provider.name.is_none() {
            provider.name = Some(key.clone());
        }
        if provider.npm.is_none() {
            // Smart npm inference based on provider key or name (case-insensitive)
            let key_lower = key.to_lowercase();
            let name_lower = provider.name.as_ref().map(|n| n.to_lowercase()).unwrap_or_default();
            
            let inferred_npm = if key_lower.contains("google") || key_lower.contains("gemini")
                || name_lower.contains("google") || name_lower.contains("gemini")
            {
                "@ai-sdk/google"
            } else if key_lower.contains("anthropic") || key_lower.contains("claude")
                || name_lower.contains("anthropic") || name_lower.contains("claude")
            {
                "@ai-sdk/anthropic"
            } else {
                "@ai-sdk/openai-compatible"
            };
            
            provider.npm = Some(inferred_npm.to_string());
        }
    }

    Ok(Some(config))
}

/// Save OpenCode configuration file
#[tauri::command]
pub async fn save_opencode_config(config: OpenCodeConfig) -> Result<(), String> {
    let config_path_str = get_opencode_config_path()?;
    let config_path = Path::new(&config_path_str);

    // Ensure directory exists
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
    }

    // Serialize with pretty printing
    let json_content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(config_path, json_content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(())
}
