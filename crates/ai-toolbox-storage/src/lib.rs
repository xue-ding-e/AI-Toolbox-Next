//! Shared storage implementation; desktop adapters re-export these exact types.

pub mod backup;
pub mod change_hook;
pub mod codex_context_journal;
pub mod health;
pub mod helpers;
pub mod migrations;
pub mod model_pricing_seed;
pub mod schema;
pub mod sqlite_state;

/// Main SQLite database file name within the app data directory.
pub const SQLITE_DATABASE_FILE: &str = "ai-toolbox.db";

pub use sqlite_state::SqliteDbState;
