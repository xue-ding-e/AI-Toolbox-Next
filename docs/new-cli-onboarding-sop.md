# 新增 CLI 工具接入 SOP

> **用途**：为 AI Toolbox 接入一个新的 CLI coding 工具（ZCode、Kimi 这类）时，按阶段执行的检查清单。
>
> **定位**：操作手册。设计规则与约束的原文以根 `AGENTS.md` 和各模块级 `AGENTS.md` 为准；本文只做整合、排序与索引，不重复规则原文。两者冲突时以规则文档为准。
>
> **依据来源**：
> - 根 `AGENTS.md`：「Tab / Page-Key Allowlist Rules」「Implementation Checklist for New Tray Integration」「Lightweight Mode」「4 Tabs WSL Direct Notes」
> - `docs/plan-kimi-code-cli.md`：§14 实施路线图 + §16 偏差与踩坑记录（目前最完整的单工具方案模板）
> - ZCode 集成实证（2026-10-06 合并）：10 个提交、63 个文件，含 3 轮「漏注册」修复
> - 记忆库 `projects/ai-toolbox/tab-allowlist-misses-new-tabs.md`（头号复发坑）
>
> **最近更新**：2026-10-06（ZCode 集成完成后）

---

## 0. 前置决策

### 0.1 先定性：配置文件型 vs 根目录型

这个选择决定后续所有路径语义，必须先定，且不能混写（见 `tauri/src/coding/AGENTS.md`）。

| 类型 | 保存对象 | 现有工具 | 路径派生方式 |
|------|---------|---------|-------------|
| **配置文件型** | 配置文件路径 | opencode、openclaw、pi、omp、hermes、dsh | 改 `config_path`，派生目录在其旁 |
| **根目录型** | 配置根目录 | claudecode、codex、grok、geminicli、antigravity、kimi、zcode | 改 `root_dir`，其余路径全部由根派生 |

判断依据：上游 CLI 是否提供「一个根目录 + 固定内部结构」（根目录型），还是「直接指向某个配置文件」（配置文件型）。

### 0.2 再定范围：做哪些能力

| 能力 | 优先级 | 参考实现 |
|------|--------|---------|
| 后端模块 + provider 管理 + 页面 | 必需 | 任意现有模块 |
| runtime_location 注册 | 必需 | `tauri/src/coding/runtime_location.rs` |
| **Allowlist 全量注册** | **必需（最高风险）** | 见第 3 节 |
| 托盘菜单 | 建议 | `tauri/src/tray.rs` + 模块 `tray_support.rs` |
| WSL/SSH 文件同步 | 建议 | `web/features/settings/hooks/useWSLSync.ts` |
| Skills / MCP 同步 | 按需 | `tauri/src/coding/tools/builtin.rs` |
| 备份 / 恢复 | 建议 | `tauri/src/settings/backup/` |
| Gateway 接管 | 可选 | claudecode / codex / kimi |
| 会话管理 | 可选 | `tauri/src/coding/session_manager/` |
| cli_resolver + 「更多选项」 | 有 CLI 调用则必需 | `tauri/src/coding/cli_resolver.rs` |

> **注意**：ZCode 集成实证表明，即使不做 Gateway 接管，`usage_stats.rs` 的读路径映射也可能需要注册（见 3.6）。

---

## 1. 阶段一：上游调研

写代码前先确认并记录以下事实，后续所有路径都从这里派生：

- [ ] CLI 名称 / npm 包名 / 可执行文件名
- [ ] 配置根目录默认值与环境变量覆盖（如 `KIMI_CODE_HOME`）
- [ ] 配置文件格式（TOML / JSON / JSONC）与主配置文件相对路径
- [ ] 是否支持自定义 provider / 官方 OAuth
- [ ] 会话存储位置与格式（决定第 10 节是否可做）
- [ ] Skills / MCP 支持情况与目录结构
- [ ] 非交互命令能力（`--version`、`session list` 等，决定 cli_resolver 探测方式）

产出格式参考 `docs/plan-kimi-code-cli.md` §1「产品概述」的事实表。

**建议**：为本工具写一份 `docs/plan-<tool>-support.md`，仿照 kimi plan 的 16 节结构。ZCode 跳过此步，结果是在收尾轮才靠排查发现多处漏注册。

---

## 2. 阶段二：后端模块骨架

新建 `tauri/src/coding/<tool>/`：

```
<tool>/
├── mod.rs            # 导出 + Tauri command 注册清单
├── types.rs          # 数据结构
├── adapter.rs        # SQLite JSONB 读写
├── constants.rs      # 默认路径、文件名常量
├── commands.rs       # apply_config_internal + Tauri commands
├── tray_support.rs   # 托盘数据获取 + apply 函数
├── projection.rs     # 配置投影（按需）
├── templates.rs      # 默认模板（按需）
└── official_accounts.rs  # 官方 OAuth（按需）
```

- [ ] `tauri/src/coding/mod.rs` 加 `pub mod <tool>;`
- [ ] `tauri/src/db/schema.rs` 的 `DbTable` 枚举加表名（`<tool>_provider` / `_common_config` / `_prompt_config` / `_official_account`，按需）
- [ ] `tauri/src/db/migrations.rs` 注册建表
- [ ] 表结构遵循 `id + data(JSONB) + created_at + updated_at`，业务字段放 JSONB（普通字段增删无需 migration）
- [ ] `apply_config_internal` 带 `from_tray` 参数（托盘与主窗口共用）
- [ ] emit `config-changed` 事件，payload 区分 `"window"` / `"tray"`

