# 03 表清单与迁移映射

## 目标表数量

目标常规业务表按当前方案和代码核对后为 41 张。

处理原则：

- 这 41 张进入 SQLite v1 schema。
- `provider_models` 是已移除 legacy 表，不进入常规 schema；如果旧库里仍有数据，迁移日志记录并可导入为 legacy table，后续不再读取。
- `oh_my_opencode_config` / `oh_my_opencode_global_config` 是历史重命名来源。SurrealDB -> SQLite 导入前应先运行当前 SurrealDB 迁移，让它们转成 `oh_my_openagent_*`。
- OpenCode runtime 的 `session` / `message` / `part` / `session_share` 属于 OpenCode 自己的 `opencode.db`，不迁入 AI Toolbox 主数据库。

## 全局域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `settings` | 单例 | `app` | 读取/保存应用设置、启动时读取自启/托盘设置 | `tauri/src/settings/commands.rs`, `tauri/src/lib.rs`, `tauri/src/http_client.rs` |
| `app_migration` | 历史记录 | migration id | 兼容历史 SurrealDB migration marker | `tauri/src/db_migration/` |

## Claude Code 域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `claude_provider` | 多记录 | - | `is_applied`、`sort_index`、`created_at`、禁用状态 | `tauri/src/coding/claude_code/commands.rs`, `tray_support.rs`, `proxy_gateway/runtime/providers.rs` |
| `claude_common_config` | 单例 | `common` | 根目录、通用配置 | `tauri/src/coding/claude_code/commands.rs`, `runtime_location.rs` |
| `claude_prompt_config` | 多记录 | - | `is_applied`、`sort_index` | `tauri/src/coding/claude_code/commands.rs` |

## Codex 域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `codex_provider` | 多记录 | - | `is_applied`、`sort_index`、禁用状态 | `tauri/src/coding/codex/commands.rs`, `official_accounts.rs`, `tray_support.rs`, `proxy_gateway/runtime/providers.rs` |
| `codex_common_config` | 单例 | `common` | 根目录、通用配置 | `tauri/src/coding/codex/commands.rs`, `runtime_location.rs` |
| `codex_prompt_config` | 多记录 | - | `is_applied`、`sort_index` | `tauri/src/coding/codex/commands.rs` |
| `codex_official_account` | 多记录 | - | `provider_id`、`is_applied`、`sort_index`、limit 字段 | `tauri/src/coding/codex/official_accounts.rs` |
| `codex_plugin_workspace_roots` | 单例 | `settings` | workspace root 列表 | `tauri/src/coding/codex/plugin_workspace.rs` |

## Gemini CLI 域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `gemini_cli_provider` | 多记录 | - | `is_applied`、`sort_index`、禁用状态 | `tauri/src/coding/gemini_cli/commands.rs`, `tray_support.rs`, `proxy_gateway/runtime/providers.rs` |
| `gemini_cli_common_config` | 单例 | `common` | 根目录、通用配置 | `tauri/src/coding/gemini_cli/commands.rs`, `runtime_location.rs` |
| `gemini_cli_prompt_config` | 多记录 | - | `is_applied`、`sort_index`、`content` | `tauri/src/coding/gemini_cli/commands.rs` |
| `gemini_cli_official_account` | 多记录 | - | `provider_id`、`is_applied`、`sort_index` | `tauri/src/coding/gemini_cli/official_accounts.rs` |

## OpenCode / OpenClaw 域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `opencode_common_config` | 单例 | `common` | config path | `tauri/src/coding/open_code/commands.rs`, `runtime_location.rs` |
| `opencode_prompt_config` | 多记录 | - | `is_applied`、`sort_index` | `tauri/src/coding/open_code/commands.rs` |
| `opencode_favorite_plugin` | 多记录 | - | `plugin_name`、created order | `tauri/src/coding/open_code/commands.rs` |
| `opencode_favorite_provider` | 多记录 | - | `provider_id`、created order | `tauri/src/coding/open_code/commands.rs` |
| `openclaw_common_config` | 单例 | `common` | config path | `tauri/src/coding/open_claw/commands.rs`, `runtime_location.rs` |

