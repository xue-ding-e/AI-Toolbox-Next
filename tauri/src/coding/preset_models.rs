use crate::db::SqliteDbState;
use crate::http_client;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

const CACHE_FILE_NAME: &str = "preset_models.json";

/// Bundled preset models JSON (compile-time embedded from resources/)
const DEFAULT_PRESET_MODELS_JSON: &str = include_str!("../../resources/preset_models.json");

/// App data directory path, set once at startup by lib.rs
static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

// ============================================================================
// Cache directory management
// ============================================================================

/// Set the cache directory (called once from lib.rs at startup)
pub fn set_cache_dir(dir: PathBuf) {
    let _ = CACHE_DIR.set(dir);
}

fn get_cache_file_path() -> Option<PathBuf> {
    CACHE_DIR.get().map(|dir| dir.join(CACHE_FILE_NAME))
}

/// Public getter for the cache file path (used by backup/restore)
pub fn get_preset_models_cache_path() -> Option<PathBuf> {
    get_cache_file_path()
}

// ============================================================================
// Bundled defaults
// ============================================================================

fn get_bundled_preset_models() -> Option<Value> {
    let data: Value = serde_json::from_str(DEFAULT_PRESET_MODELS_JSON).ok()?;
    if is_valid_preset_models(&data) {
        Some(data)
    } else {
        None
    }
}

// ============================================================================
// File-based cache read / write
// ============================================================================

fn read_cache_file() -> Option<Value> {
    let path = get_cache_file_path()?;
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Atomic write: write to .tmp then rename
fn write_cache_file(data: &Value) -> Result<(), String> {
    let path =
        get_cache_file_path().ok_or_else(|| "Cache directory not initialized".to_string())?;

    let tmp_path = path.with_extension("json.tmp");

    let json = serde_json::to_string(data)
        .map_err(|e| format!("Failed to serialize preset models cache: {}", e))?;

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create cache directory: {}", e))?;
        }
    }

    fs::write(&tmp_path, json).map_err(|e| format!("Failed to write tmp cache file: {}", e))?;
    fs::rename(&tmp_path, &path).map_err(|e| format!("Failed to rename tmp cache file: {}", e))?;

    Ok(())
}

/// Validate that the JSON looks like a preset models map
/// (non-empty object with at least one key).
fn is_valid_preset_models(data: &Value) -> bool {
    data.as_object().map(|m| !m.is_empty()).unwrap_or(false)
}

// ============================================================================
// Tauri commands
// ============================================================================

/// Load preset models: local cache first, then bundled defaults as fallback.
#[tauri::command]
pub fn load_cached_preset_models() -> Result<Option<Value>, String> {
    // Try local cache first
    if let Some(data) = read_cache_file() {
        if is_valid_preset_models(&data) {
            return Ok(Some(data));
        }
    }
    // Fallback to bundled defaults
    Ok(get_bundled_preset_models())
}