---

## 3. 阶段三：Allowlist 全量注册（最高风险）

> **这是本 SOP 存在的核心理由。** 硬编码清单散落前后端十余处，漏一处就**静默失效**（无报错、无日志），已复发 3 次（详见根 `AGENTS.md` 与记忆库记录）。

### 3.1 权威来源（先改这里）

| 来源 | 位置 | 说明 |
|------|------|------|
| `SIDEBAR_PAGE_KEYS` | `web/services/settingsApi.ts` | 侧栏专用 key（15 个，仅 coding 工具） |
| `CURRENT_DEFAULT_VISIBLE_TABS` | `tauri/src/settings/adapter.rs` | visible_tabs 全量基线（19 个，含 gateway/image/ssh/wsl） |

### 3.1.1 设置页「模块显示」的两个列表

**设置页 → 通用设置 → 模块显示**（`web/features/settings/pages/GeneralSettingsPage.tsx`）有两行模块 chip，新增 CLI **必须加进左侧那行**：

| 常量 | 对应 UI | 内容 |
|------|---------|------|
| `CODING_TABS` | **左侧**一行 | 全部 coding 工具，**新增 CLI 加这里** |
| `OTHER_TABS` | 右侧一行 | `miniBrowser` / `gateway` / `image` / `ssh` /（Windows 再加 `wsl`） |

- `CODING_TABS` 的顺序就是 chip 默认顺序，也是拖拽排序的基准（`codingTabOrder` 的初始值由它派生）。
- 漏加的表现：新 CLI **在设置页的模块显隐列表里完全不出现**，用户无法从设置页开关或排序它（kimi 集成时踩过）。
- 右侧 `OTHER_TABS` 只在新增**非 coding** 模块时才需要动；新增 CLI 不要碰它。

### 3.2 下游清单（逐一检查）

- [ ] `web/constants/modules.tsx` 的 `MODULES` subTabs —— **侧边栏显示的唯一入口**，漏了 tab 静默不显示（kimi 踩过）
- [ ] `web/features/settings/pages/GeneralSettingsPage.tsx` 的 `CODING_TABS` —— 设置页「模块显示」**左侧**列表（见 3.1.1），漏了在设置页静默消失（kimi 踩过）
- [ ] `tauri/src/settings/types.rs` 的 `AppSettings::default()`（visible_tabs）+ `default_sidebar_hidden_by_page()`
- [ ] `tauri/src/settings/adapter.rs` 的 `CURRENT_DEFAULT_VISIBLE_TABS` + 新增 `PRE_<TOOL>_DEFAULT_VISIBLE_TABS` 基线 + 匹配判断分支
- [ ] `web/services/settingsApi.ts` 的 `createDefaultSidebarHiddenByPage()` + `defaultSettings.visible_tabs`
- [ ] `tauri/src/coding/runtime_location.rs` 的 `MODULE_KEYS`（当前 14 个）
- [ ] `tauri/src/coding/reapply_applied_runtime.rs` 的 `ALL_WSL_FILE_MODULES` + `wsl_module_for_reapply_label`
- [ ] `tauri/src/settings/backup/utils.rs` 的 `ALWAYS_BACKUP_CLI_TOOLS` / `OPTIONAL_BACKUP_CLI_TOOLS` + `tool_prefixes` + `get_custom_root_dir_path_info`
- [ ] `tauri/src/tray.rs` 的 section builders + `is_tab_visible("<tab>")` 门控
- [ ] `web/features/settings/hooks/useWSLSync.ts` / `useSSHSync.ts` 的 `TAB_TO_MODULE` + `ALL_CODING_MODULES`
- [ ] `web/features/settings/components/WSLSyncModal.tsx` / `SSHSyncModal.tsx` 的 `MODULE_TO_TAB` + `ALL_MODULE_KEYS`
- [ ] `web/features/settings/components/FileMappingModal.tsx` / `SSHFileMappingModal.tsx` 模块下拉
- [ ] `web/features/settings/utils/syncMessageTranslator.ts` 同步消息翻译
- [ ] `web/components/layout/MainLayout/index.tsx` tab 图标分支
- [ ] `web/features/coding/shared/toolIcon/ToolIcon.tsx` 图标映射

### 3.3 迁移基线规则

新增**默认可见**的 tab 时：

- 全量替换 `CURRENT_DEFAULT_VISIBLE_TABS`
- 新增 `PRE_<TOOL>_DEFAULT_VISIBLE_TABS` 快照（旧基线），让仍在使用旧默认的用户通过全量替换获得新 tab
- 自定义排序用户**有意不强制插入**新 tab
- 同步更新 `visible_tabs_*` 迁移测试期望

### 3.4 兜底验证（必做）

```bash
# 把 <tool> 换成实际 tab key，全局搜字符串，逐个确认是"有意排除"还是"漏了"
rg -n "<tool>" web/ tauri/src/ --glob '!node_modules' --glob '!*.test.*'
```

漏掉的清单不会报错——**只能靠搜**。

### 3.5 回归测试要求

这类 bug 的回归测试必须断言「新增 key 能通过**完整读链路**读回其存储值」，而不是只测默认值存在。

### 3.6 Gateway 相关清单（做接管时）

