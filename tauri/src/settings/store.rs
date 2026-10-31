use serde_json::Value;

use super::{
    adapter,
    types::{AppSettings, GatewayUsageRangeSelection, SessionDetailFilters},
};
use crate::db::helpers::{db_get, db_patch_fields, db_put};
use crate::db::schema::DbTable;
use crate::db::SqliteDbState;

pub const SETTINGS_ID: &str = "app";

pub fn load_settings_from_sqlite_state(
    sqlite_state: &SqliteDbState,
) -> Result<AppSettings, String> {
    sqlite_state.with_conn(load_settings_from_sqlite_conn)
}

pub async fn load_settings_from_sqlite_state_async(
    sqlite_state: &SqliteDbState,
) -> Result<AppSettings, String> {
    let sqlite_state = sqlite_state.clone();
    tauri::async_runtime::spawn_blocking(move || load_settings_from_sqlite_state(&sqlite_state))
        .await
        .map_err(|error| format!("Failed to join settings load task: {error}"))?
}

pub fn save_settings_to_sqlite_state(
    sqlite_state: &SqliteDbState,
    settings: &AppSettings,
) -> Result<(), String> {
    sqlite_state.with_conn(|conn| save_settings_to_sqlite_conn(conn, settings))
}

pub fn update_last_auto_backup_time_in_sqlite_state(
    sqlite_state: &SqliteDbState,
    time: &str,
) -> Result<(), String> {
    sqlite_state.with_conn(|conn| {
        let updated = db_patch_fields(
            conn,
            DbTable::Settings,
            SETTINGS_ID,
            &[("last_auto_backup_time", Value::String(time.to_string()))],
        )?;

        if updated.is_none() {
            let mut payload = adapter::to_db_value(&AppSettings::default());
            if let Some(object) = payload.as_object_mut() {
                object.insert(
                    "last_auto_backup_time".to_string(),
                    Value::String(time.to_string()),
                );
            }
            db_put(conn, DbTable::Settings, SETTINGS_ID, &payload)?;
        }

        Ok(())
    })
}

/// Return the stored session-detail filter visibility, or `None` when no record
/// exists yet (frontend falls back to "all visible").
pub fn get_session_detail_filters_from_sqlite_state(
    sqlite_state: &SqliteDbState,
) -> Result<Option<SessionDetailFilters>, String> {
    Ok(load_settings_from_sqlite_state(sqlite_state)?.session_detail_filters)
}

/// Keep the GUI range read's existing full-load effects and error propagation.
pub fn get_gateway_usage_range_from_sqlite_state(
    sqlite_state: &SqliteDbState,
) -> Result<GatewayUsageRangeSelection, String> {
    Ok(load_settings_from_sqlite_state(sqlite_state)?.gateway_usage_range)
}

/// Persist only the nested `session_detail_filters` key so rapid chip toggles
/// never clobber other settings fields (avoids a full `save_settings` race).
/// When the settings record is missing, create it with defaults then patch.
pub fn save_session_detail_filters_to_sqlite_state(
    sqlite_state: &SqliteDbState,
    filters: &SessionDetailFilters,
) -> Result<(), String> {
    let value =
        serde_json::to_value(filters).map_err(|error| format!("serialize filters: {error}"))?;
    sqlite_state.with_conn(|conn| {
        let updated = db_patch_fields(
            conn,
            DbTable::Settings,
            SETTINGS_ID,
            &[("session_detail_filters", value.clone())],
        )?;

        if updated.is_none() {
            let mut payload = adapter::to_db_value(&AppSettings::default());
            if let Some(object) = payload.as_object_mut() {
                object.insert("session_detail_filters".to_string(), value.clone());
            }
            db_put(conn, DbTable::Settings, SETTINGS_ID, &payload)?;
        }

        Ok(())
    })
}

