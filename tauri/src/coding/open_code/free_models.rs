use crate::db::DbState;
use crate::http_client;
use super::types::{FreeModel, ProviderModelsData};

// Load default models data from resources/models.json at compile time
const DEFAULT_MODELS_JSON: &str = include_str!("../../../resources/models.json");

const MODELS_API_URL: &str = "https://models.dev/api.json";
const DB_TABLE: &str = "provider_models";
const OPENCODE_PROVIDER_ID: &str = "opencode"; // Default provider for free models
const CACHE_DURATION_HOURS: u64 = 6; // 6 hours cache duration

/// Get all providers data from resources/models.json
/// Returns the complete JSON object containing all providers
fn get_all_default_providers_data() -> serde_json::Value {
    serde_json::from_str(DEFAULT_MODELS_JSON).unwrap_or_else(|e| {
        eprintln!("Failed to parse default models.json: {}", e);
        serde_json::json!({})
    })
}

/// Get default provider data (opencode channel) from resources/models.json
/// Returns the complete JSON object for the opencode provider
pub fn get_default_provider_data() -> serde_json::Value {
    let api_response = get_all_default_providers_data();

    // Extract the opencode provider object
    if let Some(opencode) = api_response.get(OPENCODE_PROVIDER_ID) {
        opencode.clone()
    } else {
        serde_json::json!({
            "name": "OpenCode Zen",
            "models": {}
        })
    }
}

/// Get default free models from resources/models.json
/// Returns filtered free models from the opencode channel
pub fn get_default_free_models() -> Vec<FreeModel> {
    let provider_data = get_default_provider_data();
    filter_free_models(OPENCODE_PROVIDER_ID, &provider_data)
}

/// Fetch all providers data from API
/// Returns the complete JSON object containing all providers
async fn fetch_all_providers_from_api(state: &DbState) -> Result<serde_json::Value, String> {
    let client = http_client::client_with_timeout(state, 30).await?;

    let response = client
        .get(MODELS_API_URL)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch models API: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("API error: {}", response.status()));
    }

    let api_response: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    Ok(api_response)
}

/// Fetch provider data (opencode channel) from API
pub async fn fetch_provider_data_from_api(state: &DbState) -> Result<serde_json::Value, String> {
    let api_response = fetch_all_providers_from_api(state).await?;

    // Extract the opencode provider object
    let opencode_data = api_response
        .get(OPENCODE_PROVIDER_ID)
        .cloned()
        .ok_or_else(|| "opencode channel not found in API response".to_string())?;

    Ok(opencode_data)
}

/// Filter free models from provider data (where cost.input and cost.output are both 0)
fn filter_free_models(provider_id: &str, provider_data: &serde_json::Value) -> Vec<FreeModel> {
    let mut free_models = Vec::new();

    // Get provider name (e.g., "OpenCode Zen")
    let provider_name = provider_data
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    // Get models object
    let models_obj = match provider_data.get("models").and_then(|v| v.as_object()) {
        Some(obj) => obj,
        None => {
            return free_models;
        }
    };

    // Iterate through models
    for (model_id, model_obj) in models_obj {
        if let Some(model) = model_obj.as_object() {
            // Check if cost.input and cost.output are both 0
            let is_free = model
                .get("cost")
                .and_then(|cost| cost.as_object())
                .map(|cost| {
                    let input = cost.get("input").and_then(|v| v.as_f64()).unwrap_or(-1.0);
                    let output = cost.get("output").and_then(|v| v.as_f64()).unwrap_or(-1.0);
                    input == 0.0 && output == 0.0
                })
                .unwrap_or(false);

            if is_free {
                let model_name = model
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(model_id)
                    .to_string();

                let free_model = FreeModel {
                    id: model_id.clone(),
                    name: model_name,
                    provider_id: provider_id.to_string(),
                    provider_name: provider_name.clone(),
                    context: model
                        .get("limit")
                        .and_then(|limit| limit.as_object())
                        .and_then(|limit| limit.get("context"))
                        .and_then(|v| v.as_i64()),
                };
                free_models.push(free_model);
            }
        }
    }

    free_models
}