- [ ] `tauri/src/coding/proxy_gateway/types.rs` 的 `GatewayCliKey` 枚举
- [ ] `web/services/proxyGatewayApi.ts` 的 `GATEWAY_USAGE_TOOLS`（统计页筛选、请求筛选的数据源）
- [ ] `web/features/settings/pages/GatewaySettingsPanel.tsx` 的 `CLI_OPTIONS`
- [ ] `web/features/coding/gateway/components/ModelPricingModal.tsx` 的 `pricingCliKeys`
- [ ] `web/features/coding/shared/gateway/providerProfiles.ts` 的 `normalizeGatewayProviderTool`
- [ ] `tauri/src/coding/proxy_gateway/usage_stats.rs` 的 `load_provider_names`（漏注册会让已落库的请求行在列表/统计中静默丢弃）

---

## 4. 阶段四：前端页面

新建 `web/features/coding/<tool>/`（pages / components / utils），配套：

- [ ] `web/services/<tool>Api.ts`（+ `<tool>PromptApi.ts` 按需）
- [ ] `web/types/<tool>.ts`
- [ ] `web/features/coding/index.ts` 加 export
- [ ] `web/app/routeConfig.ts` 注册路由（含会话详情子路由）
- [ ] `web/i18n/locales/zh-CN.json` + `en-US.json`，用 `pnpm i18n:set-key` 生成 key
- [ ] **页面头部用共享组件 `CodingPageHeader`**（见 4.1）
- [ ] **供应商列表用共享组件**（见 4.2）：`ProviderListSection` 外壳 + `ProviderCard` 卡片；有模型目录的再加 `ModelListSection`
- [ ] **模型编辑弹窗用 `ModelFormModal`**，按 CLI 能力传 `show*` 开关（见 4.2.4）
- [ ] **供应商编辑弹窗的分区用 `ProviderFormSections`**（见 4.2.6）；有协议下拉时接网关支持门控（见 4.2.7）
- [ ] **全局提示词区块用 `GlobalPromptSettings`**，传 `promptFileName`（见 4.2.8）
- [ ] UI 遵循 `DESIGN.md`（改任何可见 UI 前必须先完整阅读）

### 4.1 页面头部标准（`CodingPageHeader`）

**所有 coding tab 的页面头部必须用共享组件**，不要照抄隔壁页面。组件在 `web/features/coding/shared/CodingPageHeader.tsx`，由 Codex 页（2026-10-06）作为首个消费方验证。

**为什么有这个组件**：此前 14 个页面各写各的头部，结构一致但代码分散，新工具接入只能靠"复制隔壁 + 手改"，容易漏项（zcode 漏了「预览配置」、claudedesktop 文案硬编码中文未走 i18n）。

**标准形态**：

```
┌──────────────────────────────────────────────────────────────┐
│ <标题>  🔗官方文档  👁预览配置              ⋯ 更多选项        │
│ 配置文件路径: [<path>] ✎自定义配置目录 📁打开文件夹 ⟳刷新配置  │
└──────────────────────────────────────────────────────────────┘
```

**最小用法**（新工具接入时直接照抄，改 3 处即可）：

```tsx
import CodingPageHeader from '@/features/coding/shared/CodingPageHeader';

<CodingPageHeader
  title={t('<tool>.title')}
  docsUrl="https://<上游文档地址>"
  configPath={configPath || '~/.<tool>/config.toml'}  // 换成该 CLI 的默认路径
  onPreviewConfig={appliedProviderId ? handlePreviewCurrentConfig : undefined}
  onCustomizeConfig={() => setRootDirectoryModalOpen(true)}
  onOpenFolder={handleOpenFolder}
  onRefresh={handleRefreshPage}
  onMoreOptions={() => setSettingsModalOpen(true)}
/>
```

**props 说明**：

| prop | 说明 |
|------|------|
| `title` | 页面标题，传 `t('<tool>.title')` |
| `docsUrl` | 官方文档地址；**不传则不显示**该链接 |
| `docsText` | 覆盖默认文案 `common.viewDocs`（默认「官方文档」） |
| `onPreviewConfig` | 传了才显示「预览配置」；无 provider 应用态时可传 `undefined` |
| `configPathLabel` | 覆盖默认 `common.configPath`（默认「配置文件路径」） |
| `configPath` | 路径字符串，**调用方自己给兜底值** |
| `onCustomizeConfig` | 传了才显示「自定义配置目录」 |
| `customizeConfigText` | 覆盖默认 `common.customizeConfigDir` |
| `customizeConfigDisabled` | 迁移期禁用（opencode 场景） |
| `onOpenFolder` | 传了才显示「打开文件夹」 |
| `onRefresh` / `refreshText` | 「刷新配置」，`refreshText` 覆盖默认 `common.refreshConfig` |
| `onMoreOptions` | 传了才显示「更多选项」 |
| `extraActions` | 追加额外文字按钮（openclaw 的「打开 Web UI」、opencode 的「同步模型」） |
| `hint` | 路径行下方的提示块（opencode 的页面提示） |

**i18n 策略**：组件默认值走 `common.*`（`viewDocs` / `configPath` / `customizeConfigDir` / `openFolder` / `refreshConfig`，已补齐中英双语）。**新工具零覆盖即可用**；存量工具若已有自己的措辞，通过 `docsText` / `configPathLabel` / `customizeConfigText` / `refreshText` 传入，保证中英文都不变。

**注意**：`configPathLabel` 等覆盖项只影响**标签**；「预览配置」和「更多选项」文案固定走 `common.*`，因为 14 个页面本来就一致。

