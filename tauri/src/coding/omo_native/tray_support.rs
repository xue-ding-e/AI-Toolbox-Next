//! 托盘菜单数据源：Agent 方案列表 + 当前生效方案。
//!
//! Native 的「默认模型」由 `model_profile`（模型档）与 `[native].agents` 里的
//! 具体 agent 模型共同决定，没有一个单一的全局默认模型字段，所以托盘只暴露
//! **方案切换**（与 `oh_my_openagent` 的托盘语义一致）。

use tauri::{AppHandle, Emitter, Manager, Runtime};

#[derive(Debug, Clone)]
pub struct TrayAgentsConfigItem {
    pub id: String,
    pub display_name: String,
    pub is_selected: bool,
    /// Disabled configs stay listed (greyed out) rather than disappearing, so a
    /// config the user switched off in the window is still reachable from the
    /// tray — same treatment as the Oh My OpenAgent section.
    pub is_disabled: bool,
}

#[derive(Debug, Clone)]
pub struct TrayAgentsConfigData {
    pub title: String,
    pub current_display: String,
    pub items: Vec<TrayAgentsConfigItem>,
}

impl TrayAgentsConfigData {
    pub fn empty(title: &str) -> Self {
        Self {
            title: title.to_string(),
            current_display: String::new(),
            items: vec![],
        }
    }
}

pub async fn get_omo_native_tray_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TrayAgentsConfigData, String> {
    let configs = super::commands::list_omo_native_agents_configs(app.state()).await?;
    let items = configs
        .into_iter()
        // `__local__` is the read-only bridge state for a `[native]` block that
        // was never saved from this app; it cannot be applied, so it stays out
        // of the tray. Disabled configs are kept (greyed out in the menu).
        .filter(|config| config.id != "__local__")
        .map(|config| TrayAgentsConfigItem {
            id: config.id,
            display_name: config.name,
            is_selected: config.is_applied,
            is_disabled: config.is_disabled,
        })
        .collect::<Vec<_>>();

    // The section title is re-stamped by the tray with its own wording (see
    // `texts.omo_native_header`); this is only the fallback for direct callers.
    Ok(TrayAgentsConfigData {
        title: "OmO Native".to_string(),
        current_display: String::new(),
        items,
    })
}

pub async fn apply_omo_native_agents_config<R: Runtime>(
    app: &AppHandle<R>,
    config_id: &str,
) -> Result<(), String> {
    let state = app.state::<crate::db::SqliteDbState>();
    let db = state.db();
    super::commands::write_agents_config_to_file(&db, config_id).await?;
    let now = chrono::Local::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        crate::db::helpers::db_update_applied_status(
            conn,
            crate::db::schema::DbTable::OmoNativeAgentsConfig,
            Some(config_id),
            &now,
        )
    })?;
    let _ = app.emit("config-changed", "omo_native");
    #[cfg(target_os = "windows")]
    let _ = app.emit("wsl-sync-request-omo-native", ());
    Ok(())
}

pub async fn is_enabled_for_tray<R: Runtime>(_app: &AppHandle<R>) -> bool {
    true
}
