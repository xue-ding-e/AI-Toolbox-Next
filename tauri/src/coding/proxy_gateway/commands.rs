use super::cli_proxy;
use super::listen::check_port_available;
use super::metrics;
use super::model_health;
use super::paths::ProxyGatewayPaths;
use super::request_log;
use super::runtime::ProxyGatewayState;
use super::settings;
use super::types::{
    GatewayCliKey, GatewayCliTakeoverStatus, GatewayModelHealthItem, GatewayRequestLogDetail,
    GatewayRequestLogSummary, MetricRollupItem, ProxyGatewayHealthCheckResult,
    ProxyGatewayPortCheckInput, ProxyGatewayPortCheckResult, ProxyGatewayRequestLogListInput,
    ProxyGatewaySettings, ProxyGatewayStatus, ProxyGatewayStopPreflight,
};
use crate::db::DbState;
use tauri::Manager;

pub async fn proxy_gateway_start_if_enabled_on_startup(
    db_state: &DbState,
    gateway_state: &ProxyGatewayState,
    app: &tauri::AppHandle,
) -> Result<Option<ProxyGatewayStatus>, String> {
    let settings = settings::load_settings(&db_state.db()).await?;
    if !settings.enabled_on_startup {
        return Ok(None);
    }
    let paths = proxy_gateway_paths(app)?;

    let mut manager = gateway_state
        .manager
        .lock()
        .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
    manager
        .start_with_context(settings, db_state.db(), paths)
        .map(Some)
}

#[tauri::command]
pub async fn proxy_gateway_get_settings(
    state: tauri::State<'_, DbState>,
) -> Result<ProxyGatewaySettings, String> {
    settings::load_settings(&state.db()).await
}

#[tauri::command]
pub async fn proxy_gateway_update_settings(
    gateway_state: tauri::State<'_, ProxyGatewayState>,
    state: tauri::State<'_, DbState>,
    mut settings: ProxyGatewaySettings,
) -> Result<ProxyGatewaySettings, String> {
    {
        let mut manager = gateway_state
            .manager
            .lock()
            .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
        let running = manager.status().running;
        if running {
            settings.enabled_on_startup = true;
            manager.update_runtime_settings(settings.clone())?;
        }
    }
    settings::save_settings(&state.db(), settings).await
}

#[tauri::command]
pub async fn proxy_gateway_start(
    gateway_state: tauri::State<'_, ProxyGatewayState>,
    db_state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    settings: Option<ProxyGatewaySettings>,
) -> Result<ProxyGatewayStatus, String> {
    let mut settings = match settings {
        Some(settings) => settings,
        None => settings::load_settings(&db_state.db()).await?,
    };
    let paths = proxy_gateway_paths(&app)?;
    let status = {
        let mut manager = gateway_state
            .manager
            .lock()
            .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
        manager.start_with_context(settings.clone(), db_state.db(), paths)?
    };

    settings.enabled_on_startup = true;
    if let Err(error) = settings::save_settings(&db_state.db(), settings).await {
        log::warn!("Failed to persist proxy gateway startup state after start: {error}");
    }

    Ok(status)
}

#[tauri::command]
pub async fn proxy_gateway_stop(
    gateway_state: tauri::State<'_, ProxyGatewayState>,
    db_state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
) -> Result<ProxyGatewayStatus, String> {
    let current_status = {
        let manager = gateway_state
            .manager
            .lock()
            .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
        manager.status()
    };
    let paths = proxy_gateway_paths(&app)?;
    let preflight = cli_proxy::stop_preflight(&db_state.db(), &paths, &current_status).await;
    if !preflight.allowed {
        return Err(preflight.message.unwrap_or_else(|| {
            "Restore gateway-taken-over CLIs to direct mode before stopping the gateway".to_string()
        }));
    }

    let mut settings = settings::load_settings(&db_state.db()).await?;
    settings.enabled_on_startup = false;
    settings::save_settings(&db_state.db(), settings).await?;

    let mut manager = gateway_state
        .manager
        .lock()
        .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
    manager.stop()
}

#[tauri::command]
pub fn proxy_gateway_status(
    gateway_state: tauri::State<'_, ProxyGatewayState>,
) -> Result<ProxyGatewayStatus, String> {
    let manager = gateway_state
        .manager
        .lock()
        .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
    Ok(manager.status())
}

