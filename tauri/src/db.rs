use std::sync::Arc;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use tokio::sync::Mutex;

/// Database state wrapper
pub struct DbState(pub Arc<Mutex<Surreal<Db>>>);
