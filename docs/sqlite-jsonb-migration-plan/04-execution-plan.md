# 04 分阶段执行计划

## 阶段 0：准备与冻结口径

目标：把改造边界固定下来，避免边做边扩大范围。

任务：

1. 在当前分支确认无未提交业务改动：
   - 命令：`git status --short`
   - 验收：只有本轮文档或为空。
2. 冻结目标表清单：
   - 以 `03-table-inventory.md` 的 41 张表为 v1 schema。
   - 记录 legacy 表策略。
3. 冻结发布策略：
   - 首个 SQLite 版本保留 SurrealDB 依赖。
   - 至少 1 个后续版本继续保留 SurrealDB 导入器。
4. 冻结测试闸门：
   - `pnpm test`
   - `cd tauri && cargo test`
   - `pnpm exec tsc --noEmit`
   - 涉及构建入口时补 `pnpm build`

验收：

- 本目录文档已合并或被用户确认。
- 后续实现不得把 SurrealDB 依赖在首个 SQLite 版本直接删除。

## 阶段 1：SQLite 基础设施

目标：不改业务模块，先让 SQLite 主数据库基础能力可测。

文件：

- `tauri/Cargo.toml`
- `tauri/src/db.rs`
- `tauri/src/db/helpers.rs`
- `tauri/src/db/schema.rs`
- `tauri/src/db/migrations.rs`
- `tauri/src/db/health.rs`
- `tauri/src/db/change_hook.rs`
- `tauri/tests/db_sqlite_jsonb.rs`

任务：

1. 升级/扩展 rusqlite feature。
   - 建议先评估 `rusqlite = "0.39"` 是否与当前 Rust/MSRV/tauri 构建兼容。
   - feature 至少需要：`bundled`, `backup`, `hooks`。
   - 如果使用 `modern_sqlite` 或当前 rusqlite 默认 bundled SQLite 不满足 JSONB，要加版本探针测试。
   - 版本探针至少执行：`SELECT sqlite_version(), typeof(jsonb('{}')), json_valid(jsonb('{}'), 4)`。
2. 实现 `DbState`：
   - `Arc<Mutex<Connection>>`
   - `db_path`
   - `with_conn`
   - `with_conn_mut`
3. 实现 PRAGMA 初始化：
   - WAL
   - synchronous NORMAL
   - busy_timeout 5000
   - foreign_keys ON
   - cache_size -8000
4. 实现 `schema.rs`：
   - `DbTable` enum。
   - 41 张目标表。
   - 单例 ID 常量。
   - initial index 列表。
   - table name / JSON path 校验。
5. 实现 `helpers.rs`：
   - `db_get`
   - `db_list`
   - `db_put`
   - `db_create`
   - `db_delete`
   - `db_delete_all`
   - `db_count`
   - `db_query_by_field`
   - `db_query_by_bool`
   - `db_max_i64`
   - `db_patch_fields`
   - `db_patch_where_bool`
   - `db_transaction`
6. 实现 `migrations.rs`：
   - `const TARGET_SCHEMA_VERSION: i32 = 1`
   - `get_user_version`
   - `set_user_version`
   - v0 -> v1 建 41 张表和初始索引。
   - version > target 时返回明确错误。
7. 实现 `health.rs`：
   - `PRAGMA quick_check`
   - 失败时 checkpoint 重试。
8. 实现 `change_hook.rs`：
   - 记录 table name。
   - 暂时只打 log 或发内部 channel，不驱动业务事件。

验收：

- 单元测试能验证 JSONB 写入后 `typeof(data) = 'blob'`。
- `db_get/db_list` 返回的 `Value` 必须包含干净 `id`。
- `created_at/updated_at` 注入逻辑稳定。
- 非法 table/path 被拒绝。
- 事务失败会回滚。

验证：

- `cd tauri && cargo test db_sqlite_jsonb`
- `cd tauri && cargo check`

## 阶段 2：SurrealDB -> SQLite 一次性导入

目标：老用户首次启动可自动迁移，且 crash-safe。

文件：

- `tauri/src/db/surreal_import.rs`
- `tauri/src/db.rs`
- `tauri/src/lib.rs`
- `tauri/src/db_migration/mod.rs`
- `tauri/tests/db_surreal_import.rs`

任务：

1. 定义路径：
   - 旧库：`{app_data_dir}/database`
   - 新库：`{app_data_dir}/ai-toolbox.db`
   - 完成标记：`{app_data_dir}/sqlite-migration-complete.flag`
   - 日志：`{app_data_dir}/migration.log`
   - warning：`{app_data_dir}/migration_warnings.log`
   - 旧库压缩包：`{app_data_dir}/database.migrated.zip`