### 4.2 供应商列表标准（`ProviderListSection` / `ProviderCard` / `ModelListSection`）

供应商列表按 CLI 能力分**两种形态**，但共用同一套外壳。

| 形态 | 代表 | 特征 |
|------|------|------|
| **不支持自定义模型** | Claude Code | 卡片只有字段区（Haiku/Sonnet/Opus 等），无模型列表 |
| **支持自定义模型** | Codex | 卡片内多一个「模型列表 (N)」折叠区 |

**三个组件**（均在 `web/features/coding/shared/` 或 `web/components/common/`）：

| 组件 | 路径 | 职责 |
|------|------|------|
| `ProviderListSection` | `shared/ProviderListSection.tsx` | 区域外壳：标题 + 工具栏 + 提示 + 空态 + 底部导入 |
| `ProviderCard` | `components/common/ProviderCard/` | 单张供应商卡片（已有共享组件，本次扩展了 5 个插槽） |
| `ModelListSection` | `shared/ModelListSection.tsx` | 模型列表折叠区（仅"支持自定义模型"形态用） |

#### 4.2.1 区域外壳 `ProviderListSection`

**标准形态**：

```
┌────────────────────────────────────────────────────────────────────┐
│ 📋 供应商列表  [Gateway 胶囊]      ☑多选 🔍搜索 ⇅排序 ⚡一键测试 品通用配置 +添加供应商 │
├────────────────────────────────────────────────────────────────────┤
│ ▎可添加多套供应商配置并通过「应用」或系统托盘快捷菜单快速切换。…      │  ← hint
│ ▎注意：配置存储在应用数据库中，请勿手动修改本地配置文件，…           │
│                                                                    │
│  [ 供应商卡片 … ]                                                   │
│                                                                    │
│  [从 CC Switch 导入] [导入我使用过的供应商] [从 All API Hub 导入]     │  ← footer
└────────────────────────────────────────────────────────────────────┘
```

**最小用法**：

```tsx
import ProviderListSection from '@/features/coding/shared/ProviderListSection';

<ProviderListSection
  i18nPrefix="<tool>"
  sectionId="<tool>-providers"
  collapsed={providerListCollapsed}
  onCollapsedChange={setProviderListCollapsed}
  loading={loading}
  providerCount={providers.length}
  visibleCount={visibleProviders.length}
  batch={providerBatch}                      // useProviderBatchSelection 的返回值
  batchSelectableIds={batchSelectableIds}
  keyword={providerKeyword}
  onKeywordChange={setProviderKeyword}
  sortMode={sortMode}
  sortModes={PROVIDER_SORT_MODES}
  onSortModeChange={setSortMode}
  onBatchTest={handleBatchTestProviders}
  batchTesting={batchTestingProviders}
  onOpenCommonConfig={() => setCommonConfigModalOpen(true)}
  onAddProvider={handleAddProvider}
  headerExtra={<GatewayFailoverButton ... />} // 可选：Gateway 胶囊等
  hint={<div>…两行提示…</div>}                // 文案由调用方提供
  footer={<Space wrap>…三个导入按钮…</Space>}  // 可选
>
  {/* 卡片列表（含 DndContext） */}
</ProviderListSection>
```

**关键 props**：

| prop | 说明 |
|------|------|
| `i18nPrefix` | 决定 `provider.title` / `provider.emptyText` / `commonConfigButton` / `addProvider` 的 key 前缀 |
| `sectionId` | sidebar 锚点 id，如 `<tool>-providers` |
| `batch` / `batchSelectableIds` | 供应商级多选状态（`useProviderBatchSelection`） |
| `headerExtra` | 插槽：渲染在标题旁（Gateway 的 Failover / Aggregate 胶囊走这里） |
| `emptyText` | 空态文案；**不传则取 `${i18nPrefix}.provider.emptyText`**（见下方 i18n 陷阱） |
| `hint` / `footer` | 提示块 / 底部导入按钮；**文案由调用方传**，组件不硬编码 |
| `onBatchTest` / `onOpenCommonConfig` | 传了才渲染对应按钮 |

> **i18n key 层级陷阱（迁移必查）**
>
> 组件按固定路径取词，但各页原有的 key 层级**并不统一**——`emptyText` / `addProvider` 在有的页面是顶层（`<tool>.emptyText`）、有的是 `<tool>.provider.emptyText`：
>
> | key | 顶层（需传 `emptyText` prop） | 已在 `provider` 下 |
> |-----|------|------|
> | `emptyText` | claudecode / codex / grok / kimi / zcode / opencode | dsh / hermes / pi / ohMyPi |
> | `addProvider` | kimi / zcode / opencode | claudecode / codex / grok / pi / ohMyPi |
>
> 顶层的情况**必须显式传 `emptyText={t('<tool>.emptyText')}`**，否则空态会渲染出字面量 `<tool>.provider.emptyText`。`i18n:check` 查不出这类问题——key 是模板字面量拼的，且顶层 key 确实存在（只是路径不对）。
>
> 迁移前用这段确认目标页面的四个 key 都在 `provider` 下（或准备好用 prop 覆盖）：
>
> ```bash
> python -c "import json;d=json.load(open('web/i18n/locales/zh-CN.json',encoding='utf-8'));p=d['<tool>'];print({k:(p.get(k) or (p.get('provider') or {}).get(k)) for k in ['emptyText','addProvider','commonConfigButton']})"
> ```

