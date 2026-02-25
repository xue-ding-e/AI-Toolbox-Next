use std::path::Path;
use std::sync::Arc;
use surrealdb::Surreal;
use tokio::sync::Mutex;

pub struct DbState(pub Arc<Mutex<Surreal<surrealdb::engine::local::Db>>>);

/// Run database migrations
///
/// Note: With the adapter layer pattern, database migrations are no longer needed.
/// The adapter handles all backward compatibility automatically.
pub async fn run_migrations(_db: &Surreal<surrealdb::engine::local::Db>) -> Result<(), String> {
    // No migrations needed - adapter layer handles all compatibility
    Ok(())
}

/// 获取数据库 clog 目录的总大小（字节）
pub fn get_clog_dir_size(db_path: &Path) -> u64 {
    let clog_dir = db_path.join("clog");
    if !clog_dir.exists() {
        return 0;
    }
    get_dir_size(&clog_dir)
}

/// 递归计算目录大小
fn get_dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                total += get_dir_size(&entry_path);
            } else if let Ok(metadata) = entry.metadata() {
                total += metadata.len();
            }
        }
    }
    total
}

