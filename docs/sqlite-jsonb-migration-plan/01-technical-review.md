# 01 技术方案核对

## 当前仓库事实

核心数据库现在在 [tauri/src/db.rs](/root/github/ai-toolbox/tauri/src/db.rs) 和 [tauri/src/lib.rs](/root/github/ai-toolbox/tauri/src/lib.rs) 中初始化：

- `DbState` 包装的是 `Surreal<surrealdb::engine::local::Db>`。
- 启动路径使用 `{app_data_dir}/database` 作为 SurrealKV 目录。
- 启动时会执行 `db_migration::run_all_db_migrations(&db)`。
- 如果 clog 超过阈值，会执行 `safe_compact`，流程是 export -> 删除数据库目录 -> import。
- 备份恢复目前把整个数据库目录放进 zip 的 `db/` 前缀下。

当前 [tauri/Cargo.toml](/root/github/ai-toolbox/tauri/Cargo.toml) 已经有：

```toml
surrealdb = { version = "2.6.2", features = ["kv-surrealkv"], default-features = false }
rusqlite = { version = "0.31", features = ["bundled"] }
```

`rusqlite` 现有用途主要是读取 OpenCode runtime 的 `opencode.db`，不等于 AI Toolbox 主数据库已经在用 SQLite。

## 方案中成立的部分

1. **SQLite + JSONB 适合当前桌面场景**
   本项目低频写、多数表几十到几百条记录，SQLite 单文件、WAL、backup API 比 SurrealKV 多文件目录更适合备份恢复和发布兼容。

2. **表骨架固定、业务字段放 JSONB 可保留 schemaless 体验**
   字段增删不需要 `ALTER TABLE`，adapter 层继续从 `serde_json::Value` 读取缺省值，这点和现有代码习惯匹配。

3. **分表优于统一 KV**
   当前代码和模块文档都按业务表组织，直接映射到 SQLite 表能降低一次性改造成本，也方便后续对单表加表达式索引。

4. **`PRAGMA user_version` 适合作为 schema 迁移版本号**
   它比现在的 `app_migration` 表更贴近 SQLite 文件级 schema 版本。`app_migration` 可作为历史记录继续迁移，但不应再承担主迁移调度职责。

5. **迁移必须 crash-safe**
   “旧数据先保留，新数据确认后再标记完成”是正确原则。尤其当前 `safe_compact` 会删除目录，这个风险在 SQLite 切换后应完全移除。

## 必须修正的部分

### 0. SQLite/rusqlite 版本要以实际 bundled 结果为准

概要指定 “SQLite 3.51.3 + rusqlite 0.39.0”。执行时不能只改 Cargo 版本号后假设 bundled SQLite 就是这个版本。

必须在阶段 1 加版本探针：

```sql
SELECT sqlite_version(), typeof(jsonb('{}')), json_valid(jsonb('{}'), 4);
```

验收条件是：

- `jsonb('{}')` 可用。
- `typeof(jsonb('{}')) = 'blob'`。
- `json_valid(jsonb('{}'), 4) = 1`。

如果 rusqlite/libsqlite3-sys 实际 bundled SQLite 版本低于 JSONB 需求，先解决依赖版本，再进入业务迁移。

### 1. 表数量不一致

概要写“按当前 43 张 SurrealDB 表一一对应”，但清单逐项计数是 41 张常规目标表。当前代码还涉及：

- `provider_models`：启动时已作为 legacy 表删除，不应进入目标 schema。
- `oh_my_opencode_config` / `oh_my_opencode_global_config`：历史表，当前 SurrealDB 迁移会重命名到 `oh_my_openagent_*` 后删除。
- OpenCode runtime 的 `session` / `message` / `part` / `session_share`：这是 OpenCode 自己的 SQLite 数据库，不属于 AI Toolbox 主数据库。

执行计划采用 41 张目标表，并在 SurrealDB -> SQLite 导入时动态发现 legacy/未知表。

### 2. adapter 无感知不等于命令层无感知

现有 adapter 多数确实是 `Value` 转 Rust struct，但命令层直接依赖 SurrealQL 语义：

