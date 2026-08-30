//! Per-CLI applied-account refresh passes used by the shared scheduler.

use tauri::AppHandle;

use crate::db::SqliteDbState;

pub(super) async fn grok_refresh_applied_pass(
    db: &SqliteDbState,
    app: &AppHandle,
) -> Result<(), String> {
    crate::coding::grok::refresh_applied_grok_accounts_if_needed(db, app).await
}

pub(super) async fn codex_refresh_applied_pass(
    db: &SqliteDbState,
    app: &AppHandle,
) -> Result<(), String> {
    crate::coding::codex::refresh_applied_codex_accounts_if_needed(db, app).await
}

pub(super) async fn gemini_cli_refresh_applied_pass(
    db: &SqliteDbState,
    app: &AppHandle,
) -> Result<(), String> {
    crate::coding::gemini_cli::refresh_applied_gemini_cli_accounts_if_needed(db, app).await
}

pub(super) async fn kimi_refresh_applied_pass(
    db: &SqliteDbState,
    app: &AppHandle,
) -> Result<(), String> {
    crate::coding::kimi::refresh_applied_kimi_accounts_if_needed(db, app).await
}