/// Read provider models data from database by provider_id
pub async fn read_provider_models_from_db(state: &DbState, provider_id: &str) -> Result<Option<ProviderModelsData>, String> {
    let db = state.0.lock().await;

    // Query using the same pattern as existing code
    let records_result: Result<Vec<serde_json::Value>, _> = db
        .query(&format!("SELECT * OMIT id FROM {}:`{}` LIMIT 1", DB_TABLE, provider_id))
        .await
        .map_err(|e| format!("Failed to query provider models: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                // Parse using the flat structure
                let data = ProviderModelsData {
                    provider_id: record
                        .get("provider_id")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .unwrap_or_default(),
                    value: record
                        .get("value")
                        .cloned()
                        .unwrap_or(serde_json::json!({})),
                    updated_at: record
                        .get("updated_at")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .unwrap_or_default(),
                };

                Ok(Some(data))
            } else {
                Ok(None)
            }
        }
        Err(e) => {
            Err(e.to_string())
        }
    }
}

/// Save provider models data to database
pub async fn save_provider_models_to_db(state: &DbState, data: &ProviderModelsData) -> Result<(), String> {
    let db = state.0.lock().await;

    // Use json! macro to create a flat structure (same pattern as existing code)
    let json_data = serde_json::json!({
        "provider_id": data.provider_id,
        "value": data.value,
        "updated_at": data.updated_at
    });

    // Use DELETE + CREATE pattern to avoid version conflicts
    db.query(&format!("DELETE {}:`{}`", DB_TABLE, data.provider_id))
        .await
        .map_err(|e| format!("Failed to delete old record: {}", e))?;

    db.query(format!(
        "CREATE {}:`{}` CONTENT $data",
        DB_TABLE, data.provider_id
    ))
    .bind(("data", json_data))
    .await
    .map_err(|e| format!("Failed to create record: {}", e))?;

    Ok(())
}

/// Save all provider models data to database (batch insert)
async fn save_all_provider_models_to_db(state: &DbState, all_providers: &serde_json::Value, updated_at: &str) -> Result<usize, String> {
    let providers_obj = match all_providers.as_object() {
        Some(obj) => obj,
        None => return Err("Invalid providers data: not an object".to_string()),
    };

    // Acquire lock once for all operations
    let db = state.0.lock().await;
    let mut saved_count = 0;

    for (provider_id, provider_data) in providers_obj {
        let json_data = serde_json::json!({
            "provider_id": provider_id,
            "value": provider_data,
            "updated_at": updated_at
        });

        // Use DELETE + CREATE pattern
        if let Err(e) = db.query(&format!("DELETE {}:`{}`", DB_TABLE, provider_id)).await {
            eprintln!("Failed to delete old record for {}: {}", provider_id, e);
            continue;
        }

        match db.query(format!("CREATE {}:`{}` CONTENT $data", DB_TABLE, provider_id))
            .bind(("data", json_data))
            .await
        {
            Ok(_) => saved_count += 1,
            Err(e) => eprintln!("Failed to create record for {}: {}", provider_id, e),
        }
    }

    eprintln!("[DEBUG] Saved {} providers to database", saved_count);
    Ok(saved_count)
}

/// Check if cache is expired (6 hours)
fn is_cache_expired(updated_at: &str) -> bool {
    match chrono::DateTime::parse_from_rfc3339(updated_at) {
        Ok(datetime) => {
            let now = chrono::Utc::now();
            let duration = now.signed_duration_since(datetime);
            duration.num_hours() >= CACHE_DURATION_HOURS as i64
        }
        Err(_) => true, // Parse failed, consider as expired
    }
}

