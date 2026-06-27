use super::cli_proxy;
use super::paths::ProxyGatewayPaths;
use super::runtime::ProxyGatewayState;
use super::types::{GatewayCliKey, GatewayCliTakeoverStatus, GatewayProxyMode, ProxyGatewayStatus};
use crate::db::SqliteDbState;
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub async fn apply_or_switch_provider<R: Runtime>(
    app: &AppHandle<R>,
    cli_key: GatewayCliKey,
    provider_id: &str,
    from_tray: bool,
) -> Result<GatewayCliTakeoverStatus, String> {
    let gateway_state = app.state::<ProxyGatewayState>();
    let _switch_guard = gateway_state.provider_switch_lock.lock().await;
    let db_state = app.state::<SqliteDbState>();
    let db = db_state.db();
    let paths = proxy_gateway_paths(app)?;
    let gateway_status = current_gateway_status(&gateway_state)?;
    let current_status = cli_proxy::cli_takeover_status(db, &paths, cli_key, &gateway_status).await;

    let Some(previous_mode) = current_status.mode else {
        if current_status.can_restore_direct {
            return Err(current_status.message.unwrap_or_else(|| {
                "Restore direct mode before switching Gateway proxy providers".to_string()
            }));
        }
        apply_direct_provider(app, cli_key, provider_id, from_tray).await?;
        return Ok(cli_proxy::cli_takeover_status(db, &paths, cli_key, &gateway_status).await);
    };

    if current_status.primary_provider_id.as_deref() == Some(provider_id) {
        return Ok(current_status);
    }

    if !gateway_status.running {
        return Err("Start the proxy gateway before switching Gateway proxy providers".to_string());
    }
    if !current_status.can_restore_direct {
        return Err(current_status.message.unwrap_or_else(|| {
            "Restore direct mode before switching Gateway proxy providers".to_string()
        }));
    }

    cli_proxy::ensure_proxyable_provider(db, cli_key, provider_id).await?;
    cli_proxy::restore_cli_direct(db, &paths, cli_key, &gateway_status).await?;
    apply_direct_provider_without_events(app, cli_key, provider_id).await?;

    let refreshed_gateway_status = current_gateway_status(&gateway_state)?;
    let mut next_status = cli_proxy::engage_single_cli(
        db,
        &paths,
        cli_key,
        &refreshed_gateway_status,
        provider_id.to_string(),
    )
    .await?;

    if previous_mode == GatewayProxyMode::Failover {
        let refreshed_gateway_status = current_gateway_status(&gateway_state)?;
        next_status =
            cli_proxy::engage_failover_cli(db, &paths, cli_key, &refreshed_gateway_status).await?;
    }

    gateway_state.clear_provider_cache()?;
    emit_gateway_cli_config_changed(app, from_tray);
    emit_gateway_cli_wsl_sync_request(app, cli_key);
    Ok(next_status)
}

async fn apply_direct_provider<R: Runtime>(
    app: &AppHandle<R>,
    cli_key: GatewayCliKey,
    provider_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    let db_state = app.state::<SqliteDbState>();
    let db = db_state.db();
    match cli_key {
        GatewayCliKey::Claude => {
            crate::coding::claude_code::commands::apply_config_internal_with_sync(
                &db,
                app,
                provider_id,
                from_tray,
                true,
            )
            .await
        }
        GatewayCliKey::Codex => {
            crate::coding::codex::commands::apply_config_internal_with_sync(
                &db,
                app,
                provider_id,
                from_tray,
                true,
            )
            .await
        }
        GatewayCliKey::Gemini => {
            crate::coding::gemini_cli::commands::apply_config_internal_with_sync(
                &db,
                app,
                provider_id,
                from_tray,
                true,
            )
            .await
        }
        GatewayCliKey::OpenCode => Err("This CLI is not supported by the gateway MVP".to_string()),
    }
}

async fn apply_direct_provider_without_events<R: Runtime>(
    app: &AppHandle<R>,
    cli_key: GatewayCliKey,
    provider_id: &str,
) -> Result<(), String> {
    let db_state = app.state::<SqliteDbState>();
    let db = db_state.db();
    match cli_key {
        GatewayCliKey::Claude => {
            crate::coding::claude_code::commands::apply_config_internal_without_events(
                &db,
                app,
                provider_id,
            )
            .await
        }
        GatewayCliKey::Codex => {
            crate::coding::codex::commands::apply_config_internal_without_events(
                &db,
                app,
                provider_id,
            )
            .await
        }
        GatewayCliKey::Gemini => {
            crate::coding::gemini_cli::commands::apply_config_internal_without_events(
                &db,
                app,
                provider_id,
            )
            .await
        }
        GatewayCliKey::OpenCode => Err("This CLI is not supported by the gateway MVP".to_string()),
    }
}

fn current_gateway_status(gateway_state: &ProxyGatewayState) -> Result<ProxyGatewayStatus, String> {
    let manager = gateway_state
        .manager
        .lock()
        .map_err(|_| "Proxy gateway manager lock poisoned".to_string())?;
    Ok(manager.status())
}

fn proxy_gateway_paths<R: Runtime>(app: &AppHandle<R>) -> Result<ProxyGatewayPaths, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;
    Ok(ProxyGatewayPaths::new(app_data_dir))
}

fn emit_gateway_cli_config_changed<R: Runtime>(app: &AppHandle<R>, from_tray: bool) {
    let payload = if from_tray { "tray" } else { "window" };
    if let Err(error) = app.emit("config-changed", payload) {
        log::warn!("Failed to emit config-changed after gateway CLI config change: {error}");
    }
}

fn emit_gateway_cli_wsl_sync_request<R: Runtime>(app: &AppHandle<R>, cli_key: GatewayCliKey) {
    let event_name = match cli_key {
        GatewayCliKey::Claude => "wsl-sync-request-claude",
        GatewayCliKey::Codex => "wsl-sync-request-codex",
        GatewayCliKey::Gemini => "wsl-sync-request-geminicli",
        GatewayCliKey::OpenCode => return,
    };
    if let Err(error) = app.emit(event_name, ()) {
        log::warn!("Failed to emit {event_name} after gateway CLI config change: {error}");
    }
}
