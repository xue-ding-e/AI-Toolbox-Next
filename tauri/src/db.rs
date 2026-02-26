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

/// clog 压缩阈值：超过此大小才执行 compact（1MB）
const COMPACT_THRESHOLD: u64 = 1 * 1024 * 1024;

/// 在 SurrealDB 初始化之前，用 SurrealKV 原生 API 执行 compact。
/// 关闭版本历史（enable_versions=false），使 compact 只保留每个 key 的最新版本，
/// 清除所有历史版本和已删除记录，大幅缩减 clog 体积。
pub fn compact_database(db_path: &Path) {
    let clog_size = get_clog_dir_size(db_path);
    if clog_size < COMPACT_THRESHOLD {
        return;
    }

    let size_mb = clog_size as f64 / 1024.0 / 1024.0;
    log::info!("clog 大小 {:.1}MB 超过阈值，开始压缩数据库...", size_mb);

    let mut opts = surrealkv::Options::new();
    opts.dir = db_path.to_path_buf();
    opts.enable_versions = false;

    let store = match surrealkv::Store::new(opts) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("打开 SurrealKV 执行 compact 失败: {}", e);
            return;
        }
    };

    match store.compact() {
        Ok(_) => log::info!("数据库 compact 完成"),
        Err(e) => log::warn!("数据库 compact 失败: {}", e),
    }

    if let Err(e) = store.close() {
        log::warn!("关闭 SurrealKV compact store 失败: {}", e);
    }

    let new_size = get_clog_dir_size(db_path);
    let new_size_mb = new_size as f64 / 1024.0 / 1024.0;
    log::info!(
        "compact 前: {:.1}MB → compact 后: {:.1}MB (节省 {:.1}MB)",
        size_mb,
        new_size_mb,
        size_mb - new_size_mb
    );
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

