pub mod all_api_hub;
pub mod auth_refresh;
pub mod cc_switch;
pub mod claude_code;
pub mod claude_desktop;
pub mod cli_resolver;
pub mod codex;
pub mod config_cleanup;
pub mod deeplink;
pub mod dsh;
pub mod gemini_cli;
pub mod grok;
pub mod image;
pub mod kimi;
pub mod magic_context;
pub mod mcp;
pub mod oh_my_openagent;
pub mod oh_my_opencode_slim;
pub mod oh_my_pi;
pub mod hermes;
pub mod open_claw;
pub mod open_code;
pub mod pi;
pub mod preset_models;
pub mod proxy_gateway;
pub mod reapply_applied_runtime;
pub mod runtime_location;
pub mod session_manager;
pub mod skills;
pub mod ssh;
pub mod tools;
pub(crate) mod url_utils;
pub mod wsl;

mod db_id;
pub(crate) mod file_io;
#[cfg(test)]
pub(crate) mod test_env {
    use std::sync::{LazyLock, Mutex, MutexGuard};

    static TEST_ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    pub(crate) fn lock() -> MutexGuard<'static, ()> {
        // Recover from a poisoned lock instead of propagating the panic: other
        // tests still need to take this lock even after one holder panicked
        // (e.g. an optional CLI helper that only exists on some dev machines).
        TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

mod prompt_file;
pub use db_id::{
    db_build_id, db_clean_id, db_extract_id, db_extract_id_opt, db_new_id, db_record_id,
};

mod path_expand;
pub use path_expand::expand_local_path;