/// Get free models with cache logic
/// Returns (free_models, from_cache, updated_at)
///
/// Cache strategy:
/// - If cache is fresh (< 6 hours): return cached data immediately
/// - If cache is expired (>= 6 hours): return cached data immediately, then refresh in background
/// - If no cache exists: fetch from API (synchronous)
/// - If force_refresh: fetch from API (synchronous)
pub async fn get_free_models(state: &DbState, force_refresh: bool) -> Result<(Vec<FreeModel>, bool, Option<String>), String> {
    // 1. Try to read opencode provider from database (unless force_refresh)
    if !force_refresh {
        match read_provider_models_from_db(state, OPENCODE_PROVIDER_ID).await {
            Ok(Some(cached_data)) => {
                if !is_cache_expired(&cached_data.updated_at) {
                    // Cache is fresh: filter free models from cached provider data
                    let free_models = filter_free_models(OPENCODE_PROVIDER_ID, &cached_data.value);
                    eprintln!("[CACHE HIT] Returning cached free models (fresh, updated_at: {}, count: {})", cached_data.updated_at, free_models.len());
                    return Ok((free_models, true, Some(cached_data.updated_at)));
                }

                // Cache expired: return filtered free models from cached data, then refresh in background
                let cached_models = filter_free_models(OPENCODE_PROVIDER_ID, &cached_data.value);
                let updated_at = cached_data.updated_at.clone();
                eprintln!("[CACHE EXPIRED] (updated_at: {}), returning {} stale models and refreshing in background...", updated_at, cached_models.len());

                // Spawn background task to refresh cache
                let db_arc = state.0.clone();
                let db_state = DbState(db_arc);
                tauri::async_runtime::spawn(async move {
                    eprintln!("[Background] Starting all providers data refresh...");
                    match fetch_and_update_all_providers(&db_state).await {
                        Ok(count) => {
                            eprintln!("[Background] Successfully refreshed {} providers", count);
                        }
                        Err(e) => {
                            eprintln!("[Background] Failed to refresh providers: {}", e);
                        }
                    }
                });

                return Ok((cached_models, true, Some(updated_at)));
            }
            Ok(None) => {
                eprintln!("[CACHE MISS] No cached data found, will fetch from API");
            }
            Err(e) => {
                eprintln!("[CACHE ERROR] Failed to read cache: {}, will fetch from API", e);
            }
        }
    }

    // 2. No cache or force_refresh: fetch all providers from API (synchronous)
    eprintln!("[FETCH] No cache or force_refresh, fetching all providers from API...");
    fetch_and_update_all_providers(state).await?;

    // 3. Read opencode provider from database and filter free models
    match read_provider_models_from_db(state, OPENCODE_PROVIDER_ID).await {
        Ok(Some(data)) => {
            let free_models = filter_free_models(OPENCODE_PROVIDER_ID, &data.value);
            if free_models.is_empty() {
                Ok((get_default_free_models(), false, None))
            } else {
                Ok((free_models, false, None))
            }
        }
        _ => Ok((get_default_free_models(), false, None)),
    }
}

/// Fetch all providers from API and save to database
async fn fetch_and_update_all_providers(state: &DbState) -> Result<usize, String> {
    let all_providers = fetch_all_providers_from_api(state).await?;

    // If API returned empty, use default providers data
    let final_providers = if all_providers.as_object().map(|m| m.is_empty()).unwrap_or(true) {
        eprintln!("API returned empty providers, using default data");
        get_all_default_providers_data()
    } else {
        all_providers
    };

    // Save all providers to database
    let updated_at = chrono::Utc::now().to_rfc3339();
    save_all_provider_models_to_db(state, &final_providers, &updated_at).await
}

/// Initialize default provider models in database (called on app startup)
/// Only writes if no cached data exists (checks opencode as indicator)
pub async fn init_default_provider_models(state: &DbState) -> Result<(), String> {
    // Check if opencode provider exists as indicator for all providers
    match read_provider_models_from_db(state, OPENCODE_PROVIDER_ID).await {
        Ok(Some(data)) => {
            eprintln!("Provider models cache already exists (updated_at: {}), skipping initialization", data.updated_at);
            Ok(())
        }
        Ok(None) => {
            eprintln!("No provider models cache found, initializing with default data for all providers");
            let all_providers = get_all_default_providers_data();
            let updated_at = chrono::Utc::now().to_rfc3339();

            match save_all_provider_models_to_db(state, &all_providers, &updated_at).await {
                Ok(count) => {
                    eprintln!("Successfully initialized {} providers with default data", count);
                    Ok(())
                }
                Err(e) => {
                    eprintln!("Failed to initialize providers: {}", e);
                    Err(e)
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to check provider models cache: {}, skipping initialization", e);
            Ok(())
        }
    }
}

/// Get provider models data by provider_id (internal function)
/// This is the internal API to get specific provider's model information
pub async fn get_provider_models_internal(state: &DbState, provider_id: &str) -> Result<Option<ProviderModelsData>, String> {
    read_provider_models_from_db(state, provider_id).await
}