**组件负责的固定项**：Collapse 骨架、`多选/退出` 切换、批量工具栏、搜索框、排序下拉、`一键测试`、`通用配置`、`添加供应商`、空态、搜索空态、`data-sidebar-section` 标记。

#### 4.2.2 卡片 `ProviderCard`

`web/components/common/ProviderCard/` 是**最基础的**共享卡片（拖拽手柄 / 多选复选框、名称、SDK 标签、baseUrl、编辑/复制/分享/删除按钮 + `extraActions`）。目前只有 `dsh` 直接用默认导出。

**Claude / Codex 不直接用它**——两者各自有模块内的 `ClaudeProviderCard` / `CodexProviderCard`，字段区差异太大（Claude 是 Haiku/Sonnet/Opus 映射行，Codex 是自动审批行 + 完整模型目录），强行参数化只会得到一个塞满条件分支的组件。

**给新 CLI 的取舍建议**：

| 情况 | 做法 |
|------|------|
| 卡片就是"名称 + 地址 + 几个按钮" | 直接用 `ProviderCard` |
| 有 CLI 专属字段区 | 参照 `CodexProviderCard` 写模块内卡片，**模型目录部分**复用 `ModelListSection` |
| 需要状态标签 / 应用按钮 / 更多菜单 | 写进模块内卡片；等**第二个** CLI 也需要同样结构时再上提到共享组件 |

> 教训：不要预先给共享组件加"以后可能会用到"的插槽。曾经给 `ProviderCard` 加了 5 个零消费者的插槽并写进文档，代码审查时被认定为误导性契约，已全部删除。

#### 4.2.3 模型列表 `ModelListSection`（仅"支持自定义模型"形态）

**标准形态**：

```
模型列表 (6)                    🗑批量删除  🧪模型测试  ☁获取模型  +添加模型
自动审批模型: deepseek-v4.1-flash  清除自动审批模型          ← aboveList
┌──────────────────────────────────────────────────┐
│ ⠿ DeepSeek V4.1 Flash (deepseek-v4.1-flash) ·当前主模型 │ ✏️ 📋 🗑 │
│   上下文限制: 400,000                              │
└──────────────────────────────────────────────────┘
```

**关键 props**：

| prop | 说明 |
|------|------|
| `models` / `rowKeyOf` | 行数据 + 行标识解析器（默认 `model.id`） |
| `sectionKey` | Collapse key，多供应商共存时必须唯一 |
| `selectionMode` / `selectedIds` / `onToggleSelection` | 批量删除选择态 |
| `onToggleBatchDeleteMode` / `onBatchDelete` | 批量删除入口与执行 |
| `onTest` / `onFetchModels` / `onAddModel` | 工具栏三按钮；**传了才渲染** |
| `onEditModel` / `onCopyModel` / `onDeleteModel` / `onSetPrimaryModel` | 行级操作 |
| `renderModelExtraActions` | 行级额外操作（Codex 的「设为自动审批模型」） |
| `aboveList` | 工具栏下方、列表上方的内容（Codex 的自动审批行） |
| `className` / `bodyStyle` / `transparentRows` | 样式适配（Codex 用透明背景 + `paddingLeft: 18`） |

> **⚠️ 行标识陷阱（Codex 实证）**：`rowKeyOf` 默认用 `model.id`，但当**同一上游模型以多个菜单名出现**时，行标识必须包含 displayName。Codex 的 `display.name` 在上游无 `displayName` 时会**回退成上游 id**，因此**不能**从 display 反推 key。正确做法是在构造 `models` 时建立 `Map<display 对象, rowKey>` 并按对象身份查回（见 `CodexProviderCard` 的 `rowKeyByDisplay`）。

#### 4.2.4 模型编辑弹窗（`ModelFormModal`）

`web/components/common/ModelFormModal/` 已实现，**按 CLI 能力动态裁剪字段**：

| props | 效果 |
|-------|------|
| `showOptions` / `showVariants` / `showModalities` | OpenCode 系字段 |
| `showInputTypes` / `showApi` / `showReasoning` / `showCompat` / `showThinkingLevelMap` | Pi 系字段 |
| `showThinkingLevel` + `thinkingLevelOptions` | Hermes 系字段 |
| `showOmpThinking` | OMP 系字段 |
| `showCost` / `showExtraParams` | 成本 / 额外参数 JSON |
| `limitRequired` / `requireCompleteLimitPair` / `nameRequired` | 校验强度 |
| `npmType` | 预设模型下拉的数据源 |

**三个必须保证的交互（已实现，勿回退）**：

1. **「选择预设模型」按钮在模型 ID 输入框右侧**（`ModelFormModal` 的 ID 字段 flex 布局内）
2. **编辑态 ID 不可编辑**（`disabled={isEdit}`），选其他预设**只更新其余字段**，不覆盖 ID
3. **新增态选预设**：填充**完整参数**（含 ID、名称、上下文/输出限制、options、variants、模态、能力）

#### 4.2.5 「获取模型」的预设匹配

`FetchModelsModal` 的 `onSuccess` 回调里，新增的模型要用 **model id 去预设模型匹配**，命中则自动填充其余参数。Codex 的实现是 `importModelsIntoCatalog(..., (modelId) => findPresetModelById(modelId, sdkType))`，配套 `fillCodexCatalogModelFromPreset`。

#### 4.2.6 编辑供应商弹窗的分区（`ProviderFormSections`）

供应商编辑弹窗在**顶部通用字段**（渠道 / 名称 / Base URL / API Key）之后，按固定顺序排列以下分区：