/// Persist the gateway statistics page's time range, patching only its own key.
///
/// The statistics page writes this on every filter change and the tray reads it
/// when rebuilding the usage block. A full-settings write would race with the
/// settings panel's debounced save and could revert unrelated fields.
pub fn save_gateway_usage_range_to_sqlite_state(
    sqlite_state: &SqliteDbState,
    range: &GatewayUsageRangeSelection,
) -> Result<(), String> {
    let value = serde_json::to_value(range)
        .map_err(|error| format!("serialize gateway usage range: {error}"))?;
    super::provider_list_state::patch_or_create_settings_field(
        sqlite_state,
        "gateway_usage_range",
        value,
    )
}

pub fn load_settings_from_sqlite_conn(conn: &rusqlite::Connection) -> Result<AppSettings, String> {
    let record = db_get(conn, DbTable::Settings, SETTINGS_ID)?;
    let settings = record.map(adapter::from_db_value).unwrap_or_default();
    sync_manual_cli_overrides(&settings);
    Ok(settings)
}

pub fn save_settings_to_sqlite_conn(
    conn: &rusqlite::Connection,
    settings: &AppSettings,
) -> Result<(), String> {
    let json = adapter::to_db_value(settings);
    db_put(conn, DbTable::Settings, SETTINGS_ID, &json)?;
    sync_manual_cli_overrides(settings);
    Ok(())
}

