use super::*;
use crate::helpers::db_put;
use crate::SqliteDbState;
use serde_json::json;

fn read_all(connection: &Connection) -> Value {
    json!({
        "filters": read_session_detail_filters(connection).unwrap(),
        "range": read_gateway_usage_range(connection).unwrap(),
        "providers": read_provider_list_state(connection).unwrap(),
    })
}

#[test]
fn missing_record_reads_defaults_without_creating_or_writing() {
    let db = SqliteDbState::in_memory_for_test().unwrap();
    db.with_conn(|connection| {
        let changes = connection.total_changes();
        assert_eq!(
            read_all(connection),
            json!({
                "filters": null,
                "range": {"preset": "today", "start_date": null, "end_date": null},
                "providers": {},
            })
        );
        assert!(db_get(connection, DbTable::Settings, SETTINGS_ID)?.is_none());
        assert_eq!(connection.total_changes(), changes);
        Ok(())
    })
    .unwrap();
}

#[test]
fn existing_partial_values_preserve_tolerant_decoding_and_dynamic_keys() {
    let value = json!({
        "session_detail_filters": {
            "role_filter": {"user": false, "assistant": "invalid"},
            "content_filter": {"text": false, "thinking": null, "tool_call": 0}
        },
        "gateway_usage_range": {"preset": " custom ", "start_date": -2, "end_date": "invalid"},
        "provider_sort_modes": {"new_module": "future_mode", "invalid": 7},
        "provider_last_used": {"new_module:provider": "existing-string-not-revalidated"},
        "provider_view_modes": {"new_module": "", "invalid": null}
    });
    let db = SqliteDbState::in_memory_for_test().unwrap();
    db.with_conn(|connection| {
        db_put(connection, DbTable::Settings, SETTINGS_ID, &value)?;
        let before = db_get(connection, DbTable::Settings, SETTINGS_ID)?;
        let changes = connection.total_changes();
        assert_eq!(read_all(connection), json!({
            "filters": {
                "role_filter": {"user": false, "assistant": true},
                "content_filter": {"text": false, "thinking": true, "tool_call": true, "command": true}
            },
            "range": {"preset": "custom", "start_date": -2, "end_date": null},
            "providers": {
                "sort_modes": {"new_module": "future_mode"},
                "last_used": {"new_module:provider": "existing-string-not-revalidated"},
                "view_modes": {"new_module": ""}
            }
        }));
        assert_eq!(db_get(connection, DbTable::Settings, SETTINGS_ID)?, before);
        assert_eq!(connection.total_changes(), changes);
        Ok(())
    }).unwrap();
}

#[test]
fn missing_null_and_wrong_type_blocks_keep_the_original_distinctions() {
    for value in [
        Value::Null,
        json!({}),
        json!({
            "session_detail_filters": null,
            "gateway_usage_range": [],
            "provider_sort_modes": "invalid"
        }),
        json!({"session_detail_filters": false}),
    ] {
        assert_eq!(session_detail_filters_from_db_value(&value), None);
        assert_eq!(
            gateway_usage_range_from_db_value(&value),
            GatewayUsageRangeSelection::default()
        );
        assert_eq!(
            provider_list_state_from_db_value(&value),
            ProviderListState::default()
        );
    }
    assert_eq!(
        session_detail_filters_from_db_value(&json!({
            "session_detail_filters": {"role_filter": [], "content_filter": false}
        })),
        Some(SessionDetailFilters::default())
    );
    assert_eq!(
        gateway_usage_range_from_db_value(&json!({
            "gateway_usage_range": {"preset": "  ", "start_date": 7, "end_date": 9}
        })),
        GatewayUsageRangeSelection {
            preset: "today".into(),
            start_date: Some(7),
            end_date: Some(9)
        }
    );
}

#[test]
fn unrelated_secret_fields_never_enter_the_read_projection() {
    let db = SqliteDbState::in_memory_for_test().unwrap();
    db.with_conn(|connection| {
        db_put(connection, DbTable::Settings, SETTINGS_ID, &json!({
            "webdav": {"password": "synthetic-webdav-private"},
            "s3": {"secret_key": "synthetic-s3-private"},
            "proxy_url": "https://synthetic-user:synthetic-password@example.invalid",
            "cli_manual_paths": {"tool": "synthetic-private-path"},
            "unknown": "synthetic-unknown-private",
            "session_detail_filters": {},
            "gateway_usage_range": {"preset": "today"}
        }))?;
        let projected = read_all(connection);
        assert_eq!(projected, json!({
            "filters": {
                "role_filter": {"user": true, "assistant": true},
                "content_filter": {"text": true, "thinking": true, "tool_call": true, "command": true}
            },
            "range": {"preset": "today", "start_date": null, "end_date": null},
            "providers": {}
        }));
        assert!(!projected.to_string().contains("synthetic-"));
        Ok(())
    }).unwrap();
}

#[test]
fn readers_use_the_supplied_connections_uncommitted_transaction() {
    let db = SqliteDbState::in_memory_for_test().unwrap();
    db.with_conn(|connection| {
        connection.execute_batch("BEGIN").unwrap();
        db_put(
            connection,
            DbTable::Settings,
            SETTINGS_ID,
            &json!({
                "session_detail_filters": {"role_filter": {"user": false}},
                "gateway_usage_range": {"preset": "synthetic-uncommitted"},
                "provider_view_modes": {"synthetic-module": "card"}
            }),
        )?;
        assert!(
            !read_session_detail_filters(connection)?
                .unwrap()
                .role_filter
                .user
        );
        assert_eq!(
            read_gateway_usage_range(connection)?.preset,
            "synthetic-uncommitted"
        );
        assert_eq!(
            read_provider_list_state(connection)?.view_modes["synthetic-module"],
            "card"
        );
        connection.execute_batch("ROLLBACK").unwrap();
        assert!(read_session_detail_filters(connection)?.is_none());
        assert_eq!(
            read_gateway_usage_range(connection)?,
            GatewayUsageRangeSelection::default()
        );
        assert_eq!(
            read_provider_list_state(connection)?,
            ProviderListState::default()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn unavailable_storage_reports_only_the_static_read_error() {
    // Deliberately no schema, without the storage initializer or any profile.
    let connection = Connection::open_in_memory().unwrap();
    assert_eq!(
        read_session_detail_filters(&connection).unwrap_err(),
        "Unable to read UI preferences"
    );
    assert_eq!(
        read_gateway_usage_range(&connection).unwrap_err(),
        "Unable to read UI preferences"
    );
    assert_eq!(
        read_provider_list_state(&connection).unwrap_err(),
        "Unable to read UI preferences"
    );
    assert_eq!(connection.total_changes(), 0);
}