#[tauri::command]
pub fn proxy_gateway_health_check(
    gateway_state: tauri::State<'_, ProxyGatewayState>,
) -> Result<ProxyGatewayHealthCheckResult, String> {
    let manager = gateway_state
        .manager
        .lock()
        .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
    Ok(manager.health_check())
}

#[tauri::command]
pub fn proxy_gateway_check_port_available(
    input: ProxyGatewayPortCheckInput,
) -> Result<ProxyGatewayPortCheckResult, String> {
    check_port_available(input)
}

#[tauri::command]
pub async fn proxy_gateway_cli_statuses(
    gateway_state: tauri::State<'_, ProxyGatewayState>,
    db_state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
) -> Result<Vec<GatewayCliTakeoverStatus>, String> {
    let status = {
        let manager = gateway_state
            .manager
            .lock()
            .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
        manager.status()
    };
    let paths = proxy_gateway_paths(&app)?;
    Ok(cli_proxy::cli_takeover_statuses(&db_state.db(), &paths, &status).await)
}

#[tauri::command]
pub async fn proxy_gateway_cli_status(
    gateway_state: tauri::State<'_, ProxyGatewayState>,
    db_state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    cli_key: GatewayCliKey,
) -> Result<GatewayCliTakeoverStatus, String> {
    let status = {
        let manager = gateway_state
            .manager
            .lock()
            .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
        manager.status()
    };
    let paths = proxy_gateway_paths(&app)?;
    Ok(cli_proxy::cli_takeover_status(&db_state.db(), &paths, cli_key, &status).await)
}

#[tauri::command]
pub async fn proxy_gateway_takeover_cli(
    gateway_state: tauri::State<'_, ProxyGatewayState>,
    db_state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    cli_key: GatewayCliKey,
) -> Result<GatewayCliTakeoverStatus, String> {
    let status = {
        let manager = gateway_state
            .manager
            .lock()
            .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
        manager.status()
    };
    let paths = proxy_gateway_paths(&app)?;
    cli_proxy::takeover_cli(&db_state.db(), &paths, cli_key, &status).await
}

#[tauri::command]
pub async fn proxy_gateway_restore_cli_direct(
    gateway_state: tauri::State<'_, ProxyGatewayState>,
    db_state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    cli_key: GatewayCliKey,
) -> Result<GatewayCliTakeoverStatus, String> {
    let status = {
        let manager = gateway_state
            .manager
            .lock()
            .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
        manager.status()
    };
    let paths = proxy_gateway_paths(&app)?;
    cli_proxy::restore_cli_direct(&db_state.db(), &paths, cli_key, &status).await
}

#[tauri::command]
pub async fn proxy_gateway_stop_preflight(
    gateway_state: tauri::State<'_, ProxyGatewayState>,
    db_state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
) -> Result<ProxyGatewayStopPreflight, String> {
    let status = {
        let manager = gateway_state
            .manager
            .lock()
            .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
        manager.status()
    };
    let paths = proxy_gateway_paths(&app)?;
    Ok(cli_proxy::stop_preflight(&db_state.db(), &paths, &status).await)
}

#[tauri::command]
pub fn proxy_gateway_request_logs(
    app: tauri::AppHandle,
    input: ProxyGatewayRequestLogListInput,
) -> Result<Vec<GatewayRequestLogSummary>, String> {
    let paths = proxy_gateway_paths(&app)?;
    request_log::list_request_logs(&paths, input)
}

#[tauri::command]
pub fn proxy_gateway_request_log_detail(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<Option<GatewayRequestLogDetail>, String> {
    let paths = proxy_gateway_paths(&app)?;
    request_log::get_request_log_detail(&paths, &trace_id)
}

#[tauri::command]
pub fn proxy_gateway_metric_rollups(
    app: tauri::AppHandle,
) -> Result<Vec<MetricRollupItem>, String> {
    let paths = proxy_gateway_paths(&app)?;
    metrics::list_metric_rollups(&paths)
}

#[tauri::command]
pub async fn proxy_gateway_model_health_entries(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbState>,
) -> Result<Vec<GatewayModelHealthItem>, String> {
    let paths = proxy_gateway_paths(&app)?;
    let settings = settings::load_settings(&db_state.db()).await?;
    model_health::list_model_health_items(&paths.model_health_path(), settings)
}

fn proxy_gateway_paths(app: &tauri::AppHandle) -> Result<ProxyGatewayPaths, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;
    Ok(ProxyGatewayPaths::new(app_data_dir))
}
