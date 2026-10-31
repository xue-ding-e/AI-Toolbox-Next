//! Narrow persisted UI-preference reads over an already-open shared connection.
//!
//! This service does not open/initialize storage, acquire ownership, synchronize
//! CLI overrides, or expose an IPC/CLI/MCP endpoint. The caller owns admission.
//! Full application settings and credential-bearing fields stay out of its DTOs.

use std::collections::HashMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::helpers::db_get;
use crate::schema::DbTable;

const SETTINGS_ID: &str = "app";

/// The gateway statistics page's time-range selection, persisted so the tray
/// usage block can follow the same window.
///
/// `preset` is one of the frontend `GATEWAY_USAGE_RANGE_PRESETS` values. Only
/// `custom` carries absolute bounds (`start_date` / `end_date`, unix seconds);
/// relative presets are re-resolved from the current local clock every time the
/// tray menu is built, so "today"/"this week" never freeze at save time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayUsageRangeSelection {
    pub preset: String,
    /// Unix seconds; only meaningful when `preset == "custom"`.
    #[serde(default)]
    pub start_date: Option<i64>,
    /// Unix seconds; only meaningful when `preset == "custom"`.
    #[serde(default)]
    pub end_date: Option<i64>,
}

impl Default for GatewayUsageRangeSelection {
    fn default() -> Self {
        // Matches the statistics page's own default (`{ preset: 'today' }`).
        Self {
            preset: "today".to_string(),
            start_date: None,
            end_date: None,
        }
    }
}

/// Session-detail filter chip visibility persisted across app restarts.
///
/// Mirrors the frontend `SessionDetailWorkbench` role/content filters. All
/// booleans are "visible" (true = show). Missing/None means "all visible".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDetailFilters {
    pub role_filter: SessionRoleFilter,
    pub content_filter: SessionContentFilter,
}

impl Default for SessionDetailFilters {
    fn default() -> Self {
        Self {
            role_filter: SessionRoleFilter::default(),
            content_filter: SessionContentFilter::default(),
        }
    }
}

/// Role-level visibility: user vs assistant messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRoleFilter {
    pub user: bool,
    pub assistant: bool,
}

impl Default for SessionRoleFilter {
    fn default() -> Self {
        Self {
            user: true,
            assistant: true,
        }
    }
}

/// Content-level visibility: text, thinking, tool calls, commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionContentFilter {
    pub text: bool,
    pub thinking: bool,
    pub tool_call: bool,
    pub command: bool,
}

impl Default for SessionContentFilter {
    fn default() -> Self {
        Self {
            text: true,
            thinking: true,
            tool_call: true,
            command: true,
        }
    }
}

/// Combined read model for the provider list UI: per-module sort preference,
/// the last-used timestamp map consumed by the "recently used" sort mode, and
/// the layout preference (list vs compact card grid).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProviderListState {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub sort_modes: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub last_used: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub view_modes: HashMap<String, String>,
}

/// Read the persisted gateway statistics range. A row written before this field
/// existed reads back as the statistics page's own default (`today`) instead of
/// an empty preset, so the tray never has to guess what "no selection" means.
pub fn gateway_usage_range_from_db_value(value: &Value) -> GatewayUsageRangeSelection {
    let Some(block) = value.get("gateway_usage_range").and_then(|v| v.as_object()) else {
        return GatewayUsageRangeSelection::default();
    };
    let preset = block
        .get("preset")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|preset| !preset.is_empty())
        .unwrap_or("today");
    GatewayUsageRangeSelection {
        preset: preset.to_string(),
        start_date: block.get("start_date").and_then(|v| v.as_i64()),
        end_date: block.get("end_date").and_then(|v| v.as_i64()),
    }
}

fn get_string_map(value: &Value, key: &str) -> HashMap<String, String> {
    value
        .get(key)
        .and_then(|v| v.as_object())
        .map(|object| {
            object
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

// Extracts the session-detail filter toggles from the stored settings object.
// If the whole `session_detail_filters` key is absent (upgrading from a build
// that never persisted them) returns None so the workbench falls back to
// defaults. If the key exists but only part of the nested booleans is present,
// missing keys fall back to their per-type default (all visible) instead of
// dropping the whole block.
pub fn session_detail_filters_from_db_value(value: &Value) -> Option<SessionDetailFilters> {
    let block = value.get("session_detail_filters")?.as_object()?;

    let role_block = block
        .get("role_filter")
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let role_filter = SessionRoleFilter {
        user: role_block
            .get("user")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        assistant: role_block
            .get("assistant")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
    };

    let content_block = block
        .get("content_filter")
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let content_filter = SessionContentFilter {
        text: content_block
            .get("text")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        thinking: content_block
            .get("thinking")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        tool_call: content_block
            .get("tool_call")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        command: content_block
            .get("command")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
    };

    Some(SessionDetailFilters {
        role_filter,
        content_filter,
    })
}

/// Preserve arbitrary existing module keys and string values; this is a UI
/// projection, not a new module allowlist or semantic secret detector.
pub fn provider_list_state_from_db_value(value: &Value) -> ProviderListState {
    ProviderListState {
        sort_modes: get_string_map(value, "provider_sort_modes"),
        last_used: get_string_map(value, "provider_last_used"),
        view_modes: get_string_map(value, "provider_view_modes"),
    }
}

fn read_record(connection: &Connection) -> Result<Value, String> {
    // Keep the raw row internal: it may also contain backup/proxy credentials.
    // Missing records are decoded as absent keys and are never created here.
    db_get(connection, DbTable::Settings, SETTINGS_ID)
        .map(|record| record.unwrap_or(Value::Null))
        .map_err(|_| "Unable to read UI preferences".to_string())
}

pub fn read_session_detail_filters(
    connection: &Connection,
) -> Result<Option<SessionDetailFilters>, String> {
    Ok(session_detail_filters_from_db_value(&read_record(
        connection,
    )?))
}

pub fn read_gateway_usage_range(
    connection: &Connection,
) -> Result<GatewayUsageRangeSelection, String> {
    Ok(gateway_usage_range_from_db_value(&read_record(connection)?))
}

pub fn read_provider_list_state(connection: &Connection) -> Result<ProviderListState, String> {
    Ok(provider_list_state_from_db_value(&read_record(connection)?))
}

#[cfg(test)]
mod tests;