/// Keep the in-process manual CLI override registry in `cli_resolver` aligned
/// with persisted settings so CLI calls prefer user-specified paths.
fn sync_manual_cli_overrides(settings: &AppSettings) {
    // Under `cargo test` every settings load/save funnels through here and would
    // otherwise wipe the shared registry mid-test for `cli_resolver`'s manual-cli
    // tests (which set an override then immediately resolve it). Hold the shared
    // `test_env` lock so those writers are serialized with the reader tests there.
    // No-op overhead in production builds: this branch never compiles in.
    #[cfg(test)]
    let _guard = crate::coding::test_env::lock();

    crate::coding::cli_resolver::set_manual_cli_overrides(settings.cli_manual_paths.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SqliteDbState;

    #[test]
    fn sqlite_settings_round_trip_uses_adapter_defaults() {
        let sqlite_state = SqliteDbState::in_memory_for_test().expect("sqlite");

        let default_settings =
            load_settings_from_sqlite_state(&sqlite_state).expect("load default settings");
        assert_eq!(default_settings.theme, "system");
        assert_eq!(default_settings.proxy_mode, "system");
        assert!(default_settings.backup_image_assets_enabled);

        let mut settings = default_settings;
        settings.language = "en-US".to_string();
        settings.theme = "dark".to_string();
        settings.backup_image_assets_enabled = false;
        save_settings_to_sqlite_state(&sqlite_state, &settings).expect("save settings");

        let loaded = load_settings_from_sqlite_state(&sqlite_state).expect("reload settings");
        assert_eq!(loaded.language, "en-US");
        assert_eq!(loaded.theme, "dark");
        assert!(!loaded.backup_image_assets_enabled);
    }

    #[test]
    fn gateway_usage_range_round_trips_and_legacy_rows_read_as_today() {
        let sqlite_state = SqliteDbState::in_memory_for_test().expect("sqlite");

        // A row written before the field existed: the adapter must hand back
        // the statistics page's own default instead of an empty preset.
        sqlite_state
            .with_conn(|conn| {
                db_put(
                    conn,
                    DbTable::Settings,
                    SETTINGS_ID,
                    &serde_json::json!({ "theme": "dark" }),
                )
            })
            .expect("insert legacy row");
        let legacy = load_settings_from_sqlite_state(&sqlite_state).expect("load legacy");
        assert_eq!(
            legacy.gateway_usage_range,
            GatewayUsageRangeSelection::default()
        );
        assert_eq!(legacy.gateway_usage_range.preset, "today");

        save_gateway_usage_range_to_sqlite_state(
            &sqlite_state,
            &GatewayUsageRangeSelection {
                preset: "custom".to_string(),
                start_date: Some(1_700_000_000),
                end_date: Some(1_700_086_400),
            },
        )
        .expect("save custom range");
        let loaded = load_settings_from_sqlite_state(&sqlite_state).expect("reload");
        assert_eq!(loaded.gateway_usage_range.preset, "custom");
        assert_eq!(loaded.gateway_usage_range.start_date, Some(1_700_000_000));
        assert_eq!(loaded.gateway_usage_range.end_date, Some(1_700_086_400));
        // The single-key patch leaves the rest of the row alone.
        assert_eq!(loaded.theme, "dark");

        // Switching back to a relative preset clears the absolute bounds.
        save_gateway_usage_range_to_sqlite_state(
            &sqlite_state,
            &GatewayUsageRangeSelection::default(),
        )
        .expect("save today");
        let reset = load_settings_from_sqlite_state(&sqlite_state).expect("reload today");
        assert_eq!(reset.gateway_usage_range.preset, "today");
        assert_eq!(reset.gateway_usage_range.start_date, None);
        assert_eq!(reset.gateway_usage_range.end_date, None);
    }

    #[test]
    fn keep_awake_preference_round_trips_and_old_records_default_to_disabled() {
        let sqlite_state = SqliteDbState::in_memory_for_test().expect("sqlite");
        sqlite_state
            .with_conn(|conn| {
                db_put(
                    conn,
                    DbTable::Settings,
                    SETTINGS_ID,
                    &serde_json::json!({ "theme": "dark" }),
                )?;
                Ok(())
            })
            .unwrap();

        let mut settings = load_settings_from_sqlite_state(&sqlite_state).unwrap();
        assert!(!settings.keep_computer_awake);
        for enabled in [true, false] {
            settings.keep_computer_awake = enabled;
            save_settings_to_sqlite_state(&sqlite_state, &settings).unwrap();
            settings = load_settings_from_sqlite_state(&sqlite_state).unwrap();
            assert_eq!(settings.keep_computer_awake, enabled);
            assert_eq!(settings.theme, "dark");
        }
    }

    #[test]
    fn sqlite_last_auto_backup_time_update_creates_or_patches_settings() {
        let sqlite_state = SqliteDbState::in_memory_for_test().expect("sqlite");

        update_last_auto_backup_time_in_sqlite_state(&sqlite_state, "2026-05-19T00:00:00Z")
            .expect("create last backup time");
        let created = load_settings_from_sqlite_state(&sqlite_state).expect("load created");
        assert_eq!(
            created.last_auto_backup_time.as_deref(),
            Some("2026-05-19T00:00:00Z")
        );

        update_last_auto_backup_time_in_sqlite_state(&sqlite_state, "2026-05-20T00:00:00Z")
            .expect("patch last backup time");
        let patched = load_settings_from_sqlite_state(&sqlite_state).expect("load patched");
        assert_eq!(
            patched.last_auto_backup_time.as_deref(),
            Some("2026-05-20T00:00:00Z")
        );
    }

    #[test]
    fn sqlite_session_detail_filters_round_trip_and_create_missing_record() {
        let sqlite_state = SqliteDbState::in_memory_for_test().expect("sqlite");

        // No record yet: read returns None, write creates the settings singleton.
        assert_eq!(
            get_session_detail_filters_from_sqlite_state(&sqlite_state).expect("read empty"),
            None
        );

        let filters = SessionDetailFilters {
            role_filter: crate::settings::types::SessionRoleFilter {
                user: false,
                assistant: true,
            },
            content_filter: crate::settings::types::SessionContentFilter {
                text: true,
                thinking: false,
                tool_call: true,
                command: false,
            },
        };
        save_session_detail_filters_to_sqlite_state(&sqlite_state, &filters)
            .expect("create filters");
        assert_eq!(
            get_session_detail_filters_from_sqlite_state(&sqlite_state).expect("read saved"),
            Some(filters.clone())
        );

        // Patching must not clobber other settings fields.
        let mut patched = filters;
        patched.role_filter.user = true;
        save_session_detail_filters_to_sqlite_state(&sqlite_state, &patched)
            .expect("patch filters");
        let loaded = load_settings_from_sqlite_state(&sqlite_state).expect("load settings");
        assert_eq!(loaded.session_detail_filters, Some(patched));
        // Other settings survive the nested patch.
        assert_eq!(loaded.theme, "system");
    }

    #[test]
    fn narrow_ui_preference_registry_actor() {
        if std::env::var_os("AI_TOOLBOX_PREFERENCE_READ_ACTOR").as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            return;
        }
        // Run alone: full load/save refreshes the process registry under
        // test_env's lock, so an outer same-thread lock would deadlock.
        struct ResetRegistry;
        impl Drop for ResetRegistry {
            fn drop(&mut self) {
                crate::coding::cli_resolver::set_manual_cli_overrides(Default::default());
            }
        }
        let _reset = ResetRegistry;
        let db = SqliteDbState::in_memory_for_test().expect("synthetic sqlite");
        for read in 0..3 {
            crate::coding::cli_resolver::set_manual_cli_overrides(
                [(
                    "synthetic-memory-tool".to_string(),
                    "synthetic-memory-path".to_string(),
                )]
                .into_iter()
                .collect(),
            );
            db.with_conn(|connection| {
                db_put(
                    connection,
                    DbTable::Settings,
                    SETTINGS_ID,
                    &serde_json::json!({}),
                )?;
                // Make only this in-memory fixture's table unavailable. Do not
                // disable JSONB CHECK constraints to manufacture a read failure.
                connection
                    .execute_batch("ALTER TABLE settings RENAME TO synthetic_unavailable_settings")
                    .map_err(|error| error.to_string())?;
                Ok(())
            })
            .expect("seed failing startup read");
            let startup_error = load_settings_from_sqlite_state(&db).unwrap_err();
            let narrow_error = match read {
                0 => get_session_detail_filters_from_sqlite_state(&db)
                    .map(|_| ())
                    .unwrap_err(),
                1 => get_gateway_usage_range_from_sqlite_state(&db)
                    .map(|_| ())
                    .unwrap_err(),
                _ => crate::settings::provider_list_state::get_provider_list_state(&db)
                    .map(|_| ())
                    .unwrap_err(),
            };
            assert_eq!(narrow_error, startup_error);
            assert!(crate::coding::cli_resolver::manual_cli_path_configured(
                "synthetic-memory-tool"
            ));
            db.with_conn(|connection| {
                connection
                    .execute_batch("ALTER TABLE synthetic_unavailable_settings RENAME TO settings")
                    .map_err(|error| error.to_string())?;
                db_put(
                    connection,
                    DbTable::Settings,
                    SETTINGS_ID,
                    &serde_json::json!({
                        "cli_manual_paths": {"synthetic-disk-tool": "synthetic-disk-path"}
                    }),
                )
            })
            .expect("restore fixture without registry sync");
            match read {
                0 => {
                    get_session_detail_filters_from_sqlite_state(&db).expect("filters");
                }
                1 => {
                    get_gateway_usage_range_from_sqlite_state(&db).expect("range");
                }
                _ => {
                    crate::settings::provider_list_state::get_provider_list_state(&db)
                        .expect("providers");
                }
            }
            assert!(!crate::coding::cli_resolver::manual_cli_path_configured(
                "synthetic-memory-tool"
            ));
            assert!(crate::coding::cli_resolver::manual_cli_path_configured(
                "synthetic-disk-tool"
            ));
        }
        println!("synthetic-ui-preferences-registry-restored");
    }

    #[test]
    fn narrow_ui_preference_reads_restore_manual_cli_overrides_after_read_failure() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let fixture = tempfile::tempdir().expect("synthetic child root");
        let stdout_path = fixture.path().join("stdout");
        let stderr_path = fixture.path().join("stderr");
        let mut command = Command::new(std::env::current_exe().expect("libtest executable"));
        command
            .args([
                "--exact",
                "settings::store::tests::narrow_ui_preference_registry_actor",
                "--nocapture",
            ])
            .env_clear()
            .env("AI_TOOLBOX_PREFERENCE_READ_ACTOR", "1")
            .env("HOME", fixture.path().join("missing-home"))
            .env("USERPROFILE", fixture.path().join("missing-home"))
            .env("APPDATA", fixture.path().join("missing-appdata"))
            .env("LOCALAPPDATA", fixture.path().join("missing-localappdata"))
            .env("XDG_CONFIG_HOME", fixture.path().join("missing-config"))
            .env("XDG_DATA_HOME", fixture.path().join("missing-data"))
            .env("XDG_CACHE_HOME", fixture.path().join("missing-cache"))
            .env("TMPDIR", fixture.path())
            .env("TEMP", fixture.path())
            .env("TMP", fixture.path())
            .stdin(Stdio::null())
            .stdout(std::fs::File::create(&stdout_path).unwrap())
            .stderr(std::fs::File::create(&stderr_path).unwrap());
        // Windows runtime location, not a user profile or credential input.
        if let Some(system_root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system_root);
        }
        let mut child = command.spawn().expect("isolated registry actor");
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                outcome => {
                    let kill = child.kill();
                    let cleanup_deadline = Instant::now() + Duration::from_secs(2);
                    let cleanup = loop {
                        match child.try_wait() {
                            Ok(Some(status)) => break format!("reaped: {status}"),
                            Ok(None) if Instant::now() < cleanup_deadline => {
                                std::thread::sleep(Duration::from_millis(20));
                            }
                            result => break format!("cleanup not confirmed: {result:?}"),
                        }
                    };
                    // Keep synthetic diagnostics if cleanup could not be established;
                    // never use an unbounded wait after a failed observation.
                    let retained = fixture.path().to_path_buf();
                    std::mem::forget(fixture);
                    panic!("registry actor observation {outcome:?}; kill {kill:?}; {cleanup}; fixture {retained:?}");
                }
            }
        };
        let stdout = std::fs::read_to_string(stdout_path).unwrap();
        let stderr = std::fs::read_to_string(stderr_path).unwrap();
        assert!(status.success(), "registry actor failed: {stdout} {stderr}");
        assert!(stdout.contains("synthetic-ui-preferences-registry-restored"));
        for name in [
            "missing-home",
            "missing-appdata",
            "missing-localappdata",
            "missing-config",
            "missing-data",
            "missing-cache",
        ] {
            assert!(
                !fixture.path().join(name).exists(),
                "unexpected child state: {name}"
            );
        }
    }

    #[test]
    fn narrow_ui_preference_wrappers_match_the_full_adapter_projection() {
        let db = SqliteDbState::in_memory_for_test().expect("synthetic sqlite");
        for record in [
            serde_json::json!({}),
            serde_json::json!({
                "session_detail_filters": {"role_filter": {"user": false}},
                "gateway_usage_range": {"preset": " custom ", "start_date": 7},
                "provider_sort_modes": {"future-module": "future-sort", "bad": false},
                "provider_last_used": {"future-module:provider": "existing-value"},
                "provider_view_modes": {"future-module": "card"},
                "webdav": {"password": "synthetic-unrelated-private"}
            }),
            serde_json::json!({
                "session_detail_filters": null,
                "gateway_usage_range": false,
                "provider_view_modes": []
            }),
        ] {
            db.with_conn(|connection| db_put(connection, DbTable::Settings, SETTINGS_ID, &record))
                .expect("seed preference fixture");
            let expected = adapter::from_db_value(record);
            assert_eq!(
                get_session_detail_filters_from_sqlite_state(&db).unwrap(),
                expected.session_detail_filters
            );
            assert_eq!(
                get_gateway_usage_range_from_sqlite_state(&db).unwrap(),
                expected.gateway_usage_range
            );
            assert_eq!(
                crate::settings::provider_list_state::get_provider_list_state(&db).unwrap(),
                crate::settings::types::ProviderListState {
                    sort_modes: expected.provider_sort_modes,
                    last_used: expected.provider_last_used,
                    view_modes: expected.provider_view_modes,
                }
            );
        }
    }
}
