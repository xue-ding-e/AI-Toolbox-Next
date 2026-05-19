# 05 验证、发布与回滚

## 自动化测试矩阵

### SQLite helper

必须覆盖：

- `db_put` + `db_get` 往返对象。
- `db_create` 自动生成无连字符 UUID。
- `db_list` 注入 id，并按 sort 字段排序。
- `db_query_by_bool` 查询 `is_applied`。
- `db_query_by_field` 查询字符串、数字、null。
- `db_patch_fields` 保留未知字段。
- `db_patch_where_bool` 批量取消 applied。
- transaction 成功 commit、失败 rollback。
- 非法表名/字段路径拒绝。
- `created_at` 首次写入后不被 update 改掉，`updated_at` 会更新。
- `json(data)` 解析失败时报错，不返回空对象假成功。

### SQLite migration

必须覆盖：

- 全新数据库 user_version 从 0 升到 target。
- user_version 等于 target 时 no-op。
- user_version 大于 target 时拒绝启动。
- v0 -> v1 建 41 张表和索引。
- 单个 migration 失败后不更新 user_version。
- 迁移前 backup 文件生成。

### SurrealDB -> SQLite 导入

必须覆盖：

- 空旧库迁移。
- settings/provider/skill/mcp/image 混合数据迁移。
- legacy `oh_my_opencode_*` 先经现有 SurrealDB migration 收敛。
- unknown 空表只写 warning。
- unknown 非空表可导入；导入失败整体失败。
- 计数不一致失败。
- 中断状态 B 删除不完整 SQLite 后重试。
- 状态 C 只压缩旧目录。
- 连续失败 3 次记录错误。

### 业务模块

每个域至少覆盖一条 “写入 -> 再读取 -> adapter 转 DTO”：

- settings：保存后再读，optional 清空。
- Claude/Codex/Gemini provider：新增、应用、禁用、排序。
- prompt：新增、应用、删除 applied prompt 后运行时文件处理。
- OpenCode/OpenClaw common config：设置路径、清空路径。
- Codex/Gemini official account：provider_id 过滤、应用切换。
- Skills：导入、分组、禁用、恢复、sync_details。
- MCP：导入、排序、favorite、enabled_tools。
- WSL/SSH：保存 config、mapping、last_sync 字段。
- Proxy Gateway：保存 settings，运行态共享 settings 更新。
- Image：创建 channel、job、asset，批量 asset 缺失容忍。
- Backup：SQLite 备份/恢复、旧备份恢复。

## 手工验证清单

### 新安装

1. 删除 app data。
2. 启动应用。
3. 确认生成 `ai-toolbox.db`。
4. 新建一个 Claude provider。
5. 保存 settings。
6. 退出重启后数据仍在。

### 老库自动迁移

1. 准备一份包含以下数据的旧 SurrealDB app data：
   - settings
   - Claude/Codex/Gemini provider 和 prompt
   - Skills 分组和 skill
   - MCP server
   - WSL/SSH config 和 mappings
   - Proxy Gateway settings
   - Image channel/job/asset
2. 启动新应用。
3. 确认：
   - `ai-toolbox.db` 存在。
   - `migration.log` 记录成功。
   - `database.migrated.zip` 存在。
   - 旧 `database` 目录已删除或保留 warning。
4. 逐页打开并核对数据。

### 迁移中断

1. 人工制造 B 状态：
   - `database/` 存在。
   - `ai-toolbox.db` 存在。
   - 无 complete flag。
2. 启动应用。
3. 确认删除不完整 SQLite 并重新迁移。
4. 确认旧库未删除。

### 备份恢复

1. 新 SQLite 备份：
   - 创建本地备份。
   - 检查 zip 内有 `db_manifest.json` 和 `db/ai-toolbox.db`。
   - 删除本地 DB 后恢复。
2. 旧 SurrealDB 备份：
   - 用新应用恢复旧备份 zip。
   - 确认走导入流程。
3. WebDAV：
   - 测试连接。
   - 上传备份。
   - 列表显示。
   - 下载恢复。

### 运行时联动

1. 保存 provider 后托盘菜单更新。
2. 保存 prompt 后运行时文件更新。
3. Windows 上 WSL 自动同步仍由事件触发。
4. SSH active connection 冷启动后首次手动同步可用。
5. Skills/MCP restore 后 `.resync_required` 触发 resync。

## 性能与体积验证

性能基线：

- 记录迁移 100、1000、10000 条 image_job 的耗时。
- 记录 list provider、list skill、list image_job limit 100 的耗时。
- 记录 SQLite 文件大小、SurrealDB 旧目录大小、`database.migrated.zip` 大小。

体积验证：

- 首个兼容版本因为同时依赖 SurrealDB 和 SQLite，体积可能临时增加，不能按最终体积判断。
- 移除 SurrealDB 的后续版本再记录最终体积变化。

## 发布策略

### 第一个 SQLite 版本

行为：

- 新安装使用 SQLite。
- 老用户自动迁移。
- 旧 SurrealDB 目录压缩保留。
- SurrealDB 依赖仍在。

Release notes 必须说明：

- 数据库引擎切换为 SQLite。
- 首次启动会自动迁移。
- 旧数据会保留为 `database.migrated.zip`。
- 如果启动失败，请提供 `migration.log`。

### 第二个兼容版本

行为：

- 继续保留 SurrealDB 导入器。
- 修复首个版本暴露出的迁移边界问题。
- 继续允许跳版本用户从旧库迁移。

### 移除 SurrealDB 的版本

前置条件：

- 至少一个兼容版本稳定发布。
- 没有大量迁移失败 issue。
- 用户文档已说明旧版本跳升建议。

行为：

- 删除 SurrealDB 依赖和导入器。
- 如果检测到旧 `database/` 目录但无 SQLite DB，提示用户先安装过渡版本完成迁移。

## 回滚策略

### 用户本地回滚

迁移成功后保留：

```text
{app_data_dir}/database.migrated.zip
```

用户如需回退到旧应用：

1. 退出新应用。
2. 解压 `database.migrated.zip` 为 `{app_data_dir}/database`。
3. 使用旧版本应用。

注意：新版本迁移后的新增数据不会自动反向同步回 SurrealDB。

### 应用内失败处理

迁移失败：

- 不写 complete flag。
- 删除不完整 SQLite 文件和 wal/shm。
- 保留旧 SurrealDB 目录。
- 下次启动重试。
- 连续 3 次失败后提示 `migration.log` 路径。

SQLite schema migration 失败：

- 单步 savepoint/transaction rollback。
- 不提升 user_version。
- 保留 `.pre-migration.bak`。
- 下次启动重试。

健康检查失败：

1. 尝试 `PRAGMA wal_checkpoint(TRUNCATE)`。
2. 再跑 `quick_check`。
3. 仍失败时尝试最近自动备份。
4. 无可用备份时提示用户从 WebDAV 或手动备份恢复。

## 最终交付检查

交付前必须确认：

- `tauri/src` 中不再有业务代码直接写 SurrealQL。
- `DbState` 没有 clone 出裸连接。
- 任何 `.await` 前都已释放 SQLite lock。
- `update_hook` 没有替代业务语义事件。
- `backup/AGENTS.md` 已更新 SQLite manifest 规则。
- Image 资产文件仍随备份走。
- Proxy Gateway 高频状态仍不进数据库。
- OpenCode runtime SQLite 逻辑未被误改成 AI Toolbox 主库逻辑。
