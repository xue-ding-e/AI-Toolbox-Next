use chrono::Local;

use crate::db::DbState;
use super::types::*;

// ============================================================================
// Provider Management Commands
// ============================================================================

/// List all providers ordered by sort_order
#[tauri::command]
pub async fn list_providers(state: tauri::State<'_, DbState>) -> Result<Vec<Provider>, String> {
    let db = state.0.lock().await;

    let records: Vec<ProviderRecord> = db
        .select("provider")
        .await
        .map_err(|e| format!("Failed to list providers: {}", e))?;

    let mut result: Vec<Provider> = records.into_iter().map(Provider::from).collect();
    result.sort_by_key(|p| p.sort_order);
    Ok(result)
}

/// Create a new provider
#[tauri::command]
pub async fn create_provider(
    state: tauri::State<'_, DbState>,
    provider: ProviderInput,
) -> Result<Provider, String> {
    let db = state.0.lock().await;

    // Check if ID already exists
    let existing: Option<ProviderRecord> = db
        .select(("provider", &provider.id))
        .await
        .map_err(|e| format!("Failed to check provider existence: {}", e))?;

    if existing.is_some() {
        return Err(format!("Provider with ID '{}' already exists", provider.id));
    }

    // Set timestamps
    let now = Local::now().to_rfc3339();
    let content = ProviderContent {
        provider_id: provider.id.clone(),
        name: provider.name,
        provider_type: provider.provider_type,
        base_url: provider.base_url,
        api_key: provider.api_key,
        headers: provider.headers,
        timeout: provider.timeout,
        set_cache_key: provider.set_cache_key,
        sort_order: provider.sort_order,
        created_at: now.clone(),
        updated_at: now,
    };

    // Create provider
    let created: Option<ProviderRecord> = db
        .create(("provider", &provider.id))
        .content(content)
        .await
        .map_err(|e| format!("Failed to create provider: {}", e))?;

    created
        .map(Provider::from)
        .ok_or_else(|| "Failed to create provider".to_string())
}

/// Update an existing provider
#[tauri::command]
pub async fn update_provider(
    state: tauri::State<'_, DbState>,
    provider: Provider,
) -> Result<Provider, String> {
    let db = state.0.lock().await;

    // Update timestamp
    let now = Local::now().to_rfc3339();
    let content = ProviderContent {
        provider_id: provider.id.clone(),
        name: provider.name,
        provider_type: provider.provider_type,
        base_url: provider.base_url,
        api_key: provider.api_key,
        headers: provider.headers,
        timeout: provider.timeout,
        set_cache_key: provider.set_cache_key,
        sort_order: provider.sort_order,
        created_at: provider.created_at,
        updated_at: now,
    };

    // Update provider
    let updated: Option<ProviderRecord> = db
        .update(("provider", &provider.id))
        .content(content)
        .await
        .map_err(|e| format!("Failed to update provider: {}", e))?;

    updated
        .map(Provider::from)
        .ok_or_else(|| "Provider not found".to_string())
}

/// Delete a provider and its associated models
#[tauri::command]
pub async fn delete_provider(state: tauri::State<'_, DbState>, id: String) -> Result<(), String> {
    let db = state.0.lock().await;

    // Delete all models associated with this provider
    let models: Vec<ModelRecord> = db
        .select("model")
        .await
        .map_err(|e| format!("Failed to query models: {}", e))?;

    for model in models {
        if model.provider_id == id {
            let _: Option<ModelRecord> = db
                .delete(("model", &format!("{}:{}", model.provider_id, model.model_id)))
                .await
                .map_err(|e| format!("Failed to delete model: {}", e))?;
        }
    }

    // Delete provider
    let _: Option<ProviderRecord> = db
        .delete(("provider", &id))
        .await
        .map_err(|e| format!("Failed to delete provider: {}", e))?;

    Ok(())
}

/// Reorder providers
#[tauri::command]
pub async fn reorder_providers(
    state: tauri::State<'_, DbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.0.lock().await;

    for (index, id) in ids.iter().enumerate() {
        let record: Option<ProviderRecord> = db
            .select(("provider", id))
            .await
            .map_err(|e| format!("Failed to get provider: {}", e))?;

        if let Some(r) = record {
            let content = ProviderContent {
                provider_id: r.provider_id,
                name: r.name,
                provider_type: r.provider_type,
                base_url: r.base_url,
                api_key: r.api_key,
                headers: r.headers,
                timeout: r.timeout,
                set_cache_key: r.set_cache_key,
                sort_order: index as i32,
                created_at: r.created_at,
                updated_at: Local::now().to_rfc3339(),
            };

            let _: Option<ProviderRecord> = db
                .update(("provider", id))
                .content(content)
                .await
                .map_err(|e| format!("Failed to update provider order: {}", e))?;
        }
    }

    Ok(())
}

// ============================================================================
// Model Management Commands
// ============================================================================

