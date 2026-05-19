use std::sync::{Arc, Mutex};

use rusqlite::hooks::Action;
use rusqlite::Connection;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DbChange {
    pub action: DbChangeAction,
    pub database: String,
    pub table: String,
    pub row_id: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DbChangeAction {
    Insert,
    Update,
    Delete,
    Unknown,
}

impl From<Action> for DbChangeAction {
    fn from(action: Action) -> Self {
        match action {
            Action::SQLITE_INSERT => DbChangeAction::Insert,
            Action::SQLITE_UPDATE => DbChangeAction::Update,
            Action::SQLITE_DELETE => DbChangeAction::Delete,
            Action::UNKNOWN => DbChangeAction::Unknown,
            _ => DbChangeAction::Unknown,
        }
    }
}

pub fn install_change_recorder(
    conn: &Connection,
    changes: Arc<Mutex<Vec<DbChange>>>,
) -> Result<(), String> {
    conn.update_hook(Some(
        move |action: Action, database: &str, table: &str, row_id: i64| {
            if let Ok(mut changes) = changes.lock() {
                changes.push(DbChange {
                    action: action.into(),
                    database: database.to_string(),
                    table: table.to_string(),
                    row_id,
                });
            }
        },
    ))
    .map_err(|error| format!("Failed to install SQLite update hook: {error}"))
}
