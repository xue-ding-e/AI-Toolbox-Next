use ai_toolbox_lib::db;
use ai_toolbox_storage as storage;
use serde_json::json;

#[test]
fn desktop_reexports_share_storage_types_and_connection() {
    let state: storage::SqliteDbState = db::SqliteDbState::in_memory_for_test().unwrap();
    let desktop_state: &db::sqlite_state::SqliteDbState = &state;
    desktop_state
        .with_conn(|conn| {
            let table: storage::schema::DbTable = db::schema::DbTable::Settings;
            db::helpers::db_put(conn, table, "shared", &json!({"value": 41}))?;
            let value = storage::helpers::db_get(conn, table, "shared")?.unwrap();
            assert_eq!(value["value"], 41);
            assert_eq!(db::SQLITE_DATABASE_FILE, storage::SQLITE_DATABASE_FILE);
            db::health::quick_check(conn)
        })
        .unwrap();
}