/// List models for a specific provider ordered by sort_order
#[tauri::command(rename_all = "snake_case")]
pub async fn list_models(
    state: tauri::State<'_, DbState>,
    provider_id: String,
) -> Result<Vec<Model>, String> {
    let db = state.0.lock().await;

    let all_records: Vec<ModelRecord> = db
        .select("model")
        .await
        .map_err(|e| format!("Failed to list models: {}", e))?;

    let mut filtered: Vec<Model> = all_records
        .into_iter()
        .filter(|m| m.provider_id == provider_id)
        .map(Model::from)
        .collect();

    filtered.sort_by_key(|m| m.sort_order);
    Ok(filtered)
}

/// Create a new model
#[tauri::command]
pub async fn create_model(
    state: tauri::State<'_, DbState>,
    model: ModelInput,
) -> Result<Model, String> {
    let db = state.0.lock().await;

    // Check if model ID already exists under this provider
    let record_id = format!("{}:{}", model.provider_id, model.id);
    let existing: Option<ModelRecord> = db
        .select(("model", record_id.as_str()))
        .await
        .map_err(|e| format!("Failed to check model existence: {}", e))?;

    if existing.is_some() {
        return Err(format!(
            "Model with ID '{}' already exists under provider '{}'",
            model.id, model.provider_id
        ));
    }

    // Set timestamps
    let now = Local::now().to_rfc3339();
    let content = ModelContent {
        model_id: model.id.clone(),
        provider_id: model.provider_id,
        name: model.name,
        context_limit: model.context_limit,
        output_limit: model.output_limit,
        options: model.options,
        variants: model.variants,
        sort_order: model.sort_order,
        created_at: now.clone(),
        updated_at: now,
    };

    // Create model
    let created: Option<ModelRecord> = db
        .create(("model", record_id.as_str()))
        .content(content)
        .await
        .map_err(|e| format!("Failed to create model: {}", e))?;

    created
        .map(Model::from)
        .ok_or_else(|| "Failed to create model".to_string())
}

/// Update an existing model
#[tauri::command]
pub async fn update_model(
    state: tauri::State<'_, DbState>,
    model: Model,
) -> Result<Model, String> {
    let db = state.0.lock().await;

    let record_id = format!("{}:{}", model.provider_id, model.id);

    // Update timestamp
    let now = Local::now().to_rfc3339();
    let content = ModelContent {
        model_id: model.id,
        provider_id: model.provider_id,
        name: model.name,
        context_limit: model.context_limit,
        output_limit: model.output_limit,
        options: model.options,
        variants: model.variants,
        sort_order: model.sort_order,
        created_at: model.created_at,
        updated_at: now,
    };

    // Update model
    let updated: Option<ModelRecord> = db
        .update(("model", record_id.as_str()))
        .content(content)
        .await
        .map_err(|e| format!("Failed to update model: {}", e))?;

    updated
        .map(Model::from)
        .ok_or_else(|| "Model not found".to_string())
}

/// Delete a model
#[tauri::command(rename_all = "snake_case")]
pub async fn delete_model(
    state: tauri::State<'_, DbState>,
    provider_id: String,
    id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    let record_id = format!("{}:{}", provider_id, id);

    let _: Option<ModelRecord> = db
        .delete(("model", record_id.as_str()))
        .await
        .map_err(|e| format!("Failed to delete model: {}", e))?;

    Ok(())
}

/// Reorder models for a specific provider
#[tauri::command(rename_all = "snake_case")]
pub async fn reorder_models(
    state: tauri::State<'_, DbState>,
    provider_id: String,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.0.lock().await;

    for (index, id) in ids.iter().enumerate() {
        let record_id = format!("{}:{}", provider_id, id);
        let record: Option<ModelRecord> = db
            .select(("model", record_id.as_str()))
            .await
            .map_err(|e| format!("Failed to get model: {}", e))?;

        if let Some(r) = record {
            let content = ModelContent {
                model_id: r.model_id,
                provider_id: r.provider_id,
                name: r.name,
                context_limit: r.context_limit,
                output_limit: r.output_limit,
                options: r.options,
                variants: r.variants,
                sort_order: index as i32,
                created_at: r.created_at,
                updated_at: Local::now().to_rfc3339(),
            };

            let _: Option<ModelRecord> = db
                .update(("model", record_id.as_str()))
                .content(content)
                .await
                .map_err(|e| format!("Failed to update model order: {}", e))?;
        }
    }

    Ok(())
}

/// Get all providers with their models
#[tauri::command]
pub async fn get_all_providers_with_models(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<ProviderWithModels>, String> {
    let db = state.0.lock().await;

    // Get all providers
    let provider_records: Vec<ProviderRecord> = db
        .select("provider")
        .await
        .map_err(|e| format!("Failed to list providers: {}", e))?;

    let mut providers: Vec<Provider> = provider_records.into_iter().map(Provider::from).collect();
    providers.sort_by_key(|p| p.sort_order);

    // Get all models
    let model_records: Vec<ModelRecord> = db
        .select("model")
        .await
        .map_err(|e| format!("Failed to list models: {}", e))?;

    let all_models: Vec<Model> = model_records.into_iter().map(Model::from).collect();

    // Build result
    let mut result = Vec::new();
    for provider in providers {
        let mut models: Vec<Model> = all_models
            .iter()
            .filter(|m| m.provider_id == provider.id)
            .cloned()
            .collect();

        models.sort_by_key(|m| m.sort_order);

        result.push(ProviderWithModels { provider, models });
    }

    Ok(result)
}