2. 实现四阶段状态机：
   - A：只有旧库，无新库：执行迁移。
   - B：旧库和新库都有，无完成标记：删除新库及 `-wal/-shm` 后重试。
   - C：旧库和新库都有，有完成标记：压缩旧库，成功后删除旧目录。
   - D：只有新库：正常启动。
3. 旧库导入前先执行当前 SurrealDB migration：
   - `db_migration::run_all_db_migrations(&surreal_db).await`
   - 这样 legacy `oh_my_opencode_*` 先收敛到 `oh_my_openagent_*`。
4. 动态发现 SurrealDB 实际表：
   - 首选 `INFO FOR DB` 或等效方式。
   - 兜底用目标表清单逐表 `count_records`。
5. 全量导入：
   - SQLite 侧一个事务覆盖整次导入。
   - 每条 Surreal record 去掉 SurrealDB record wrapper，保留干净 id。
   - `INSERT OR REPLACE` 到同名 SQLite 表。
6. 计数校验：
   - 每张表 Surreal count == SQLite count。
   - unknown table 有数据必须迁移或失败。
7. 抽样校验：
   - 每张非空表取前 3 条和后 3 条。
   - 对比 id、JSON 字段数、关键字段 `name`/`provider_id`/`created_at`。
8. 完成标记：
   - 只有导入事务提交、计数校验、抽样校验都成功后才写 flag。
9. 压缩旧库：
   - flag 存在后压缩旧目录为 `database.migrated.zip`。
   - 压缩成功后删除旧目录。
   - 删除失败不影响使用，但写 warning。
10. 连续失败计数：
   - 记录到 `{app_data_dir}/sqlite-migration-failures.json`。
   - 连续 3 次失败后启动时返回可展示错误，包含 `migration.log` 路径。

验收：

- 任意一步失败不写完成标记。
- 旧 SurrealDB 目录在完成前绝不删除。
- 删除新 SQLite 文件时同时删除 `ai-toolbox.db-wal` 和 `ai-toolbox.db-shm`。
- migration log 不依赖 stdout。

验证：

- 新建一个临时 SurrealDB 测试库，写入 settings/provider/skill/image 数据，运行导入器。
- 模拟 B 状态，确认会重试。
- 模拟 C 状态，确认只压缩旧目录。
- `cd tauri && cargo test db_surreal_import`

## 阶段 3：启动链路切换

目标：应用启动注册 SQLite `DbState`，但业务模块还可以分批改造。

文件：

- `tauri/src/lib.rs`
- `tauri/src/db.rs`
- `tauri/src/http_client.rs`
- `tauri/src/update.rs`

任务：

1. 替换 `Surreal::new::<SurrealKv>` 初始化为 `db::open_or_migrate(app_data_dir)`。
2. 删除启动阶段 `safe_compact` 调用。
3. 启动后刷新 runtime location cache 的签名改为 `&DbState`。
4. `http_client.rs` 中读取 settings 的逻辑改为 SQLite helper。
5. `update.rs` 中读取 proxy settings 的逻辑改为 SQLite helper。
6. 确认窗口关闭事件里读取 `minimize_to_tray_on_close` 不再 `block_on` Surreal query，改为同步 helper 读 settings。

验收：

- 新安装启动能创建空 SQLite schema。
- 没有业务模块改造时，编译会暴露旧类型调用点；这些调用点进入后续阶段逐个消除。

验证：

- `cd tauri && cargo check`

## 阶段 4：Settings 试点

目标：用最小业务模块验证完整 “读 -> adapter -> 写 -> side effect”。

文件：

- `tauri/src/settings/commands.rs`
- `tauri/src/settings/adapter.rs`
- `tauri/src/settings/types.rs`
- `tauri/src/settings/backup/auto_backup.rs`
- `tauri/src/settings/backup/utils.rs`
- `web/test/features/settings/...` 如需新增前端测试

任务：

1. `get_settings`：
   - `db_get(Settings, "app")`
   - 缺失返回 `AppSettings::default()`。
2. `save_settings`：
   - `db_put(Settings, "app", adapter::to_db_value(&settings))`
   - 释放 DB 锁后再刷新托盘。
3. 自动备份读取 settings 改 helper。
4. `last_auto_backup_time` 更新改 `db_patch_fields`。
5. 保留“开机自启偏好先落库，系统副作用失败不阻止偏好保存”的既有约束。

验收：

- settings 读写往返后字段不丢。
- optional 字段显式清空不会留下旧值。
- 保存后托盘刷新仍发生。

验证：

- `cd tauri && cargo test settings`
- `pnpm test`

