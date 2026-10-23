# 共享 SQLite 存储模块

## Source of Truth

- 本模块从 AI Toolbox 原桌面存储层抽取，保留原项目许可证、归属、SQL 与历史迁移语义。仓库根 LICENSE 与 NOTICE 继续适用。
- 连接、schema、迁移、JSONB helper、备份、change hook 与同步 pricing seed 只在这里实现。桌面 `db` re-export 相同类型和函数，不复制第二份 SQL。
- 内置定价唯一源仍是 `tauri/resources/model_pricing.json`。此 crate 是 `publish = false` 的仓库内部组件，通过显式相对路径编译期嵌入该文件，不复制默认数据。
- 全局数据库约束见根 AGENTS.md；详细存储行为与桌面远端同步边界见 `tauri/src/db/AGENTS.md`。

## 关键流程与边界

- 保持启动顺序：先只读检查 user_version，再设置 PRAGMA，验证 JSONB，真实文件库按需创建迁移前快照，然后执行迁移、缓存 seed、内置 seed，最后 quick_check。未来 schema 拒绝、快照失败、迁移失败均不得被吞掉。
- 缓存与 bundled seed、桌面远端同步均只用 INSERT OR IGNORE 补缺，不能覆盖用户已有价格。共享 parser、Decimal 校验与缓存写入只有一份；桌面网络请求继续走原 http_client 与代理设置，不把 HTTP 或应用设置引入这里。
- CACHE_DIR 是进程级 OnceLock，当前生命周期仅支持启动时设置一次的单 profile。不要把同一进程内反复调用 set_cache_dir 当作切换 profile；不能据此宣称多 profile 隔离已经实现。
- 该抽取不是进程 owner、daemon、CLI 管理入口或只读访问模式。open 仍可能设置 WAL、迁移并补 seed，不能用于承诺无文件变更的读取命令。WAL/SHM 与跨进程所有权方案须独立设计验证。
- 不用通用 callback/no-op seed 替换原初始化，不把备份、安全检查或错误传播变成调用方可跳过的默认选项。

## 最小验证

- 必须显式运行 `cargo test --manifest-path crates/ai-toolbox-storage/Cargo.toml`。原同步 pricing 单测属于此 crate，桌面 cargo test 不会自动运行依赖 crate 的单测。
- 同时保留并运行桌面的完整 `sqlite_jsonb` 集成测试和 `storage_reexports` 身份回归；新 smoke 不能代替旧完整套件。
- 文件库、未来 schema、迁移前快照等边界必须用临时真实文件验证，不能仅用内存库。完整交付仍遵循根 AGENTS.md 的全量验证要求。