- `CREATE table CONTENT $data` 后再 `ORDER BY created_at DESC LIMIT 1` 找新记录。
- `UPDATE table SET is_applied = false WHERE is_applied = true`。
- `UPDATE record SET sort_index = $index`。
- `DELETE FROM table WHERE plugin_name = $name`。
- `SELECT count() FROM table GROUP ALL`。
- `SELECT *, type::string(id) as id FROM table WHERE id INSIDE $asset_ids`。

SQLite JSONB 下不能简单把 SQL 字符串替换掉。必须先建立 helper，再逐模块把这些查询替换为 helper 语义。

### 3. JSONB helper 要读写 JSON 文本边界

Rust 侧继续使用 `serde_json::Value`。写入 SQLite 时建议统一走：

- `serde_json::to_string(&value)` 生成 JSON text。
- SQL 层用 `jsonb(?1)` 写入 `data` BLOB。

读取时建议统一走：

- SQL 层用 `json(data)` 还原 JSON text。
- Rust 侧 `serde_json::from_str::<Value>()`。
- helper 注入 `id`，必要时注入 `created_at` / `updated_at`，保持现有 adapter 输入形态。

不要让业务模块直接拿 JSONB BLOB。

### 4. `Mutex<Connection>` 要配合 async 约束

`rusqlite::Connection` 是同步 API。当前 Tauri 命令大量是 `async fn`，迁移后不能在持有 `MutexGuard<Connection>` 时执行 `.await`，否则容易把数据库锁和异步运行时任务交织起来。

建议 `DbState` 提供闭包式 API：

```rust
state.with_conn(|conn| {
    db_get(conn, DbTable::Settings, "app")
})
```

长操作使用：

- 启动阶段 SurrealDB -> SQLite 导入：启动时阻塞流程或 `spawn_blocking`。
- 备份/恢复/VACUUM：`spawn_blocking`。
- 普通命令：短闭包内完成读写，闭包结束立即释放锁，再做托盘刷新、文件写入、网络请求或事件发送。

### 5. update hook 不能替代业务事件

`update_hook` 适合告诉自动备份调度器“某张表发生写入”。它不适合替代现有业务事件，因为现有事件包含业务语义：

- `config-changed`：刷新托盘。
- `wsl-sync-request-*`：触发某个工具的 WSL 同步。
- `skills-changed`：触发 Skills WSL/SSH 同步。
- `mcp-changed`：触发 MCP 同步。

这些事件仍应由命令层在完成 DB 写入和运行时文件写入后显式发出。

### 6. SurrealDB 依赖不能过早删除

如果首个 SQLite 版本移除 `surrealdb`，老用户的 `{app_data_dir}/database` 无法被自动读取。合理节奏是：

1. 第一个 SQLite 版本：同时保留 `surrealdb` 和 `rusqlite`，只用 SurrealDB 读取旧库并导入 SQLite。
2. 后续至少一个版本：继续保留导入器，处理跳版本升级用户。
3. 再后续版本：如果 telemetry/issue 反馈稳定，再移除 SurrealDB 依赖和导入器。

### 7. 备份格式必须版本化

新备份包需要 `db_manifest.json`。旧备份包没有 manifest 时默认按 SurrealDB 目录恢复，再走迁移。新应用恢复旧备份是硬要求；旧应用恢复新备份无法完全兼容，但必须让新备份结构避免被旧逻辑误识别成空数据库。

## 设计取舍

采用 KISS 原则，首期不做通用 ORM，也不做复杂查询生成器。

推荐边界：

- 通用 helper 只负责 CRUD、排序、布尔/字段过滤、批量 patch、事务。
- 模块 adapter 继续负责业务字段兼容和默认值。
- 模块 commands 负责业务动作、运行时文件写入和语义事件。
- 表名用常量/白名单，不允许任意字符串进入 SQL identifier。
- JSON path 只允许由字段名片段组成，例如 `settings_config.env.GEMINI_API_KEY` 转成 `$.settings_config.env.GEMINI_API_KEY`，不接受调用方传入原始 `$...` path。
