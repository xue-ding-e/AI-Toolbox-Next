//! Pi 的扩展管理命令层。
//!
//! **逻辑全在 `crate::coding::cli_extensions`**——它与 OmO Native 共用 senpi 引擎
//! 的包体系（同一份 `settings.json` 形状、同一套子命令、逐字相同的 `list` 输出），
//! 差异只有 [`PI_EXTENSION_SPEC`] 里那几项。本文件只负责：
//!
//! 1. 声明 Pi 的 spec（CLI 名 / 环境变量 / 内置扩展前缀 / 扫描超时）；
//! 2. 解析 `pi` 二进制与 runtime location；
//! 3. 把共享实现的结果转成前端的 `PiExtension*` 契约。
//!
//! 类型也在这里再导出：前端契约名是 `PiExtensionSummary` 等，与共享层的
//! `CliExtensionSummary` 同形但名字不同（两个页面各自沿用自己 CLI 的名字）。

use std::path::Path;

use super::commands::get_pi_settings_path_async;
use super::types::{
    PiExtensionActionInput, PiExtensionCommandResult, PiExtensionEnabledInput,
    PiExtensionInstallInput, PiExtensionKind, PiExtensionListResult, PiExtensionScope,
    PiExtensionSummary, PiExtensionUpdateInput,
};
use crate::coding::cli_extensions::{
    self as shared, CliExtensionKind, CliExtensionScope, CliExtensionSpec,
};
use crate::coding::cli_resolver::resolve_local_pi_program;
use crate::coding::runtime_location;
use crate::db::SqliteDbState;

/// Pi 与共享实现之间的全部差异。
pub const PI_EXTENSION_SPEC: CliExtensionSpec = CliExtensionSpec {
    cli_name: "pi",
    env_key: "PI_CODING_AGENT_DIR",
    offline_env_key: "PI_OFFLINE",
    extensions_dir: "extensions",
    protected_prefixes: &["pi-deck-", "ai-toolbox-"],
    resource_type_keys: &["extensions", "skills", "prompts", "themes"],
    scan_timeout: std::time::Duration::from_secs(20),
    wsl_sync_event: "wsl-sync-request-pi",
};

fn to_pi_scope(scope: CliExtensionScope) -> PiExtensionScope {
    match scope {
        CliExtensionScope::User => PiExtensionScope::User,
        CliExtensionScope::Project => PiExtensionScope::Project,
        CliExtensionScope::Unknown => PiExtensionScope::Unknown,
    }
}

fn from_pi_scope(scope: PiExtensionScope) -> CliExtensionScope {
    match scope {
        PiExtensionScope::User => CliExtensionScope::User,
        PiExtensionScope::Project => CliExtensionScope::Project,
        PiExtensionScope::Unknown => CliExtensionScope::Unknown,
    }
}

fn to_pi_kind(kind: CliExtensionKind) -> PiExtensionKind {
    match kind {
        CliExtensionKind::Package => PiExtensionKind::Package,
        CliExtensionKind::LocalFile => PiExtensionKind::LocalFile,
        CliExtensionKind::LocalDirectory => PiExtensionKind::LocalDirectory,
    }
}

fn from_pi_kind(kind: PiExtensionKind) -> CliExtensionKind {
    match kind {
        PiExtensionKind::Package => CliExtensionKind::Package,
        PiExtensionKind::LocalFile => CliExtensionKind::LocalFile,
        PiExtensionKind::LocalDirectory => CliExtensionKind::LocalDirectory,
    }
}

fn to_pi_summary(summary: shared::CliExtensionSummary) -> PiExtensionSummary {
    PiExtensionSummary {
        id: summary.id,
        source: summary.source,
        scope: to_pi_scope(summary.scope),
        kind: to_pi_kind(summary.kind),
        path: summary.path,
        built_in: summary.built_in,
        current_version: summary.current_version,
        latest_version: summary.latest_version,
        update_available: summary.update_available,
        enabled: summary.enabled,
        switch_supported: summary.switch_supported,
    }
}

/// Pi 的宿主侧二进制与 runtime location。
struct PiExtensionContext {
    program: shared::ResolvedCliProgram,
    runtime_location: runtime_location::RuntimeLocationInfo,
    settings_path: std::path::PathBuf,
}

