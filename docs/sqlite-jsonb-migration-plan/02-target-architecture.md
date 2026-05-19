# 02 目标架构

## 目录与模块拆分

保留 [tauri/src/db.rs](/root/github/ai-toolbox/tauri/src/db.rs) 作为外部 facade，新增 `tauri/src/db/` 子模块：

```text
tauri/src/db.rs
tauri/src/db/
  helpers.rs
  schema.rs
  migrations.rs
  surreal_import.rs
  backup.rs
  health.rs
  change_hook.rs
```

职责划分：

- `db.rs`：定义 `DbState`、打开数据库、初始化 PRAGMA、对外 re-export helper。
- `helpers.rs`：JSONB CRUD、查询、patch、事务、分页预留。
- `schema.rs`：目标表清单、单例 ID、索引定义、identifier/path 校验。
- `migrations.rs`：SQLite `user_version` 迁移框架。
- `surreal_import.rs`：只在兼容期存在，负责旧 SurrealKV 目录导入。
- `backup.rs`：SQLite backup API、checkpoint、restore 文件替换、VACUUM。
- `health.rs`：`quick_check`、WAL recovery、损坏提示。
- `change_hook.rs`：`update_hook` 到内部变更事件的桥接。

## DbState 形态

目标结构：

```rust
pub struct DbState {
    conn: std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
    db_path: std::path::PathBuf,
}
```

推荐 API：

```rust
impl DbState {
    pub fn with_conn<T>(
        &self,
        operation: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
    ) -> Result<T, String>;

    pub fn with_conn_mut<T>(
        &self,
        operation: impl FnOnce(&mut rusqlite::Connection) -> Result<T, String>,
    ) -> Result<T, String>;

    pub fn db_path(&self) -> &std::path::Path;
}
```

迁移中的注意点：

- 不再暴露 `state.db()` 返回可 clone 的数据库句柄。
- 当前依赖 clone 句柄的调用点要改成显式传 `&DbState` 或闭包式 helper。
- `skills/tool_adapters.rs` 里的 `OnceLock<Surreal<Db>>` 要改掉，不能保存旧连接类型。优先改为调用方显式传 `&DbState`；如果必须缓存，只缓存 runtime location 派生结果，不缓存数据库连接。

## SQLite 打开流程

启动顺序：

1. 解析 `app_data_dir`。
2. 计算旧库路径 `{app_data_dir}/database` 和新库路径 `{app_data_dir}/ai-toolbox.db`。
3. 调用 `surreal_import::ensure_sqlite_database(app_data_dir)`：
   - 处理四阶段状态机。
   - 必要时从 SurrealDB 导入。
   - 新安装则创建空 SQLite 文件。