## 阶段 5：核心 Provider/Prompt 模块

目标：迁移 Claude Code、Codex、Gemini CLI、OpenCode、OpenClaw 的数据库操作。

文件：

- `tauri/src/coding/claude_code/commands.rs`
- `tauri/src/coding/claude_code/tray_support.rs`
- `tauri/src/coding/codex/commands.rs`
- `tauri/src/coding/codex/official_accounts.rs`
- `tauri/src/coding/codex/plugin_workspace.rs`
- `tauri/src/coding/codex/tray_support.rs`
- `tauri/src/coding/gemini_cli/commands.rs`
- `tauri/src/coding/gemini_cli/official_accounts.rs`
- `tauri/src/coding/gemini_cli/tray_support.rs`
- `tauri/src/coding/open_code/commands.rs`
- `tauri/src/coding/open_claw/commands.rs`
- `tauri/src/coding/runtime_location.rs`
- `tauri/src/coding/proxy_gateway/runtime/providers.rs`

任务：

1. 所有 `CREATE table CONTENT $data` 改 `db_create` 或 `db_put`。
2. 所有 `SELECT ..., type::string(id) as id` 改 `db_get/db_list/db_query_by_*`。
3. 所有 `WHERE is_applied = true` 改 `db_query_by_bool(..., "is_applied", true)`。
4. 所有 “取消旧 applied + 设置新 applied” 改成单事务。
5. 所有 sort 更新改成事务内批量 `db_patch_fields`。
6. Codex/Gemini official account 的 `provider_id` 查询和 count 改 helper。
7. OpenCode favorite plugin/provider 的 `plugin_name`、`provider_id` 查询改 helper。
8. runtime_location 全部改为通过 `DbState` helper 读取 common config。
9. tray support 读取 provider 列表改 helper。

验收：

- provider 保存、编辑、删除、应用、禁用、排序都可用。
- prompt 保存、删除、应用、排序仍触发运行时文件写入和事件。
- Codex/Gemini official account 与 provider 绑定关系不丢。
- OpenCode favorite plugin/provider 去重逻辑不变。
- OpenClaw common config 清空语义不变。

验证：

- `cd tauri && cargo test claude`
- `cd tauri && cargo test codex`
- `cd tauri && cargo test gemini`
- `cd tauri && cargo test runtime_location`
- `cd tauri && cargo check`

## 阶段 6：Skills / MCP / Custom Tools

目标：迁移管理数据，并保持中央仓库和运行时同步语义。

文件：

- `tauri/src/coding/skills/skill_store.rs`
- `tauri/src/coding/skills/central_repo.rs`
- `tauri/src/coding/skills/cache_cleanup.rs`
- `tauri/src/coding/skills/tool_adapters.rs`
- `tauri/src/coding/skills/commands.rs`
- `tauri/src/coding/skills/installer.rs`
- `tauri/src/coding/skills/tray_support.rs`
- `tauri/src/coding/mcp/mcp_store.rs`
- `tauri/src/coding/mcp/config_sync.rs`
- `tauri/src/coding/tools/custom_store.rs`
- `tauri/src/coding/tools/detection.rs`
- `tauri/src/coding/tools/claude_plugins.rs`

任务：

1. `skill_store.rs` 全部改 helper。
2. group 删除时 `UPDATE skill SET group_id = NONE` 改事务内逐条 patch。
3. Inventory apply、禁用/恢复、sync_details 更新保持原语义。
4. `skill_settings` / `skill_preferences` 单例改 helper。
5. `tool_adapters.rs` 移除 `OnceLock<Surreal<Db>>`。
6. `custom_tool` 按 key 保存/读取，保持 display_name 排序。
7. `mcp_server` CRUD、排序、enabled_tools、sync_details 改 helper。
8. `mcp_preferences` / `favorite_mcp` 改 helper。
9. 保持 `skills-changed` / `mcp-changed` 事件由命令层显式发出。

验收：

- Skills 中央仓库仍是唯一 source of truth。
- 禁用 skill 后数据库 desired state 收敛，工具目录清理仍 best-effort。
- MCP 导入、同步、排序和 favorite 不变。
- 自定义工具能被 Skills/MCP 共享读取。

验证：

- `cd tauri && cargo test skills`
- `cd tauri && cargo test mcp`
- `cd tauri && cargo test tools`

## 阶段 7：WSL / SSH 同步配置

目标：迁移同步配置表，保持事件驱动同步和 session 恢复语义。

文件：

