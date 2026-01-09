#[allow(unused_imports)]
use tauri::Manager;

use std::fs;
use std::sync::Arc;
use surrealdb::engine::local::SurrealKv;
use surrealdb::Surreal;
use tokio::sync::Mutex;

// Module declarations
pub mod coding;
pub mod db;
pub mod settings;
pub mod update;

// Re-export DbState for use in other modules
pub use db::DbState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Create app data directory
            let app_data_dir = app_handle
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");

            if !app_data_dir.exists() {
                fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");
            }

            let db_path = app_data_dir.join("database");

            // Initialize SurrealDB
            tauri::async_runtime::block_on(async {
                let db = Surreal::new::<SurrealKv>(db_path)
                    .await
                    .expect("Failed to initialize SurrealDB");

                db.use_ns("ai_toolbox")
                    .use_db("main")
                    .await
                    .expect("Failed to select namespace and database");

                // Run database migrations
                db::run_migrations(&db)
                    .await
                    .expect("Failed to run database migrations");

                app.manage(DbState(Arc::new(Mutex::new(db))));
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Update
            update::check_for_updates,
            // Settings
            settings::get_settings,
            settings::save_settings,
            // Backup - Local
            settings::backup::backup_database,
            settings::backup::restore_database,
            settings::backup::get_database_path,
            settings::backup::open_app_data_dir,
            // Backup - WebDAV
            settings::backup::backup_to_webdav,
            settings::backup::list_webdav_backups,
            settings::backup::restore_from_webdav,
            // Claude Code
            coding::claude_code::list_claude_providers,
            coding::claude_code::create_claude_provider,
            coding::claude_code::update_claude_provider,
            coding::claude_code::delete_claude_provider,
            coding::claude_code::select_claude_provider,
            coding::claude_code::reorder_claude_providers,
            coding::claude_code::get_claude_config_path,
            coding::claude_code::reveal_claude_config_folder,
            coding::claude_code::read_claude_settings,
            coding::claude_code::apply_claude_config,
            coding::claude_code::get_claude_common_config,
            coding::claude_code::save_claude_common_config,
            // OpenCode
            coding::open_code::get_opencode_config_path,
            coding::open_code::get_opencode_config_path_info,
            coding::open_code::read_opencode_config,
            coding::open_code::save_opencode_config,
            coding::open_code::get_opencode_common_config,
            coding::open_code::save_opencode_common_config,
            // Oh My OpenCode
            coding::oh_my_opencode::list_oh_my_opencode_configs,
            coding::oh_my_opencode::create_oh_my_opencode_config,
            coding::oh_my_opencode::update_oh_my_opencode_config,
            coding::oh_my_opencode::delete_oh_my_opencode_config,
            coding::oh_my_opencode::apply_oh_my_opencode_config,
            coding::oh_my_opencode::reorder_oh_my_opencode_configs,
            coding::oh_my_opencode::get_oh_my_opencode_config_path_info,
            coding::oh_my_opencode::get_oh_my_opencode_global_config,
            coding::oh_my_opencode::save_oh_my_opencode_global_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