```
[模型映射]        ← 可选：只有走"角色→上游模型"映射的 CLI 才有（Claude 有，Codex 没有）
[高级设置]        ← 调用方自己传：JSON（Claude）还是 TOML（Codex）
[计费配置]
[自定义请求头]
[模型改写]
[备注]
```

用共享组件 `web/features/coding/shared/providerConfig/ProviderFormSections.tsx` 编排：

```tsx
import ProviderFormSections from '@/features/coding/shared/providerConfig/ProviderFormSections';

<ProviderFormSections
  editable={!isOfficialMode}          // 官方模式下只保留备注，其余分区隐藏
  modelMapping={renderModelMappingSection()}  // 不传则整个分区不渲染
  advancedSettings={<Form.Item wrapperCol={...}>…JSON 或 TOML 编辑器…</Form.Item>}
  billing={billingConfig} onBillingChange={setBillingConfig}
  customHeaders={customHeaders} onCustomHeadersChange={setCustomHeaders}
  modelRewrites={modelRewrites} onModelRewritesChange={setModelRewrites}
  i18nPrefix="<tool>"
  notesRows={3}
  notesResetKey={notesCollapseResetKey}
/>
```

**设计要点**：

| 点 | 说明 |
|----|------|
| `modelMapping` 可选 | **有映射表的 CLI 传，没有的不传**。Codex 的模型目录在卡片列表上，弹窗里就没有映射表——这是有意差异，不是缺失 |
| `advancedSettings` 由调用方传 | 编辑器类型随 CLI 变（JSON vs TOML），校验规则也各不同，不适合塞进共享组件 |
| 顶部通用字段**不共享** | 各 CLI 的字段、校验和渠道联动差异太大（`providerEndpointKey` / `apiFormat` / `configToml`…），共享收益低于成本 |
| `editable=false` | 官方模式：计费/请求头/改写全部隐藏，只留备注 |

**注意**：导入模式（从 provider 导入）的分支同样走这个组件，传 `editable={false}` + `modelMapping`。

#### 4.2.7 协议下拉的网关支持门控

供应商表单里若有**协议/格式下拉**（`apiFormat`），它的可用性依赖"该 CLI 能否被网关接管"：

```tsx
import { useGatewaySupportedCliKeys } from '@/features/coding/shared/gateway/useGatewaySupportedCliKeys';

const { isGatewaySupported } = useGatewaySupportedCliKeys();
// `undefined` = 列表还在加载；只有明确 false 才置灰，避免加载瞬间锁死用户
const gatewaySupportsThisCli = isGatewaySupported('<cliKey>') !== false;

<Select
  options={apiFormatOptions}
  disabled={!gatewaySupportsThisCli || !selectedIsCustomProviderProfile}
/>
```

**为什么**：某个 CLI 若不能被网关接管，它就没有协议转换能力，上游格式由渠道固定决定，这个下拉形同虚设。

**判定源**：后端 `GatewayCliKey::supported_mvp()`，经命令 `proxy_gateway_supported_cli_keys` 暴露。**不要在前端再维护一份镜像列表**——该列表当前为 `claude / claude_desktop / codex / grok / kimi / gemini / antigravity`。

**与 `GATEWAY_USAGE_TOOLS` 的区别**（容易混）：

| 列表 | 含义 | 数量 |
|------|------|------|
| `GatewayCliKey::supported_mvp()` | 网关**能接管**（可改配置、可转协议） | 7 |
| `GATEWAY_USAGE_TOOLS` | 只是**统计收集**，不意味着能接管 | 15 |

**当前状态**：有格式下拉的 4 个 CLI（claudecode/codex/geminicli/grok）**全部在支持列表内**，所以这个门控今天不改变任何行为。它的价值在于：将来接入**不在支持列表**的 CLI 时，格式下拉会自动置灰，无需再改代码。

#### 4.2.8 全局提示词区块

**三个组件已全部共享**，位于 `web/features/coding/shared/prompt/`：

| 组件 | 职责 |
|------|------|
| `GlobalPromptSettings` | 外壳：Collapse + 「添加全局提示词」+ 提示块 + 列表 + 拖拽排序 |
| `GlobalPromptConfigCard` | 卡片：拖拽手柄 + 展开/收起 + 应用 + 更多菜单（编辑/禁用/删除） |
| `GlobalPromptConfigModal` | 编辑弹窗（名称 + Markdown 内容） |

**接入方式**（新 CLI 只需三行）：

```tsx
import { GlobalPromptSettings } from '@/features/coding/shared/prompt';

<GlobalPromptSettings
  key={`<tool>-prompt-${promptExpandNonce}`}
  translationKeyPrefix="<tool>.prompt"
  promptFileName="AGENTS.md"          // 该 CLI 的运行时提示词文件名
  service={<tool>PromptApi}
  collapseKey="<tool>-prompt"
  refreshKey={promptRefreshKey}
/>
```

**`promptFileName` 的语义**：

- **传了** → 提示块第二行渲染共享模板 `common.globalPrompt.sectionWarning`，`{{fileName}}` 填入该文件名。
- **不传** → 回退到该 CLI 自己的 `<prefix>.prompt.sectionWarning`。

**为什么这样设计**：13 个 CLI 里有 10 个的警告句**只差文件名**（`AGENTS.md` / `CLAUDE.md` / `SOUL.md`），句子结构完全一致。与其维护 10 份重复文案，不如共享一个模板。

