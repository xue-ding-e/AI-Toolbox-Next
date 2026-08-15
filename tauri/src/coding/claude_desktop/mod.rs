//! Claude Desktop backend module.
//!
//! Claude Desktop is a "config file path module": it locates and rewrites the
//! Claude Desktop 3P profile files (`claude_desktop_config.json`,
//! `configLibrary/<PROFILE_ID>.json`, `configLibrary/_meta.json`) rather than
//! managing a CLI root directory.
//!
//! The module reuses the provider compiler pattern from `claude_code` but adapts
//! it to a config-file-path module. Only the `claude_desktop_provider` table is
//! used; there is no separate common/prompt table.

pub mod adapter;
pub mod commands;
pub mod config_writer;
pub mod constants;
pub mod prompt;
pub mod tray_support;
pub mod types;

pub use commands::*;
pub use prompt::*;
pub use types::*;
