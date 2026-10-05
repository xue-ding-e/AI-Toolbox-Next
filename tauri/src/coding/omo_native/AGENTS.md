# OmO Native 后端模块说明

## 一句话职责

- `omo_native/` 负责 **OmO Native**（上游 oh-my-openagent 5.0 起的独立 edition：`omo` 二进制 + senpi 引擎，npm 包 `omo-ai`）的运行时根目录、`[native]` 配置块、以及引擎 agent 目录（provider / MCP / skills）的可视化管理。

## 与 OpenCode 插件版 OMO 的边界（最重要）

上游 5.0 起一个仓库里并存**两套 agent 名单不相交的 edition**：

| | OpenCode 插件版 | OmO Native（本模块） |
|---|---|---|
| 用户侧 | `opencode.json` 里的 `oh-my-openagent` 插件 | `omo` 二进制 / `bun add -g omo-ai` |
| 上游包 | `packages/omo-opencode` | `packages/omo-native`（+ `omo-senpi` / `senpi-task`） |
| 配置块 | `[opencode]` | `[native]` |
| 后端模块 | `tauri/src/coding/oh_my_openagent/` | `tauri/src/coding/omo_native/` |
| 前端 | `web/features/coding/opencode/` 的 OMO 区块 | `web/features/coding/omo_native/` |

**两套名单必须分开维护**：

- Native agents（7 个）= `explore` / `librarian` / `plan-consultant` / `plan-reviewer` / `omo-native-code-reviewer` / `omo-native-gate-reviewer` / `omo-native-qa-executor`
- 插件版 agents（神话命名）= `sisyphus` / `hephaestus` / `prometheus` / `oracle` / `librarian` / `explore` / `multimodal-looker` / `metis` / `momus` / `atlas` / `sisyphus-junior`
- Native categories（10 个）**多一个 `architect`**；两边的 `deep` 都已拆成 `deep-low` / `deep-high`
- `metis` / `momus` 作为 `plan-consultant` / `plan-reviewer` 别名的窗口已在上游 5.0.0-beta.51 后关闭

把 Native 名单写进 `[opencode]`、或把神话名写进 `[native]`，都会静默变成「自定义 agent/category」而不是报错——**改动任一模块的名单前先确认改的是哪一边**。issue #405 的诉求来自 Native 文档，方向**不适用于** `oh_my_openagent`。

## Source of Truth

- **统一配置文件 `~/.omo/omo.jsonc` 是两版共用**（JSONC，允许注释与尾逗号）。本模块只读写 `[native]` 块，**绝不触碰 `[opencode]` 块与共享 base 键**。写入必须保留注释，走 `crate::coding::omo_jsonc_patch` 的原地补丁。
- 引擎状态目录 `<agentDir>`（默认 `~/.omo/agent`）：`settings.json`（默认模型）/ `models.json`（自定义 provider）/ `auth.json`（密钥，0600）/ `mcp.json`（`mcpServers`）/ `skills/` / `sessions/`。
- 长期主数据在 SQLite JSONB 表 `omo_native_settings_config` / `omo_native_agents_config`；数据库为空时页面先看到从本地文件读出的 `__local__` 桥接态。
- 运行时根目录由 `runtime_location::get_omo_native_runtime_location_async()` 决议。

## 决议顺序（镜像上游）

`<agentDir>` 按 `packages/omo-senpi/src/components/agent-home/resolve-agent-home.ts`：

1. 应用内自定义根目录（DB `omo_native_settings_config` 的 `root_dir`）
2. `OMO_CODING_AGENT_DIR` / `SENPI_CODING_AGENT_DIR` / `PI_CODING_AGENT_DIR`（进程环境）
3. 上述变量的 shell 配置（与 OMP/Pi 一致）
4. `~/.omo/agent`（哨兵 `settings.json`）
5. pre-unification 扁平布局 `~/.omo`（哨兵 `settings.json`）
6. `~/.senpi/agent`（无 branded 布局时的引擎回退）