/// Fetch preset models JSON from a remote URL, save to local cache,
/// and return the data to the frontend.
#[tauri::command]
pub async fn fetch_remote_preset_models(
    state: tauri::State<'_, SqliteDbState>,
    url: String,
) -> Result<Value, String> {
    let client = http_client::client_with_timeout(&state, 30).await?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch remote preset models: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Remote preset models request failed: {}",
            response.status()
        ));
    }

    let json: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse remote preset models JSON: {}", e))?;

    // Only cache valid data
    if !is_valid_preset_models(&json) {
        return Err("Remote preset models JSON is empty or invalid".to_string());
    }

    // Save to local cache file
    if let Err(e) = write_cache_file(&json) {
        log::warn!("[PresetModels] Failed to write cache: {}", e);
    } else {
        log::info!("[PresetModels] Cache updated from remote");
    }

    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_PRESET_MODELS_JSON;
    use serde_json::Value;

    const ADAPTIVE_EFFORT_LEVELS: [&str; 4] = ["low", "medium", "high", "max"];
    const EXTENDED_ADAPTIVE_EFFORT_LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
    const LEGACY_THINKING_LEVELS: [(&str, u64); 3] =
        [("low", 5_000), ("medium", 13_000), ("high", 18_000)];

    fn bundled_anthropic_models() -> Value {
        let presets: Value = serde_json::from_str(DEFAULT_PRESET_MODELS_JSON)
            .expect("bundled preset models JSON should parse");
        presets
            .get("@ai-sdk/anthropic")
            .cloned()
            .expect("Anthropic preset group should exist")
    }

    fn model<'a>(models: &'a Value, model_id: &str) -> &'a Value {
        models
            .as_array()
            .expect("Anthropic preset group should be an array")
            .iter()
            .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
            .unwrap_or_else(|| panic!("Anthropic preset model {model_id} should exist"))
    }

    fn assert_adaptive_variants(
        models: &Value,
        model_id: &str,
        effort_levels: &[&str],
        summarized: bool,
    ) {
        let variants = model(models, model_id)
            .get("variants")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{model_id} should define variants"));

        assert_eq!(
            variants.len(),
            effort_levels.len(),
            "{model_id} should expose exactly the supported effort levels"
        );

        for effort_level in effort_levels {
            let variant = variants
                .get(*effort_level)
                .unwrap_or_else(|| panic!("{model_id} should define the {effort_level} variant"));
            assert_eq!(
                variant.get("effort").and_then(Value::as_str),
                Some(*effort_level)
            );

            let thinking = variant
                .get("thinking")
                .and_then(Value::as_object)
                .unwrap_or_else(|| {
                    panic!("{model_id}/{effort_level} should enable adaptive thinking")
                });
            assert_eq!(
                thinking.get("type").and_then(Value::as_str),
                Some("adaptive")
            );
            assert!(
                thinking.get("budgetTokens").is_none(),
                "{model_id}/{effort_level} must not retain a fixed thinking budget"
            );
            assert!(
                thinking.get("effort").is_none(),
                "{model_id}/{effort_level} effort must be a sibling of thinking"
            );

            if summarized {
                assert_eq!(
                    thinking.get("display").and_then(Value::as_str),
                    Some("summarized"),
                    "{model_id}/{effort_level} should request visible thinking summaries"
                );
            } else {
                assert!(thinking.get("display").is_none());
            }
        }
    }

    fn assert_legacy_thinking_variants(models: &Value, model_id: &str) {
        let variants = model(models, model_id)
            .get("variants")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{model_id} should define variants"));

        assert_eq!(variants.len(), LEGACY_THINKING_LEVELS.len());
        for (variant_name, budget_tokens) in LEGACY_THINKING_LEVELS {
            let variant = variants
                .get(variant_name)
                .unwrap_or_else(|| panic!("{model_id} should define the {variant_name} variant"));
            assert!(
                variant.get("effort").is_none(),
                "{model_id}/{variant_name} should use a fixed thinking budget, not effort"
            );
            assert_eq!(
                variant.pointer("/thinking/type").and_then(Value::as_str),
                Some("enabled")
            );
            assert_eq!(
                variant
                    .pointer("/thinking/budgetTokens")
                    .and_then(Value::as_u64),
                Some(budget_tokens)
            );
        }
    }

    #[test]
    fn anthropic_presets_use_adaptive_thinking_for_claude_4_6_and_later() {
        let models = bundled_anthropic_models();

        for model_id in ["claude-opus-4-6", "claude-sonnet-4-6"] {
            assert_adaptive_variants(&models, model_id, &ADAPTIVE_EFFORT_LEVELS, false);
            assert_eq!(
                model(&models, model_id)
                    .get("contextLimit")
                    .and_then(Value::as_u64),
                Some(1_000_000)
            );
            assert_eq!(
                model(&models, model_id)
                    .get("outputLimit")
                    .and_then(Value::as_u64),
                Some(128_000)
            );
        }

        for model_id in [
            "claude-fable-5",
            "claude-sonnet-5",
            "claude-opus-4-8",
            "claude-opus-4-7",
        ] {
            assert_adaptive_variants(&models, model_id, &EXTENDED_ADAPTIVE_EFFORT_LEVELS, true);
        }
    }

    #[test]
    fn anthropic_presets_keep_legacy_thinking_budgets_on_older_models() {
        let models = bundled_anthropic_models();

        for model_id in [
            "claude-sonnet-4-5-20250929",
            "claude-haiku-4-5-20251001",
            "claude-opus-4-1",
            "claude-sonnet-4-0",
            "claude-3-7-sonnet-latest",
        ] {
            assert_legacy_thinking_variants(&models, model_id);
        }
    }

    #[test]
    fn anthropic_opus_4_5_uses_effort_without_adaptive_thinking() {
        let models = bundled_anthropic_models();
        let variants = model(&models, "claude-opus-4-5-20251101")
            .get("variants")
            .and_then(Value::as_object)
            .expect("Claude Opus 4.5 should define variants");

        assert_eq!(variants.len(), 3);
        for effort_level in ["low", "medium", "high"] {
            let variant = variants
                .get(effort_level)
                .unwrap_or_else(|| panic!("Claude Opus 4.5 should define {effort_level}"));
            assert_eq!(
                variant.get("effort").and_then(Value::as_str),
                Some(effort_level)
            );
            assert!(variant.get("thinking").is_none());
        }
    }
}