4. 打开 SQLite connection。
5. 执行 PRAGMA：

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA busy_timeout = 5000;
PRAGMA foreign_keys = ON;
PRAGMA cache_size = -8000;
```

6. 执行 `health::quick_check`。
7. 执行 `migrations::run_all(&mut conn)`。
8. 注册 `DbState` 到 Tauri app。
9. 刷新 runtime location cache、创建托盘、注册事件监听器。

## 数据表骨架

每张业务表使用统一结构：

```sql
CREATE TABLE IF NOT EXISTS {table_name} (
  id TEXT PRIMARY KEY NOT NULL,
  data BLOB NOT NULL CHECK (json_valid(data, 4)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

说明：

- `data` 用 `jsonb(?)` 写入，所以实际是 SQLite JSONB BLOB。
- `CHECK (json_valid(data, 4))` 用 JSONB flag 做快速校验。不要用单参数 `json_valid(data)`，它默认只按严格 JSON 文本判断，可能把合法 JSONB BLOB 判为无效。
- `created_at` / `updated_at` 是表级元数据，helper 返回 `Value` 时会注入到 JSON 对象里，避免旧 adapter 因字段缺失而行为变化。
- 如果 payload 自己包含 `created_at` / `updated_at`，helper 不覆盖 payload 字段；列字段用于排序和兜底。

## JSONB 读写边界

写入：

```sql
INSERT INTO table_name (id, data, created_at, updated_at)
VALUES (?1, jsonb(?2), ?3, ?4)
ON CONFLICT(id) DO UPDATE SET
  data = excluded.data,
  updated_at = excluded.updated_at;
```

读取：

```sql
SELECT id, json(data) AS data_json, created_at, updated_at
FROM table_name
WHERE id = ?1
LIMIT 1;
```

Rust helper 流程：

1. `serde_json::Value` -> JSON text。
2. SQL `jsonb(?json_text)` 写 BLOB。
3. SQL `json(data)` 读 JSON text。
4. JSON text -> `serde_json::Value`。
5. 注入 `id`、`created_at`、`updated_at`。

## Helper API

基础 API：

```rust
pub fn db_get(conn: &Connection, table: DbTable, id: &str) -> Result<Option<Value>, String>;
pub fn db_list(conn: &Connection, table: DbTable, order: Option<OrderSpec>) -> Result<Vec<Value>, String>;
pub fn db_put(conn: &Connection, table: DbTable, id: &str, data: &Value) -> Result<(), String>;
pub fn db_create(conn: &Connection, table: DbTable, data: &Value) -> Result<Value, String>;
pub fn db_delete(conn: &Connection, table: DbTable, id: &str) -> Result<bool, String>;
pub fn db_delete_all(conn: &Connection, table: DbTable) -> Result<usize, String>;
pub fn db_count(conn: &Connection, table: DbTable) -> Result<i64, String>;
```

查询 API：

```rust
pub fn db_query_by_field(
    conn: &Connection,
    table: DbTable,
    field_path: JsonFieldPath,
    expected: &Value,
    order: Option<OrderSpec>,
    limit: Option<usize>,
) -> Result<Vec<Value>, String>;

pub fn db_query_by_bool(
    conn: &Connection,
    table: DbTable,
    field_path: JsonFieldPath,
    expected: bool,
    order: Option<OrderSpec>,
    limit: Option<usize>,
) -> Result<Vec<Value>, String>;

pub fn db_max_i64(
    conn: &Connection,
    table: DbTable,
    field_path: JsonFieldPath,
) -> Result<Option<i64>, String>;
```

patch API：

```rust
pub fn db_patch_fields(
    conn: &Connection,
    table: DbTable,
    id: &str,
    patch: &[(&str, Value)],
) -> Result<Option<Value>, String>;

pub fn db_patch_where_bool(
    conn: &Connection,
    table: DbTable,
    predicate_path: JsonFieldPath,
    predicate_value: bool,
    patch: &[(&str, Value)],
) -> Result<usize, String>;
```

首期建议 patch 用 “读出 JSON -> Rust 修改 -> db_put” 实现，不直接手写复杂 `jsonb_set` 链。数据量小，清晰优先。

事务 API：

```rust
pub fn db_transaction<T>(
    conn: &mut Connection,
    operation: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T, String>,
) -> Result<T, String>;
```

事务内提供 `_tx` 版本 helper，避免在事务内重新拿全局锁。

## 表名和字段路径安全

不要让业务模块传任意字符串拼 SQL。

推荐：

```rust
pub enum DbTable {
    Settings,
    ClaudeProvider,
    CodexProvider,
    // ...
}
```

`DbTable::name()` 返回静态字符串。动态迁移 unknown table 时使用单独的 `ValidatedTableName`，只允许 `[A-Za-z_][A-Za-z0-9_]*`。

字段路径：

- 调用方传 `["settings_config", "env", "GEMINI_API_KEY"]` 或常量 `JsonFieldPath::new("is_applied")`。
- helper 负责生成 `$.settings_config.env.GEMINI_API_KEY`。
- 字段名只允许字母、数字、下划线和必要的已有兼容键；禁止传入原始 SQL 片段。

## 索引策略

v1 建表时只加确定高频索引：

- `is_applied`：provider、prompt、official_account、Oh My OpenAgent/Slim config。
- `sort_index`：provider、prompt、skill、skill_group、mcp_server、Oh My OpenAgent/Slim config、official_account。
- `sort_order`：image_channel、ssh_connection。
- `created_at` / `updated_at`：按时间排序的 history/favorite 表。
- 关键唯一/查找字段：`skill.name`、`mcp_server.name`、`opencode_favorite_plugin.plugin_name`、`opencode_favorite_provider.provider_id`、official_account.provider_id。

表达式示例：

```sql
CREATE INDEX IF NOT EXISTS idx_claude_provider_is_applied
ON claude_provider (json_extract(data, '$.is_applied'));
```

后续新增索引必须递增 `user_version`，不能在业务命令里临时建。

## 备份恢复目标结构

新备份 zip 中新增：

```text
db_manifest.json
db/ai-toolbox.db
external-configs/...
image-studio/assets/...
custom-backup/...
```

`db_manifest.json`：

```json
{
  "engine": "sqlite",
  "schema_version": 1,
  "app_version": "0.9.1",
  "created_at": "2026-05-19T00:00:00+08:00"
}
```

旧备份没有 manifest 时按 SurrealDB 备份处理。

## 数据库健康与压缩

启动健康检查：

```sql
PRAGMA quick_check;
```

压缩：

```sql
PRAGMA wal_checkpoint(TRUNCATE);
VACUUM;
```

不再保留 SurrealKV 的 `safe_compact`。
