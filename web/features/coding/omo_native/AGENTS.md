# OmO Native 前端模块说明

## 一句话职责

- `omo_native/` 页面负责 **OmO Native**（`omo` 二进制 / senpi 引擎）的 `[native]` 配置块、Agent/Category 方案、Provider、MCP 与 Skills、会话记录的可视化。

## 与 opencode tab 里 OMO 区块的边界（最重要）

上游 5.0 起同一仓库并存两套 **agent 名单不相交**的 edition。本页面只服务 Native：

| | OpenCode 插件版 | OmO Native（本模块） |
|---|---|---|
| 前端位置 | `web/features/coding/opencode/` 的 OMO 区块 | `web/features/coding/omo_native/` |
| 配置块 | `[opencode]` | `[native]`（legacy 拼写 `[senpi]`） |
| Agent 名 | 神话命名（`sisyphus` / `hephaestus` / …） | `explore` / `librarian` / `plan-consultant` / `plan-reviewer` / `omo-native-*` |
| Category | 无 `architect`，`deep` 未拆分 | 多 `architect`，`deep-low` / `deep-high` |

**两套名单必须分开维护**。把 Native 名单写进 `[opencode]`、或把神话名写进 `[native]`，都**不会报错**——只会静默变成「自定义 agent/category」。名单常量在 `web/types/omoNative.ts`（`OMO_NATIVE_AGENTS` / `OMO_NATIVE_CATEGORIES` / `OMO_NATIVE_BLOCK_KEYS`），改动前先确认改的是哪一边。

## Source of Truth

- **统一配置文件 `~/.omo/omo.jsonc` 两版共用**（JSONC，注释与尾逗号合法）。本页面只读写 `[native]` 块，**绝不触碰 `[opencode]` 块与共享 base 键**。
- `runtimeConfig.effective` 是后端按上游 `resolveOmoConfigView` 折叠出的**生效视图**（共享 base → `[senpi]` → `[native]`，后者胜），不是 `[native]` 块本身。页面必须展示生效视图：只看块内会显示与实际运行不符的值。
- `runtimeConfig.nativeBlock` 才是 `[native]` 块的原始内容，用于「来源」列判断该键来自块内还是共享 base。
- 引擎状态目录 `<agentDir>`（默认 `~/.omo/agent`）：`settings.json` / `models.json` / `auth.json` / `mcp.json` / `skills/` / `sessions/`。长期主数据在 SQLite，DB 为空时先显示从本地文件读出的 `__local__` 桥接态。

## 核心设计决策（Why）

- **`[native]` 块是 `.strict()` 校验的**（上游 `OmoTypedHarnessConfigSchema`）。只允许 `OMO_NATIVE_BLOCK_KEYS` 那 14 个键；写别的（尤其 `[opencode]` 的 `disabled_agents` / `claude_code` / `background_task`）会触发上游 unknown-key diagnostic。前端的 `OMO_NATIVE_MANAGED_KEYS` 与后端 `build_native_block` 都按这个白名单过滤。
- 会话管理直接复用 `shared/sessionManager` 的 `SessionManagerPanel tool="omo_native"`。Native 与 OMP 同源引擎（senpi），JSONL 格式同构，后端 `session_manager/oh_my_pi.rs` 是参数化的同一份解析器——**不要**在前端另写一套解析。
- Provider 面读取的是引擎自己的 `models.json` / `auth.json`，不是本应用的 provider 表。`auth.json` 的值是「config value」语义（`$NAME` 插值、`!cmd` 执行 shell），写入必须走后端 `escape_literal_config_value`。

## 易错点与历史坑（Gotchas）

- **只写 `[native]` 块**：本页面的 apply/clear 走 `omo_jsonc_patch` 的原地补丁，保留注释与 `[opencode]` 块。不要改成前端拼整份 JSON 落盘，那会抹掉注释并可能覆盖插件版的配置。
- **不要 stamp `2026-07-opencode-config-unification`** 到 `_migrations`——那是插件版的迁移标记。
- **禁用已应用方案 = 撤回运行文件**（移除 `[native]` 块），不留「已禁用但仍生效」的悬挂状态。
- `__local__` 是本地文件桥接态，不是 DB 记录：不可 apply、不可删除，托盘里也不出现。
- WSL/SSH 默认映射里 `omo-native-*` 只覆盖引擎文件（`settings.json` / `models.json` / `mcp.json` / `auth.json`）；**共用的 `~/.omo/omo.jsonc` 归 opencode 模块的 `opencode-oh-my`**，两边都登记会互相覆盖。

## 页面结构（与其他 tab 同构）

页头 = 大标题 + 文档链接 + 预览配置链接 + 配置路径行（自定义目录 / 打开目录 / 刷新）+ 右侧更多选项；
下面是分区卡片（`Collapse` + `className={styles.omoSection}`），与 OMP/OpenCode 一致。
**不要**把分区做成裸 `Collapse` 堆叠或给内容加内层 padding——那会与其他 tab 的留白不一致。

- **供应商**（`components/OmoNativeProvidersSection.tsx`）组件级复用 OMP/Pi 那一套：
  `ProviderCard` + `ProviderFormModal` 风格的表单 + `ModelFormModal` + `FetchModelsModal` +
  `ProviderConnectivityTestModal` + `shared/providerList` 的搜索/排序/批量。
  Native ↔ 共享形状的适配全在 `utils/omoNativeProviders.ts`，组件里不写映射逻辑。
- **MCP 与 Skills 不在本页内嵌**（与其他 tab 一致）：区块只给指引，实际管理走顶部的
  MCP / Skills 页——那两个页通过 `tools/builtin.rs` 的工具注册表发现 `omo_native`。
- 会话管理直接用 `SessionManagerPanel tool="omo_native"`。

## 跨模块依赖

- 后端 `omo_native::*` 命令（`web/services/omoNativeApi.ts` 一一对应）。
- `shared/sessionManager`：会话列表与详情页（路由 `/coding/omo-native/sessions/detail`）。
- `shared/toolIcon`：`omo_native` 复用侧栏的 `web/assets/omo-native.svg`。
- `shared/useRootDirectoryConfig` + `shared/RootDirectoryModal`：根目录自定义。
- 事件：`config-changed` + `wsl-sync-request-omo-native`（仅 Windows）。

## 最小验证

- 打开本页：能读到当前 `[native]` 块与生效视图；本机块为空时显示空态而非报错。
- apply 一个方案后：`~/.omo/omo.jsonc` 的 `[opencode]` 块与全部注释逐字未变，只新增/修改了 `[native]` 块。
- 反向验证：在 opencode tab 改 OMO 配置 → `[native]` 块不受影响。
- 会话管理能列出 `~/.omo/agent/sessions/` 下的记录并打开详情。