**哪些 CLI 不传**（它们的警告句确实不同，保留自己的文案）：

| CLI | 原因 |
|-----|------|
| `geminicli` | 指向「Gemini CLI 全局提示词文件」，不是具名文件 |
| `antigravity` | 引用绝对路径 `~/.gemini/config/GEMINI.md` |
| `hermes` | 句子结构不同（"预设存储在…请勿手动编辑本地的 `SOUL.md`"） |

**注意**：各 CLI 的 `<prefix>.prompt.sectionWarning` key **保留不删**——组件有 fallback 分支，删了会让不传 `promptFileName` 的 CLI 显示原始 key 名。

**新增 CLI 时**：默认传 `promptFileName`，并**不需要**再写 `sectionWarning` 文案。只有当该 CLI 的警告句结构与共享模板不符时，才不传并自己写一份。

**提示词文件名的事实源**：后端各模块 `constants.rs`（如 `KIMI_PROMPT_FILE` / `HERMES_PROMPT_FILE` / `DEFAULT_GEMINI_CLI_PROMPT_FILE`）；Claude Code 的在 `claude_code/commands.rs` 内联（`CLAUDE.md`）。前端目前是**手写字符串**传入，改动时需与后端常量保持一致。

#### 4.2.9 尚未迁移的页面

以下页面仍是各自内联实现，是已知待办（改前先读本节）：

| 页面 | 状态 |
|------|------|
| claudecode | ✅ 已迁移（`ProviderListSection` 试点 1） |
| codex | ✅ 已迁移（`ProviderListSection` + `ModelListSection` 试点 2） |
| 其余 8 个页面 | 结构合规，可参照两个试点迁移；卡片行数合计约 5900 行 |

### 4.3 页面头部尚未迁移的页面

以下页面**暂未**改用 `CodingPageHeader`（见 4.1）：

| 页面 | 差异点 |
|------|--------|
| zcode | 缺「预览配置」；`defaultValue` 硬编码兜底文案 |
| claudedesktop | 文案硬编码中文、未走 i18n；缺「自定义配置目录」 |
| openclaw | 预览配置用 `openclaw.previewConfig` 而非 `common.previewConfig` |
| 其余 10 个页面 | 结构合规，可直接迁移 |

---

## 5. 阶段五：托盘

按根 `AGENTS.md`「Implementation Checklist for New Tray Integration」：

- [ ] `apply_config_internal(from_tray)` 参数化
- [ ] `get_<tool>_tray_data()` 返回当前选择
- [ ] `apply_<tool>_selection()` 处理托盘选择
- [ ] `tray.rs` 接入 section（`is_tab_visible("<tab>")` 门控 + 事件 dispatch）
- [ ] **Lightweight 模式**：新增需要主窗口的入口必须先判断 `lightweight::is_lightweight_mode()`（详见根 `AGENTS.md`「Lightweight Mode」）

> ZCode 踩坑：`tray_support.rs` 写好了但 `tray.rs` 从未 import，托盘支持是死代码。

---

## 6. 阶段六：WSL/SSH 同步

- [ ] `lib.rs` 加 `wsl-sync-request-<tool>` 事件监听器（当前已有 14 个，对照补一个）
- [ ] 前端 `TAB_TO_MODULE` / `ALL_CODING_MODULES` / `MODULE_TO_TAB` / `ALL_MODULE_KEYS`（见 3.2）
- [ ] 配置文件路径通过 runtime_location 派生，**不在同步 helper 里临时查 DB/环境变量**
- [ ] WSL Direct 下 CLI 调用必须走 `wsl -d <distro> --exec`，路径转 Linux 格式
- [ ] MCP 同步映射：`tauri/src/coding/wsl/mcp_sync.rs` + `ssh/mcp_sync.rs` 加 arm
- [ ] MCP 形态转换：`tauri/src/coding/mcp/command_normalize.rs` 按需加专用 processor

> ZCode 踩坑：`is_mapped_mcp_config_file` 注册了映射 id，但后处理 match 没有 arm，导致 Windows `cmd /c` wrapper 被原样复制到 Linux 侧。

---

## 7. 阶段七：Skills / MCP

- [ ] `tauri/src/coding/tools/builtin.rs` 加内置工具定义（Skills 路径、MCP 路径、安装检测）
- [ ] 路径必须与模块 Source of Truth 一致（kimi 曾误写 `~/.kimi/*`，实际是 `~/.kimi-code/`）
- [ ] 强制复制的工具（不支持 symlink）扩展 `builtin_tool_forces_skill_copy`
- [ ] MCP 配置文件格式映射（`format_configs.rs`）

---

## 8. 阶段八：备份 / 恢复

- [ ] `settings/backup/utils.rs` 备份清单（见 3.2）+ `get_<tool>_*_path_from_db` 系列
- [ ] `settings/backup/restore.rs` 恢复分支
- [ ] 归档路径保留 data-root 子目录层级（ZCode 踩坑：扁平化后恢复到了 CLI 不读取的位置）
- [ ] `web/features/settings/components/BackupSettingsModal.tsx` 前端清单
- [ ] 恢复后 reapply：`reapply_applied_runtime.rs` 加 `<tool>` arm

---

## 9. 阶段九：Gateway 接管（可选）

- [ ] `GatewayCliKey` 加枚举值
- [ ] cli_proxy manifest（受管字段、入站路由）
- [ ] `ensure_<tool>_gateway_direct` 直连拒绝
- [ ] reapply 锁定重灌
- [ ] 前端接管入口 + 状态胶囊
- [ ] 3.6 全部清单

