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
- 共享连接可以保留一个无反向 management 依赖的 opaque owner lease；所有已有和后续 DB clone 共用它，最后先释放真实 Connection，再释放 lease。lease 不得引用 DB state，避免形成循环。只读验证与首次保留在同一个连接互斥内完成，验证失败不安装；同一 Arc 分配重复保留是幂等操作，不同分配必须拒绝，不能替换原 owner。
- 该保活机制不获取所有权、不执行初始化，也不能追认调用方已经满足 owner-before-open。它是可信 Rust 调用边界，不是恶意进程内代码隔离；with_conn_mut 保留原签名，业务服务仍须在锁内重新核对真实 main 连接，不能只相信缓存路径。伴随文件可由 SQLite 合法创建或移除，不能把主库 inode 的长期身份约束扩大为 WAL/SHM/journal 永久不变。
- 保留 owner 后，with_conn_mut 调用方不得 replace、swap、移出 Connection，或让原连接逃逸共享 holder；真正需要替换时须另行设计 detach/reattach 生命周期。同路径、同 inode 上重新打开连接仍可能通过文件身份检查，这不证明它是原连接，也不保留原事务或 hook；不得用裸指针相等声称解决该身份或 ABA 问题。
- 不用通用 callback/no-op seed 替换原初始化，不把备份、安全检查或错误传播变成调用方可跳过的默认选项。
- v30 新增 Codex context 专用 JSONB 操作账本；fresh 库的 v1 通过 `ALL_TABLES` 建好表，v30 幂等建表是为升级既有 v29 库，不修改已发布迁移。真实文件升级仍须经过原快照与未来版本拒绝路径。
- 账本接口只接受有限长安全标识、固定长度摘要、正整数结果和封闭阶段/错误枚举，不保存原始配置、认证、路径或任意错误文本。它不执行投影或声明回滚，不用其它业务表代替。
- 同 request_id 必须先查指纹再检查 revision 或修改主数据；SavedOnly 的插入与主数据 patch 同事务，重复请求不再次修改主数据。所有账本写接口接收调用方事务并复用现有 helper，不重新开库或嵌套事务。
- 只有新的 Advanced→ProjectionStarted 已成功提交，才允许调用方尝试一次文件写；提交失败禁止写，Replay 不授权重写。重启后遗留 ProjectionStarted 只能解释为 ProjectionUncertain；不能倒退成 SavedOnly 或自动 resume。封闭失败标记只可补记一次，不能清除来隐式重试。
- `live_changed=false` 仍须实际读回后才可从 SavedOnly 进入 Verified；读回失败或不匹配只保留 SavedOnly 并记对应封闭错误，不虚构文件写入。`live_changed=true` 的 SavedOnly 不允许记录读回阶段错误，读取和推进均验证此约束。
- 字符串格式校验不等于能判断其是否含秘密；只有可信业务服务可生成摘要、解析 provider ID 并构造账本输入，不能直接接收和持久化客户端提供的账本对象。

## 最小验证

- 必须显式运行 `cargo test --manifest-path crates/ai-toolbox-storage/Cargo.toml`。原同步 pricing 单测属于此 crate，桌面 cargo test 不会自动运行依赖 crate 的单测。
- 同时保留并运行桌面的完整 `sqlite_jsonb` 集成测试和 `storage_reexports` 身份回归；新 smoke 不能代替旧完整套件。
- 文件库、未来 schema、迁移前快照等边界必须用临时真实文件验证，不能仅用内存库。完整交付仍遵循根 AGENTS.md 的全量验证要求。
