// Preserve desktop import paths without a second storage implementation.
pub use ai_toolbox_storage::{
    backup, change_hook, health, helpers, migrations, schema, sqlite_state, SqliteDbState,
    SQLITE_DATABASE_FILE,
};

// Only remote HTTP synchronization belongs to the desktop adapter.
pub mod model_pricing_seed;