> Kimi 踩坑：受管字段写死 `[providers."managed:kimi-code"]`，自定义 provider 流量绕过网关导致统计恒 0。应按 `default_model → models.<key>.provider` 动态解析。

---

## 10. 阶段十：会话管理（可选）

- [ ] `tauri/src/coding/session_manager/mod.rs` 的 `SessionTool` 枚举加值
- [ ] `session_manager/<tool>.rs` 实现解析
- [ ] 导入快照路径必须走 `join_safe_relative`（kimi review 轮发现路径遍历）
- [ ] 前端 `shared/sessionManager/` 接入

---

## 11. 阶段十一：cli_resolver 与「更多选项」

- [ ] `tauri/src/coding/cli_resolver.rs` 加 `resolve_local_<tool>_program()`
- [ ] 全局 bin 候选覆盖（nvm/volta/fnm/bun/mise/asdf）
- [ ] Windows spawn 加 `CREATE_NO_WINDOW`
- [ ] `web/components/common/CliManualPathSetting.tsx` 入口
- [ ] 版本探测：`--version`/`-v`/`version`，async command + 有界超时

---

## 12. 验收

- [ ] `pnpm test` + `cargo test` + `pnpm exec tsc --noEmit` 全绿
- [ ] 至少一条「表单提交 → 持久化 → 再读取」的往返用例
- [ ] 3.4 的全局 grep 兜底通过
- [ ] 本机 + WSL Direct 两条路径都验证（runtime_location 改动必做）
- [ ] 主窗口保存、托盘刷新、WSL 设置页状态三者一致
- [ ] 新 tab 的侧栏开关重启后保持
- [ ] 新 tab 的配置文件在 WSL/SSH 同步中不被静默跳过

---

## 13. 历史踩坑汇编

| # | 坑 | 后果 | 修复 |
|---|-----|------|------|
| 1 | `get_sidebar_hidden_by_page` 7-key 白名单 | 新 tab 侧栏开关重启还原 | 改为遍历 DB 全 key |
| 2 | `TAB_TO_MODULE` 只有 7 项 | 新 tab 文件永不参与 WSL/SSH 同步 | 补全映射 + `ALL_*` |
| 3 | dsh/Hermes 未登记 runtime_location | Windows UNC 路径传给 Linux `cp`（issue #331） | 注册模块 + 保存后刷新缓存 |
| 4 | kimi：`MODULES` subTabs 漏注册 | 侧边栏静默不显示 | 补 `MODULES` |
| 5 | kimi：Gateway 统计/定价/normalize 漏注册 | 统计页看不到 kimi | 三处补注册 |
| 6 | kimi：`usage_stats.rs` 漏 `cli_key_from_app_type` | 已落库请求行被静默丢弃 | 补映射 + 回归测试 |
| 7 | kimi：受管字段写死 managed provider | 自定义 provider 流量绕过网关 | 动态解析当前生效 provider |
| 8 | zcode：`tray_support.rs` 未接入 `tray.rs` | 托盘支持是死代码 | 端到端接线 |
| 9 | zcode：`is_mapped_mcp_config_file` 有映射但后处理无 arm | `cmd /c` wrapper 复制到 Linux | 加专用 processor |
| 10 | zcode：备份归档扁平化路径 | 恢复后文件在 CLI 不读取的位置 | 保留 data-root 子目录层级 |
| 11 | zcode：`select_*_provider` 不写 `is_applied` | 默认徽章无值、启动 reapply 找不到 provider | 写入后镜像 flag |
| 12 | kimi：导入快照路径未走 `join_safe_relative` | 路径遍历漏洞 | 修复 + 恶意路径回归测试 |

---

## 附录 A：ZCode 集成改动面（实证样本）

一个「最小完整集成」实际触达的文件分布（10 提交 / 63 文件）：

- **后端 30 个**：`tauri/src/coding/zcode/`（9 个模块文件）+ `runtime_location.rs` + `reapply_applied_runtime.rs` + `session_manager/` + `tools/builtin.rs` + `settings/{types,adapter}.rs` + `settings/backup/{utils,restore}.rs` + `tray.rs` + `lib.rs` + `db/{schema,migrations}.rs` + `wsl/`+`ssh/`（mcp_sync + commands）+ `mcp/`（3 个）
- **前端 33 个**：`web/features/coding/zcode/`（7 个）+ `web/services/`（3 个）+ `web/types/` + `constants/modules.tsx` + `app/routeConfig.ts` + `MainLayout` + `toolIcon` + `features/settings/`（7 个）+ i18n（2 个）

> ZCode **未做**：Gateway 接管、`CliManualPathSetting`（页面有「更多选项」但只是配置弹窗；ZCode 模块本身不 spawn CLI，故无 cli_resolver 需求）、`docs/plan` 文档。这些是可选阶段——反过来说，**如果新工具需要调用 CLI**，cli_resolver 与手动路径入口就是必需项。

---

## 何时更新本文件

- 新增 CLI 工具完成后，把新踩的坑补进第 13 节
- 发现清单项过时或新增了硬编码清单，同步更新第 3 节并**同时**修正根 `AGENTS.md` 的对应章节
- 本文件与根 `AGENTS.md` 的「Tab / Page-Key Allowlist Rules」是配套关系：后者是规则，前者是流程