async fn resolve_context(db: &SqliteDbState) -> Result<PiExtensionContext, String> {
    let runtime_location = runtime_location::get_pi_runtime_location_async(db).await?;
    Ok(PiExtensionContext {
        program: shared::ResolvedCliProgram {
            path: resolve_local_pi_program().path,
        },
        runtime_location,
        settings_path: get_pi_settings_path_async(db).await?,
    })
}

#[tauri::command]
pub async fn list_pi_extensions(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<PiExtensionListResult, String> {
    let db = state.db();
    let context = resolve_context(&db).await?;
    let result = shared::list_cli_extensions(
        &PI_EXTENSION_SPEC,
        &db,
        &context.program,
        &context.runtime_location,
        &context.settings_path,
    )
    .await?;

    Ok(PiExtensionListResult {
        extensions_path: result.extensions_path,
        packages_path: result.packages_path,
        extensions: result.extensions.into_iter().map(to_pi_summary).collect(),
        raw: result.raw,
        cli_path: result.cli_path,
        cli_version: result.cli_version,
    })
}

#[tauri::command]
pub async fn install_pi_extension(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: PiExtensionInstallInput,
) -> Result<PiExtensionCommandResult, String> {
    let db = state.db();
    let context = resolve_context(&db).await?;
    let result = shared::install_cli_extension(
        &PI_EXTENSION_SPEC,
        &app,
        "pi-extensions",
        &context.program,
        &context.runtime_location,
        &shared::CliExtensionInstallInput {
            source: input.source,
        },
    )
    .await?;
    Ok(PiExtensionCommandResult {
        command: result.command,
        output: result.output,
    })
}

#[tauri::command]
pub async fn uninstall_pi_extension(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: PiExtensionActionInput,
) -> Result<PiExtensionCommandResult, String> {
    let db = state.db();
    let context = resolve_context(&db).await?;
    let result = shared::uninstall_cli_extension(
        &PI_EXTENSION_SPEC,
        &app,
        "pi-extensions",
        &context.program,
        &context.runtime_location,
        &shared::CliExtensionActionInput {
            source: input.source,
            scope: input.scope.map(from_pi_scope),
            kind: input.kind.map(from_pi_kind),
            path: input.path,
        },
    )
    .await?;
    Ok(PiExtensionCommandResult {
        command: result.command,
        output: result.output,
    })
}

#[tauri::command]
pub async fn update_pi_extensions(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: Option<PiExtensionUpdateInput>,
) -> Result<PiExtensionCommandResult, String> {
    let db = state.db();
    let context = resolve_context(&db).await?;
    let shared_input = input.map(|value| shared::CliExtensionUpdateInput {
        source: value.source,
    });
    let result = shared::update_cli_extensions(
        &PI_EXTENSION_SPEC,
        &app,
        "pi-extensions",
        &context.program,
        &context.runtime_location,
        shared_input.as_ref(),
    )
    .await?;
    Ok(PiExtensionCommandResult {
        command: result.command,
        output: result.output,
    })
}

#[tauri::command]
pub async fn set_pi_extension_enabled(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: PiExtensionEnabledInput,
) -> Result<(), String> {
    let db = state.db();
    let settings_path = get_pi_settings_path_async(&db).await?;
    shared::set_cli_extension_enabled(
        &PI_EXTENSION_SPEC,
        &app,
        "pi-extensions",
        &settings_path,
        &shared::CliExtensionEnabledInput {
            source: input.source,
            kind: from_pi_kind(input.kind),
            enabled: input.enabled,
        },
    )
    .await
}

/// 供别处（如 WSL / SSH 同步）查询本地扩展目录用。
pub fn get_pi_extensions_path_from_root(root_dir: &Path) -> std::path::PathBuf {
    shared::extensions_path_from_root(root_dir, &PI_EXTENSION_SPEC)
}

pub fn get_pi_packages_path_from_root(root_dir: &Path) -> std::path::PathBuf {
    shared::packages_path_from_root(root_dir)
}

pub async fn get_pi_extensions_path_async(
    db: &SqliteDbState,
) -> Result<std::path::PathBuf, String> {
    Ok(get_pi_extensions_path_from_root(
        &runtime_location::get_pi_runtime_location_async(db)
            .await?
            .host_path,
    ))
}