- `tauri/src/coding/wsl/commands.rs`
- `tauri/src/coding/wsl/mcp_sync.rs`
- `tauri/src/coding/wsl/skills_sync.rs`
- `tauri/src/coding/ssh/commands.rs`
- `tauri/src/coding/ssh/mcp_sync.rs`
- `tauri/src/coding/ssh/skills_sync.rs`
- `tauri/src/coding/ssh/adapter.rs`
- `tauri/src/coding/wsl/adapter.rs`

任务：

1. `wsl_sync_config` 单例和 `defaults_version` 固定记录改 helper。
2. `wsl_file_mapping` 保存、删除、清空、排序读取改 helper。
3. `ssh_sync_config` 单例和 `defaults_version` 固定记录改 helper。
4. `ssh_connection` 按 `sort_order, name` 读取。
5. `ssh_file_mapping` 保存、删除、清空、排序读取改 helper。
6. `active_connection_id` 清空/设置改 patch。
7. `last_sync_*` 更新改 patch。
8. SSH 冷启动 session restore 改为从 `DbState` 读取，不假设 session 会自动从 DB 补回。

验收：

- WSL 自动同步仍只由事件监听器决定，不因 DB 写入 hook 自动触发。
- SSH 首次手动同步仍能按 saved active connection 恢复 session。
- directory_excludes optional 字段保真。

验证：

- `cd tauri && cargo test wsl`
- `cd tauri && cargo test ssh`
- Windows 环境手工验证 WSL Direct 路径和普通 WSL 同步。

## 阶段 8：Oh My OpenAgent / Oh My OpenCode Slim

目标：迁移两个 OpenCode 旁挂配置模块，并保留 `__local__` 桥接语义。

文件：

- `tauri/src/coding/oh_my_openagent/commands.rs`
- `tauri/src/coding/oh_my_openagent/adapter.rs`
- `tauri/src/coding/oh_my_openagent/tray_support.rs`
- `tauri/src/coding/oh_my_opencode_slim/commands.rs`
- `tauri/src/coding/oh_my_opencode_slim/adapter.rs`
- `tauri/src/coding/oh_my_opencode_slim/tray_support.rs`

任务：

1. config 列表、创建、更新、删除改 helper。
2. global config 单例 `global` 改 helper。
3. apply config 的 “取消旧 applied + 设置新 applied” 改事务。
4. sort_index 更新改 helper。
5. 禁用状态 patch 改 helper。
6. 保留数据库为空时从本地文件生成 `__local__` 临时记录。
7. 不恢复写入历史 `agents.*.fallback_models`。

验收：

- `__local__` 不被当成真实 ID。
- 清除已应用配置只删除运行时文件并取消 applied，不删 profile。
- Slim 顶层 `fallback.chains` 写入规则不变。

验证：

- `cd tauri && cargo test oh_my_openagent`
- `cd tauri && cargo test oh_my_opencode_slim`

## 阶段 9：Proxy Gateway / Image

目标：迁移剩余业务表，同时保护高频文件状态不进数据库。

文件：

- `tauri/src/coding/proxy_gateway/settings.rs`
- `tauri/src/coding/proxy_gateway/commands.rs`
- `tauri/src/coding/proxy_gateway/runtime.rs`
- `tauri/src/coding/proxy_gateway/runtime/upstream.rs`
- `tauri/src/coding/proxy_gateway/cli_proxy/mod.rs`
- `tauri/src/coding/image/store.rs`
- `tauri/src/coding/image/commands.rs`

任务：

1. `proxy_gateway_settings` 单例 `gateway` 改 helper。
2. runtime/upstream 中 provider 查找改走新 provider helper。
3. 保持 CLI manifest、request log、metrics、model health 继续使用文件状态。
4. `image_channel` CRUD、sort_order 查询改 helper。
5. `image_job` 创建、列表分页/limit、删除改 helper。
6. `image_asset` 创建、批量读取、缺失容忍、删除改 helper。
7. `image_asset` 批量读取必须按输入 asset_ids 顺序重排结果。

验收：

- 网关运行中保存设置仍同步更新运行态共享 settings。
- 请求日志/metrics/model health 不写主数据库。
- 图片任务成功时 DB 元数据和 `image-studio/assets/` 文件同时存在。
- 缺失 asset 不导致任务 DTO 整体失败。

验证：

- `cd tauri && cargo test proxy_gateway`
- `cd tauri && cargo test image`

## 阶段 10：备份、恢复、WebDAV、自动备份

目标：把数据库目录备份替换成 SQLite 单文件备份，并支持旧备份恢复。

文件：

- `tauri/src/settings/backup/local.rs`
- `tauri/src/settings/backup/webdav.rs`
- `tauri/src/settings/backup/auto_backup.rs`
- `tauri/src/settings/backup/utils.rs`
- `tauri/src/settings/backup/AGENTS.md`
- `web/features/settings/...` 如需展示错误

