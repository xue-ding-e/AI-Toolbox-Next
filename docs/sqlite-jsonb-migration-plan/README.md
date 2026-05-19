# SQLite JSONB 数据库改造执行计划

本目录把用户提供的 SQLite JSONB 改造概要拆成可以直接落地的工程计划。计划基于当前仓库状态，而不是只按概要方案推演。

## 结论

方案方向可以采用：用 SQLite 单文件数据库替代 SurrealDB/SurrealKV，用每张业务表 `id + data + timestamps` 的骨架结构保留 schemaless 开发体验。

但执行时必须修正下面几点：

1. 当前仓库已经依赖 `rusqlite = 0.31`，用于读取 OpenCode runtime 自己的 SQLite 数据库；本次不是单纯“新增 rusqlite”，而是要升级/扩展 feature，并明确区分 AI Toolbox 主数据库和 OpenCode runtime 数据库。
2. 方案里写“43 张 SurrealDB 表”，但按给出的目标清单逐项计数是 41 张常规业务表。代码里还存在 `provider_models` 和 `oh_my_opencode_*` 这类 legacy/迁移期表，迁移阶段要动态发现并处理，但它们不应成为新 schema 的常规目标表。
3. “adapter 几乎不需要修改”只对 `serde_json::Value -> Rust struct` 这一层基本成立；命令层大量使用 `UPDATE ... SET field`、`WHERE field = ...`、`ORDER BY field`、`DELETE FROM ... WHERE ...`，必须通过 SQLite helper 重写。
4. 首个 SQLite 正式版本不能移除 SurrealDB 依赖，否则老用户无法从现有 SurrealKV 目录自动迁移。SurrealDB 依赖应至少保留 1 个可迁移版本，建议保留 2 个发布版本后再删除。
5. `update_hook` 只能作为数据库变更通知的兜底，不应替代现有 `config-changed`、`wsl-sync-request-*`、`skills-changed`、`mcp-changed` 这些语义事件。
6. `Mutex<Connection>` 可行，但 Rust async 命令中必须严格保证：持锁期间不 `.await`，长事务、备份、恢复、迁移放到 `spawn_blocking` 或启动阶段同步流程。

## 文件说明

- `01-technical-review.md`：对概要方案的事实核对、当前仓库约束和必须修正的设计点。
- `02-target-architecture.md`：目标 SQLite JSONB 架构、Rust 模块拆分、helper API、锁和事务规则。
- `03-table-inventory.md`：目标表清单、单例 ID、排序/过滤字段和当前代码入口。
- `04-execution-plan.md`：分阶段执行清单，精确到文件、动作、验收条件和验证命令。
- `05-validation-rollout.md`：自动化测试、手工验证、兼容发布、回滚和后续移除 SurrealDB 的计划。

## 外部依据

执行前需要再次核对依赖版本，但当前计划使用这些稳定事实：

- SQLite 3.45 起支持 JSONB，即把 SQLite 内部 JSON parse tree 以 BLOB 形式存储；应用应把 JSONB 当作 SQLite 私有格式处理，不自行解释 BLOB。参考：https://www.sqlite.org/json1.html
- `PRAGMA user_version` 是 SQLite 数据库文件头里的应用自定义 schema 版本字段，适合作为迁移版本号。参考：https://www.sqlite.org/pragma.html#pragma_user_version
- WAL 模式支持读写并发的基本目标，仍然只允许同一时刻一个 writer。参考：https://www.sqlite.org/wal.html
- rusqlite 的 backup/update hook 能力需要对应 feature，不能只保留当前的 `bundled`。参考：https://docs.rs/rusqlite/latest/rusqlite/

## Definition of Done

首个 SQLite 版本完成时必须同时满足：

1. 新安装用户直接创建 `ai-toolbox.db`，所有功能默认可用。
2. 老用户首次启动自动从 `{app_data_dir}/database` 迁移到 `{app_data_dir}/ai-toolbox.db`。
3. 迁移过程中任意崩溃都不会删除旧 SurrealDB 目录，也不会留下被误判为成功的 SQLite 文件。
4. 本地备份、WebDAV 备份、自动备份和恢复都支持新 SQLite 格式。
5. 新版本可恢复旧 SurrealDB 备份；旧版本遇到新 SQLite 备份时至少不会误恢复成空数据。
6. `pnpm test`、`cd tauri && cargo test`、`pnpm exec tsc --noEmit` 在可用环境中通过，若有既有环境问题必须明确记录。
7. 所有模块级 `AGENTS.md` 中把 “SurrealDB” 作为事实源的描述同步更新为 “AI Toolbox 主数据库”，避免后续开发继续写 SurrealQL。