## Oh My OpenAgent / Oh My OpenCode Slim 域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `oh_my_openagent_config` | 多记录 | - | `is_applied`、`sort_index`、禁用状态 | `tauri/src/coding/oh_my_openagent/commands.rs`, `tray_support.rs` |
| `oh_my_openagent_global_config` | 单例 | `global` | global config | `tauri/src/coding/oh_my_openagent/commands.rs` |
| `oh_my_opencode_slim_config` | 多记录 | - | `is_applied`、`sort_index`、禁用状态 | `tauri/src/coding/oh_my_opencode_slim/commands.rs`, `tray_support.rs` |
| `oh_my_opencode_slim_global_config` | 单例 | `global` | global config | `tauri/src/coding/oh_my_opencode_slim/commands.rs` |

## Skills / Tools 域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `skill` | 多记录 | - | `sort_index`、`name`、`group_id`、`management_enabled` | `tauri/src/coding/skills/skill_store.rs` |
| `skill_group` | 多记录 | - | `sort_index`、`name` | `tauri/src/coding/skills/skill_store.rs` |
| `skill_repo` | 多记录 | - | owner/name order | `tauri/src/coding/skills/skill_store.rs` |
| `skill_preferences` | 单例 | `default` | central repo path、view mode | `tauri/src/coding/skills/skill_store.rs`, `central_repo.rs` |
| `skill_settings` | 单例 | `skills` | git cache cleanup settings | `tauri/src/coding/skills/cache_cleanup.rs`, `central_repo.rs` |
| `custom_tool` | 多记录 | tool key | display name order、tool type | `tauri/src/coding/tools/custom_store.rs` |

## MCP 域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `mcp_server` | 多记录 | - | `sort_index`、`name`、tool sync fields | `tauri/src/coding/mcp/mcp_store.rs`, `config_sync.rs` |
| `mcp_preferences` | 单例 | `default` | preferences | `tauri/src/coding/mcp/mcp_store.rs` |
| `favorite_mcp` | 多记录 | - | `name`、created order | `tauri/src/coding/mcp/mcp_store.rs` |

## WSL / SSH 域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `wsl_sync_config` | 少量固定记录 | `config`, `defaults_version` | enabled、last sync、默认映射版本 | `tauri/src/coding/wsl/commands.rs`, `mcp_sync.rs`, `skills_sync.rs` |
| `wsl_file_mapping` | 多记录 | mapping id | `module`、`name` order | `tauri/src/coding/wsl/commands.rs`, `mcp_sync.rs` |
| `ssh_sync_config` | 少量固定记录 | `config`, `defaults_version` | enabled、active connection、last sync | `tauri/src/coding/ssh/commands.rs` |
| `ssh_connection` | 多记录 | connection id | `sort_order`、`name` | `tauri/src/coding/ssh/commands.rs` |
| `ssh_file_mapping` | 多记录 | mapping id | `module`、`name` order | `tauri/src/coding/ssh/commands.rs`, `mcp_sync.rs` |

## Proxy Gateway 域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `proxy_gateway_settings` | 单例 | `gateway` | 网关低频设置、enabled_on_startup | `tauri/src/coding/proxy_gateway/settings.rs`, `commands.rs`, `runtime.rs` |

注意：请求日志、请求明细、metrics rollup、模型健康快照不进入数据库，仍保持文件状态。

## Image 域

| 表 | 类型 | 固定 ID | 主要查询 | 当前入口 |
|---|---|---:|---|---|
| `image_channel` | 多记录 | - | `sort_order`、created order | `tauri/src/coding/image/store.rs` |
| `image_job` | 多记录 | - | `created_at` desc、status、asset refs | `tauri/src/coding/image/store.rs`, `commands.rs` |
| `image_asset` | 多记录 | - | `job_id`、批量按 id 读取 | `tauri/src/coding/image/store.rs`, `commands.rs` |

图片资产文件仍在 app data 的 `image-studio/assets/`，数据库只保存元数据和引用。

## 迁移期动态表策略

SurrealDB -> SQLite 导入时执行：

1. 先运行当前 SurrealDB migration，尤其是 `oh_my_openagent_rename_v1` 和 skills 名称规范化。
2. 查询 SurrealDB 实际存在的表。
3. 对目标 41 表逐表导入。
4. 对 unknown 表：
   - 记录数为 0：写入 `migration_warnings.log`，不中断。
   - 记录数大于 0：导入到同名 SQLite JSONB 表，并写入 warning；如果导入失败则整体迁移失败。
5. 对明确废弃表：
   - `provider_models`：如果仍有数据，记录 warning；不再被业务读取。
   - `oh_my_opencode_*`：如果 SurrealDB migration 后仍有数据，视为异常，写入 migration log 并失败，避免静默丢弃。