任务：

1. 新增 `db_manifest.json` 结构体。
2. backup 创建：
   - 对 SQLite 执行 checkpoint 或 backup API。
   - zip 写入 `db/ai-toolbox.db`。
   - zip 写入 manifest。
   - 保持 external-configs、image assets、custom-backup 原路径。
3. 本地 restore：
   - 读取 manifest。
   - `engine=sqlite` 且 schema_version <= target：替换 db 文件，删除 `-wal/-shm`。
   - `engine=sqlite` 且 schema_version > target：拒绝，提示升级应用。
   - `engine=surrealdb` 或无 manifest：解压旧 db 到临时目录，走 SurrealDB -> SQLite 导入。
4. WebDAV backup：
   - 复用新 `create_backup_zip`。
   - 上传文件命名规则保持兼容。
5. WebDAV restore：
   - 下载后走同一 restore parser。
6. restore 后：
   - 写 `.resync_required`。
   - 重启/重开 DB 后刷新 runtime location cache。
   - 触发 Skills/MCP resync 既有流程。
7. `get_database_path`：
   - 更新为返回 SQLite db 文件路径或显示 app data database 状态。

验收：

- 新备份包含 manifest 和 `db/ai-toolbox.db`。
- 旧备份无 manifest 时可恢复并迁移。
- restore 后 external-configs 和 DB 仍一致。
- WebDAV 和本地备份行为一致。

验证：

- `cd tauri && cargo test backup`
- 手工做：SQLite 备份 -> 删除本地库 -> 恢复。
- 手工做：旧 SurrealDB 备份 -> 新应用恢复。
- 手工做：WebDAV 上传、列表、下载、恢复。

## 阶段 11：文档与模块 AGENTS 更新

目标：消除“SurrealDB 是当前事实源”的过期开发指引。

文件：

- 根 `AGENTS.md`
- 所有关联模块 `AGENTS.md`
- `docs/module-agents-template.md`

任务：

1. 根文档把数据库全局规则改为 SQLite JSONB 主数据库。
2. 各模块把 “SurrealDB” 改为 “AI Toolbox 主数据库” 或 “SQLite 主数据库”。
3. 保留特殊事实：
   - Image 元数据在主数据库，资产文件在 `image-studio/assets/`。
   - Proxy Gateway 高频日志和健康状态仍不进数据库。
   - Session Manager 的会话事实源不是主数据库。
   - OpenCode runtime SQLite 不是 AI Toolbox 主数据库。
4. 更新最小验证命令。

验收：

- `grep -R "SurrealDB" AGENTS.md tauri/src web/features docs` 只剩历史迁移/兼容期说明。

## 阶段 12：全量验证与首个 SQLite 版本发布

目标：发布一个可迁移版本。

任务：

1. 全量自动化：
   - `pnpm test`
   - `cd tauri && cargo test`
   - `pnpm exec tsc --noEmit`
   - 需要时 `pnpm build`
2. 手工核心路径：
   - 新安装启动。
   - 带旧 SurrealDB 数据启动并自动迁移。
   - provider/prompt/settings/skills/mcp/wsl/ssh/gateway/image 操作。
   - 本地备份恢复。
   - WebDAV 备份恢复。
3. 构建验证：
   - macOS/Windows/Linux release workflow。
   - 确认 release workflow 不依赖 Rust target cache。
4. 发布说明：
   - 明确这是数据库引擎切换版本。
   - 明确首次启动会自动迁移并保留旧数据库压缩包。
   - 明确遇到问题时提供 `migration.log`。

验收：

- 首个 SQLite 版本仍包含 SurrealDB 导入器。
- 用户无需手动迁移。

## 阶段 13：后续移除 SurrealDB 依赖

目标：在确认升级用户基本完成后移除兼容代码。

触发条件：

- 至少 1 个后续版本继续保留导入器。
- 没有集中出现迁移失败 issue。
- release notes 已提前告知旧版本跳升路径。

任务：

1. 删除 `surrealdb` 依赖。
2. 删除 `db/surreal_import.rs` 中的读取旧库逻辑，只保留“检测到旧库时提示先升级到过渡版本”的错误。
3. 删除 `tauri/src/db_migration/` 旧 SurrealDB migration 运行逻辑，或标记为 legacy-only。
4. 删除所有 `surrealdb::` import。
5. 再跑全量测试和构建。

验收：

- `grep -R "surrealdb" tauri/src tauri/Cargo.toml` 无运行时代码引用。
- 新版本仍能打开已迁移 SQLite DB。
