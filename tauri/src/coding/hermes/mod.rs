//! Hermes Agent backend module.
//!
//! Looks like pi in shape: a single-config-file tool whose provider fact
//! source lives in the runtime `config.yaml`. SQLite only stores the custom
//! config directory and prompt presets; it never holds provider records.

pub mod adapter;
pub mod commands;
pub mod constants;
pub mod tray_support;
pub mod types;
pub mod web_ui;

// NOTE: tray_support is intentionally NOT glob-reexported. Its
// `apply_hermes_prompt_config` would collide with the same-name
// `#[tauri::command]` in commands. Access it via `hermes::tray_support::*`
// (same convention as pi/oh_my_pi).
pub use commands::*;