`[native]` 块解析按 `packages/omo-config-core/src/loader/resolution.ts`：**共享 base 键 → `[senpi]`（legacy 拼写）→ `[native]`，后者胜**。所以界面读配置要展示「Native 生效视图」（折叠后），不能只看块内——否则显示的值和实际运行的不符。

## Gotchas

- **`[native]` 块的键是 `.strict()` 校验的**（上游 `OmoTypedHarnessConfigSchema`）。只允许：`formatOnMutation` / `gateway` / `categories` / `agents` / `git_master` / `task` / `teams` / `models` / `model_profiles` / `model_profile` / `memory` / `telemetry` / `computer` / `disabled_skills`。写别的（尤其 `[opencode]` 的 `disabled_agents` / `sisyphus_agent` / `claude_code` / `background_task` / `browser_automation_engine`）会触发上游 unknown-key diagnostic。`build_native_block` 与 `collect_other_native_keys` 都按这个白名单过滤。
- **不要 stamp `2026-07-opencode-config-unification`** 到 `_migrations`——那是插件版的迁移标记，Native 不该写。本模块也不动 `$schema`。
- **`models.json` 没有对外发布的 schema**，字段形状反推自上游 `packages/omo-native/bin/lib/setup-opencode-providers.js` 的 `convertProvider`。写入必须按 provider key 局部更新、保留其他 provider 与未知字段，不能全量校验式重写。
- **`auth.json` 的值是「config value」语义**：`$NAME` / `${NAME}` 插值、`!cmd` 执行 shell、`$$` / `$!` 是字面量转义。写用户密钥必须走 `providers::escape_literal_config_value`，否则含 `$` 的 key 会被引擎当变量展开。注意转义只能做一次（幂等性不成立）。
- `model_profile` 是**车道 id**（`daily-normal` / `daily-heavy` / `geeky-normal` / `geeky-heavy`，或未设置时的隐式 `recommended`）或字面 `provider/model` pin。`model_profiles.<name>` 与内建档同名时是**整体替换**，不是逐字段合并。
- **resume 旗标与 OMP 不同**：OMP 的 `-r/--resume` **接受**值（ID 前缀/路径），OmO 的 `--resume` 是**无值的交互式选择器**。按路径续接要用 `omo --session <path|id>`；写成 `omo --resume <path>` 会打开选择器并把路径当成一条聊天消息发出去。`OMO_NATIVE_IDENTITY.resume_template` 是唯一事实源，有回归测试守护。
- `[native]` 块写入是**幂等**的（连续写两次结果逐字相同）——`omo_jsonc_patch` 会保留值后原有的空白。改补丁逻辑时要保住这个性质，`writing_native_block_twice_is_idempotent` 是回归守护。
- 禁用已应用方案 = 撤回运行文件（移除 `[native]` 块），不留「已禁用但仍生效」的悬挂状态。

## 跨模块依赖

- `runtime_location`：模块 key 是 `omo_native`（别名 `omo`）。
- `omo_jsonc_patch`：与 `oh_my_openagent` 共用的 JSONC 原地补丁助手。
- 事件：`config-changed` + `wsl-sync-request-omo-native`（仅 Windows）。
- 备份：`omo_native` 在 `ALWAYS_BACKUP_CLI_TOOLS` 里（运行文件由 CLI 自己拥有）。
- 重放：`reapply_applied_runtime` 的 `reapply_omo_native` 在备份恢复后按同一份方案重新写 `[native]` 块（恢复期间不 emit 事件）。

## 最小验证

- 写入 `[native]` 块后，同一文件的 `[opencode]` 块、共享键与注释逐字未变（`tests/coding/omo_native/jsonc_isolation.rs`）。
- 连续写两次 `[native]` 块，产物逐字相同。
- `omo doctor` 对写出的 `[native]` 块不报 unknown-key / invalid-value。
- 清除 `[native]` 块后 `[opencode]` 块仍在。
- 数据库为空时能从本地 `[native]` 块读出 `__local__` 桥接态。
- apply / clear 后 `~/.omo/omo.jsonc` 的 `[native]` 块与方案一致，且 WSL 同步事件已发出。
