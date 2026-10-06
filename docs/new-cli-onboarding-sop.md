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

### 2.1 行 id 约定：**不要含冒号**

`adapter.rs` 的读取函数默认调 `crate::coding::db_id::db_extract_id`，它会把**第一个 `:` 之前的内容当作遗留的 `table:id` 前缀剥掉**：

```rust
db_extract_id(&json!({"id": "custom:axonhub-deepseek"}))  // → "axonhub-deepseek"  ← 剥错了
```

后果是**写库成功、读回被篡改**：返回给前端的 id 与真实行 id 不一致，后续按 id 的每一次调用（投影、删除、应用、编辑）都会报 `not found`，而数据明明在表里。

ZCode 是唯一一个**业务 id 合法含冒号**的模块——它的托管供应商 id 必须以 `custom:` 开头（`ZCODE_MANAGED_PROVIDER_ID_PREFIX`），且这个 id 同时就是行 id。所以它不能用共享的 `db_extract_id`，必须在自己的 adapter 里读原始值。

**规则**：

| 情况 | 做法 |
|------|------|
| 行 id 是不含冒号的 UUID / slug | 用 `db_extract_id`（默认，保持不变） |
| 行 id 是**业务 id 且业务 id 允许含冒号** | 自己读 `value["id"]` 原始字符串，**不要**走 `db_clean_id` |
| 能让行 id 与业务 id 分离（行 id 用 UUID） | 首选。这样既保留 `db_extract_id`，又不把业务约束泄漏到主键 |

> 若选了第三种（UUID 行 id），业务 id 只存 JSONB 里，`db_extract_id` 可正常工作。ZCode 采用第二种是因为历史实现已把两者合一。
>
> **配套**：无论选哪种，都要加一条「往返」回归测试——用真实的含冒号 id 走一遍 `from_db_value_*`，断言 id 原样返回。这类 bug 单看代码读不出来。

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
- [ ] **先做 §4.0 的「选参照 CLI + 逐项核对形态」** —— 这是本阶段最容易返工的环节，不要跳过
- [ ] **页面头部用共享组件 `CodingPageHeader`**（见 4.1）
- [ ] **供应商列表用共享组件**（见 4.2）：`ProviderListSection` 外壳 + `ProviderCard` 卡片；有模型目录的再加 `ModelListSection`
- [ ] **模型编辑弹窗用 `ModelFormModal`**，按 CLI 能力传 `show*` 开关 + `toolName`；只有字段语义/示例确实不同才用 `messageOverrides`（见 4.2.4）
- [ ] **供应商编辑弹窗的分区用 `ProviderFormSections`**（见 4.2.6）；有协议下拉时接网关支持门控（见 4.2.7）
- [ ] **全局提示词区块用 `GlobalPromptSettings`**，传 `promptFileName`（见 4.2.8）
- [ ] UI 遵循 `DESIGN.md`（改任何可见 UI 前必须先完整阅读）

### 4.0 迁移前必做：选参照 CLI + 逐项核对形态

> **这一节是本 SOP 最重要的一节。** 以下是实际踩过的教训：迁移共享组件时，最容易犯的错误是把「换成共享组件」理解成「替换组件引用」。正确理解是「**对齐参照 CLI 的形态**」——组件只是承载形态的容器。只换组件、不对齐形态，结果是「用了共享组件但长得像另一个产品」。

#### 4.0.1 第一步：为每类界面指定一个参照 CLI

不要凭印象，先明确「照着谁改」：

| 界面 | 参照 | 理由 |
|------|------|------|
| 供应商列表 / 卡片 | **Codex** | 功能最全：模型目录 + 网关门控 + 批量操作 |
| 供应商编辑弹窗 | **Codex** | 含渠道行、协议门控、完整分区 |
| 模型编辑弹窗 | **Pi / Hermes** | 可选字段最多，`show*` 覆盖面最广 |
| 页面头部 | **Codex** | 首个消费方，无遗留覆盖 prop |
| 模型列表工具栏 | **Codex** | 传全了全部 handler |

#### 4.0.2 第二步：逐项核对（机械清单，不许凭印象）

打开参照文件逐行读，**不是**「我记得应该差不多」。ZCode 迁移时漏掉的每一项都在下面：

**A. 外层容器（共享组件管不到的部分）**

- [ ] `layout` / `labelCol` / `wrapperCol` —— **共享组件只管内部，外层布局仍由调用方写**。
  > ZCode 保留了 `layout="vertical"`（标签在输入框上方），而其他 5 个 CLI 全是 `layout="horizontal"` + `labelCol={span: 4|6}` / `wrapperCol={span: 20}`。弹窗看起来像另一个产品。
- [ ] `width` / `title` / `okText` / `cancelText` / `destroyOnHidden`
- [ ] 表单字段的**顺序与数量**：逐字段列出「参照有我没有」「我有参照没有」，确认每一处差异都是**有意的**

**B. 第一行放什么**

- [ ] Codex 供应商表单**第一行是「渠道」**：左边渠道选择器 + 右边格式选择器，**同一行**（grid 两列），下方跟 hint。
  > ZCode 把渠道放在第 3、4 个字段且分成两行，用户第一眼看到的是「名称」而不是「选渠道」。

**C. 每个字段的交互细节**

- [ ] API Key 是否有**显示/隐藏按钮**（4/5 的 CLI 用 `masked input + addonAfter` 按钮；ZCode 用了朴素的 `Input.Password`）
- [ ] 是否该用 `ImeSafeInput` / `ImeSafeAutoComplete`（IME 组合输入安全）
- [ ] hint 走 `help` 还是 `extra`，字号是 11 还是 12

**D. 下拉的选项列表**

- [ ] 是否有**显式的「自定义」选项**，还是靠 placeholder 暗示。
  > **不要用 `allowClear` + placeholder 表达「不选」。** Codex 给「自定义」一个具名选项（`CUSTOM_PROVIDER_ENDPOINT_KEY`），语义明确；ZCode 用 `allowClear` + 「不使用模板」placeholder，用户看不出「留空 = 自定义」。
- [ ] 选项数据源、`showSearch` / `allowClear` 是否与参照一致

**E. 卡片的详情区**

- [ ] Codex 是**一行**：`baseUrl` + 格式 Tag + API Key + 备注，用 `|` 分隔。
  > ZCode 渲染了**三行**（id / baseUrl / 备注）。注意参照不显示 provider id —— 对自动生成的 id 它只是名称的 slug 副本。
- [ ] 操作按钮的数量、**样式**（`type="link"` / `type="text"` / default）、图标、禁用条件
  > ZCode 的「应用」用了 default 按钮（带边框），其他 CLI 全是 `type="link"`（蓝色文字）。

**F. 工具栏的按钮全集（最容易漏的一类）**

- [ ] 枚举共享组件的**全部**可选 prop，逐个决定「传 / 不传」，并记录理由。
  > `ModelListSection` 的按钮是「传了 handler 才渲染」。ZCode 只传了 1 个，于是工具栏只有「添加模型」——**看起来像设计如此，实际是漏传**。
  >
  > 位置全集：批量删除入口 / 模型测试 / 获取模型 / 添加模型 + 每行的编辑 / 复制 / 删除 / 设为主模型 / `renderModelExtraActions`。

**G. 状态与空态**

- [ ] 空态 / 搜索空态 / 加载态 / 禁用态文案是否与参照一致

#### 4.0.3 第三步：核对共享组件的**前置条件**

共享组件不是无条件适用的，用之前先确认它的前提：

- [ ] **分区可能只对部分 CLI 有效。** `ProviderFormSections` 的**计费 / 自定义请求头 / 模型改写三块只由本地网关消费**（`inject_custom_headers` / `resolve_upstream_model_id` 是唯一读者）。CLI 不在 `GatewayCliKey::supported_mvp()` 里就必须传 `show*=false`。
  > ZCode 不在网关支持列表，三个分区填了没有任何代码读；更糟的是表单还把值写进 `meta`，制造了「配置已生效」的假象。
  >
  > **推论**：看到共享组件有 `meta` 合并 helper（`merge*IntoMeta`），先确认「谁读这个 key」，再决定是否调用。

- [ ] **组件可能暗示了不属于本 CLI 的概念。** 例如 billing 意味着「该 CLI 的请求会经过网关计费」。

#### 4.0.4 第四步：验证方式

**「编译通过」不等于「形态对了」。** 编译只保证类型正确，不保证长得对。

- [ ] 对照参照 CLI **打开实际界面比对**（截图对比最有效）
- [ ] 逐条走 4.0.2 的清单，每项在界面上指认一次
- [ ] 若无法运行界面，**在 PR/提交说明里明确写出「未做视觉核对」**，不要默认通过

> ZCode 这一轮累计返工 8 次（布局 / API Key 按钮 / 渠道行位置 / 自定义选项 / 网关门控 / 卡片详情行 / 工具栏按钮 / 静默错误），全部属于「编译通过但形态不对」——若第一步就做了逐项核对，这些都不会发生。

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
| `onPreviewConfig` | 传了才显示「预览配置」；无 provider 应用态时可传 `undefined` |
| `configPath` | 路径字符串，**调用方自己给兜底值** |
| `onCustomizeConfig` | 传了才显示「自定义配置目录」 |
| `customizeConfigDisabled` | 迁移期禁用（opencode 场景） |
| `onOpenFolder` | 传了才显示「打开文件夹」 |
| `onRefresh` | 传了才显示「刷新配置」 |
| `onMoreOptions` | 传了才显示「更多选项」 |
| `extraActions` | 追加额外文字按钮（openclaw 的「打开 Web UI」、opencode 的「同步模型」） |
| `hint` | 路径行下方的提示块（opencode 的页面提示） |

**文案一律走 `common.*`，没有 per-CLI 覆盖 prop**：`viewDocs` / `configPath` / `customizeConfigDir` / `openFolder` / `refreshConfig` / `previewConfig` / `moreOptions`。

> **为什么不做覆盖 prop（踩过的坑）**
>
> 组件最初提供了 `docsText` / `configPathLabel` / `customizeConfigText` / `openFolderText` / `refreshText` 五个覆盖项，理由是要"保持既有中英文文案不变"。实际统计 14 个页面后发现这些"差异"全是同义异写，没有一处是真实语义差别：
>
> | 文案 | 实际分布 |
> |------|---------|
> | 打开文件夹 | **14/14 完全相同** |
> | 刷新配置 | 12 个「刷新配置」，zcode/geminicli 是「刷新」 |
> | 官方文档 | 「官方文档」 vs 「查看文档」 |
> | 自定义配置目录 | 「自定义配置目录」 vs zcode「自定义根目录」 |
>
> 覆盖 prop 不但没保住什么，反而让 zcode 的「刷新」这种不一致固化了下来。**已全部删除**，组件只用 `common.*`；codex/zcode 的 8 个孤儿 key 一并 prune。
>
> **唯一保留的例外**：`pi` / `ohMyPi` 显示的是**目录**（`rootPathInfo.path`）而非配置文件路径，标签是「配置目录路径」。这是真实语义差异，但它们尚未迁移；迁移时应给组件加 `configPathLabel` 之类的 prop，而不是把差异抹平。
>
> **教训**：不要为了"迁移时不动文案"而预先加覆盖 prop。先把各页面的值统计出来，同义的直接统一，只有真正不同的才参数化。

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
| `ProviderCard` | `components/common/ProviderCard/` | 最基础的共享卡片（无 CLI 专属插槽，见 §4.2.2） |
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
  emptyTextHint={t('<tool>.importFromX')}      // 可选：仅当能从此处导入时
  hint={<div>…两行提示…</div>}                // 文案由调用方提供
  footer={<Space wrap>…三个导入按钮…</Space>}  // 可选
>
  {/* 卡片列表（含 DndContext） */}
</ProviderListSection>
```

**关键 props**：

| prop | 说明 |
|------|------|
| `sectionId` | sidebar 锚点 id，如 `<tool>-providers` |
| `batch` / `batchSelectableIds` | 供应商级多选状态（`useProviderBatchSelection`） |
| `headerExtra` | 插槽：渲染在标题旁（Gateway 的 Failover / Aggregate 胶囊走这里） |
| `emptyTextHint` | 追加在通用空态文案**下方**的一句补充，仅用于说明导入来源 |
| `hint` / `footer` | 提示块 / 底部导入按钮；**文案由调用方传**，组件不硬编码 |
| `onBatchTest` / `onOpenCommonConfig` | 传了才渲染对应按钮 |

**文案一律走 `common.provider.*`，没有 `i18nPrefix`**：标题「供应商列表」、按钮「添加供应商」/「通用配置」、空态「暂无供应商配置，点击上方按钮添加」。组件不再按 `<tool>.provider.*` 取词——那套 per-CLI key 层级混乱（有的在顶层、有的在 `provider` 下），是空态渲染出字面量 key 的根因。claudecode / codex 迁移时产生的孤儿 key 已 prune。

> 各 CLI 的空态文案内容确实有差异（claudecode 要提「或从 OpenCode 导入」），但**差异只在补充说明**，主体是同一句。所以拆成「通用基座 + 可选 `emptyTextHint`」，而不是让每个 CLI 传整句。

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
| `toolName` | 插值进 `capabilitiesHint` / `inputTypesHint`（"支持 **Pi** 的 extended thinking"） |
| `messageOverrides` | 见下方 |

**文案策略**：默认全部走 `common.model.*`（约 55 个 key）。只有**真正因工具而异**的才用 `messageOverrides` 覆盖：

```tsx
<ModelFormModal
  toolName="Pi"
  messageOverrides={{
    idPlaceholder: t('pi.model.idPlaceholder'),          // 示例模型名不同
    contextLimit: t('pi.model.contextLimit'),            // 字段叫法不同
    thinkingLevelHint: t('pi.model.thinkingLevelHint'),  // 字段语义不同
  }}
/>
```

> **`messageOverrides` 的值必须是 `t(...)` 的结果，不能是 key 字符串。**
>
> 最初写成 `idPlaceholder: 'pi.model.idPlaceholder'`（传 key，组件再 `t()`），结果 `i18n:prune` 把 `pi.model.idPlaceholder` 判定为"无人使用"并删除——静态分析看不见对象字面量里的字符串。改成 `t(...)` 后：值就是最终文本，组件用 `??` 短路，工具链也能正常追踪。
>
> 改这个语义时顺带暴露了两个**早就存在的坏 key**（`i18n:check` 现在能看见了）：
> - `omoNative.model.*` 整组不存在（页面一直渲染字面量 key 名）
> - `hermes` 借用 `pi` 前缀，`hermes.model.thinkingLevelHint` 并不存在
>
> 教训：**传 key 字符串给组件做延迟翻译，会同时骗过 i18n 检查工具和未来读代码的人。**

**哪些 key 需要覆盖**（各 CLI 的真实差异）：

| key | 差异性质 |
|-----|---------|
| `idPlaceholder` | 示例模型名（`deepseek-chat` / `gpt-4o` / `claude-sonnet-4.5`） |
| `name` / `namePlaceholder` / `nameOptionalPlaceholder` | 字段叫法（"模型名称" vs "显示名称"） |
| `contextLimit` / `outputLimit` (+Placeholder) | 叫法与示例（"上下文限制" vs "上下文"） |
| `reasoning` | "推理" vs "支持思考" |
| `costHint` / `extraParamsHint` | 是否提示"留空则删除" |
| `thinkingLevelHint` / `thinkingLevelMapHint` | 写入位置不同（如 `agent.reasoning_overrides`） |

其余（`addModel` / `editModel` / `id` / `api*` / `compat*` / `cost*` / `inputTypes*` / `variants*` / 各类校验消息）**都是同一句话，直接共用 `common.model.*`**。

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
  notesRows={3}
  notesResetKey={notesCollapseResetKey}
/>
```

备注区文案走 `common.provider.notes` / `common.provider.notesPlaceholder`（各 CLI 5/5 相同，无需覆盖）。

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
  toolName="Grok"                     // 插值进提示块与名称占位符
  promptFileName="AGENTS.md"          // 该 CLI 的运行时提示词文件名
  service={<tool>PromptApi}
  collapseKey="<tool>-prompt"
  refreshKey={promptRefreshKey}
/>
```

**文案一律走 `common.prompt.*`，没有 `translationKeyPrefix`**。所有 22 个 key（标题、应用/禁用/删除确认、空态、名称与内容校验、占位符……）都通用化了，13 个 CLI 的 `<prefix>.prompt.*` 副本已全部 prune（每个 22 个，共 286 个）。

**两个插值 prop**：

| prop | 用途 | 不传时 |
|------|------|--------|
| `toolName` | 提示块（"预设多套 **Grok** 全局提示词方案"）与名称占位符（"默认 **Grok** 助手"） | 必传 |
| `promptFileName` | 警告句、本地文件提示、内容占位符里的文件名 | 回退为字面量 `prompt` |

**为什么这样设计**：13 个 CLI 的提示词文案**只有工具名和文件名不同**，句子结构完全一致（`namePlaceholder` 是「默认开发助手」vs「默认 Grok 助手」，`contentPlaceholder` 只差写入哪个文件）。与其维护 13 份重复文案，不如共享模板 + 两个插值参数。

> **`toolName` 的取值**：用各页原本 `namePlaceholder` 里的工具名——`Grok` / `Pi` / `Oh My Pi` / `ZCode` / `Claude Code` 等。原本写「默认开发助手」的 5 个页面（claudecode / claudedesktop / codex / opencode / zcode）现在会显示各自的工具名，这是有意的改进。
>
> **`promptFileName` 不传的两个页面**：geminicli（指向「Gemini CLI 全局提示词文件」，非具名文件）、antigravity（引用绝对路径）。它们现在走通用句子 + 字面量 `prompt`，**文案比原来弱**——如果要恢复精确表述，应给组件加一个 `promptFileLabel` prop，而不是把 22 个 key 再拆回去。

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
| zcode | ✅ 已迁移（2026-10-06，含搜索/排序/多选/拖拽/一键测试/通用配置） |
| claudedesktop | 文案硬编码中文、未走 i18n；缺「自定义配置目录」 |
| openclaw | 预览配置用 `openclaw.previewConfig` 而非 `common.previewConfig` |
| 其余 10 个页面 | 结构合规，可直接迁移 |

---

## 5. 阶段五：系统托盘快捷菜单

托盘不是「每个 CLI 注册一次」，而是「一个巨型 `tray.rs` + 每模块一个 `tray_support.rs`」。

### 5.1 模块侧：`tauri/src/coding/<tool>/tray_support.rs`

参照 `zcode/tray_support.rs`（151 行），必须导出：

| 导出 | 说明 |
|------|------|
| `TrayProviderItem { id, display_name, is_selected, is_disabled }` | 单个供应商行 |
| `TrayProviderData { title, current_display, items }` | 供应商子菜单数据 |
| `TrayPromptItem` / `TrayPromptData` | 全局提示词子菜单数据 |
| `async fn is_enabled_for_tray(app) -> bool` | ZCode 恒 `true`；**可见性由 `tray.rs` 的 `is_tab_visible("<tab>")` 决定**，不要在这里判 |
| `async fn get_<tool>_tray_data(app)` | 返回当前选择 |
| `async fn apply_<tool>_provider(app, row_id)` | 处理托盘点击 |
| `async fn get_<tool>_prompt_tray_data` / `apply_<tool>_prompt_config` | 提示词侧 |

**关键陷阱（ZCode 踩过）**：托盘回传的是 **DB 行 id**，但很多 CLI 内部按自己的 providerId 索引。ZCode 在 `tray_support.rs:96-119` 做了一次 row_id → providerId 的解析（查 DB 行 → 解析 settingsConfig → 调 `select_*_internal_without_events`）。**直接拿 row_id 当 providerId 用会选错或选不中。**

**刷新机制**：`apply_*` 函数本身不刷新菜单；由 `tray.rs` 的 dispatch 分支在 await 之后调 `refresh_tray_menus`。另有全局链路：任何 `app.emit("config-changed", ...)` → `lib.rs` 的全局 listener → `tray::refresh_tray_menus`。

### 5.2 `tauri/src/tray.rs`：9 个必改点

以 zcode 为例（行号为当前工作区）：

| # | 位置 | 内容 |
|---|------|------|
| 1 | `:17-35` | `use crate::coding::<tool>::tray_support as <tool>_tray;` |
| 2 | `:44-80` | `struct TrayTexts` 加 `<tool>_header: &'static str` |
| 3 | `:169`(en) / `:204`(zh) | **两处**都要给值（漏一处 → 该语言下标题为空） |
| 4 | `:467-488` | dispatch 分支：`event_id.strip_prefix("<tool>_provider_")` + `"<tool>_prompt_"`，spawn → `apply_*` → `refresh_tray_menus` |
| 5 | `:848` | `let <tool>_enabled = is_tab_visible("<tab>") && <tool>_tray::is_enabled_for_tray(app).await;` |
| 6 | `:1018-1038` | 取数据 + **else 分支构造空 `TrayProviderData` 并覆写 `.title`** |
| 7 | `:1509-1526` | `<tool>_has_items` / `<tool>_has_prompt_items` / `<tool>_has_section` |
| 8 | `:1567-1576`、`:1877-1896` | 构造 `<tool>_prompt_submenu`、`<tool>_header`（`MenuItem::with_id`）、`<tool>_provider_submenu` |
| 9 | `:2139-2151` + `:3084-3108` + `:3390-3416` | ① section append 进 `menu`；② `impl NamedPromptTrayItem/Data`；③ `impl NamedProviderTrayItem/Data` |

**好消息**：第 9 点的两个 trait impl 漏了**编译不过**（不是静默失效）。真正的静默失效是 `tray.rs` 根本没 import `tray_support`——ZCode 的 142 行全是死代码，端到端接线后才生效。

**MCP / Skills 区不用改 `tray.rs`**：它们的工具列表来自 `get_mcp_runtime_tools` / `get_all_tool_adapters`，注册进 `BUILTIN_TOOLS` 后自动出现（见第 7 节）。

### 5.3 Lightweight 模式

新增**需要主窗口**的入口必须先判断 `lightweight::is_lightweight_mode()`（详见根 `AGENTS.md`「Lightweight Mode」）。托盘自身的 `show` / `lightweight_mode` 项已覆盖，但新 CLI 若加「打开页面」类菜单项必须自己处理。

### 5.4 其他

- **事件 id 前缀必须唯一**：dispatch 用 `strip_prefix` 顺序匹配，若新 CLI key 是另一个 key 的前缀会误命中。
- `tray.rs:796-809` 的 fallback `visible_tabs` 数组（`get_settings` 读失败时兜底）也建议加上新 tab。

---

## 6. 阶段六：WSL / SSH 同步

### 6.1 路径解析：单一事实源

所有同步路径从 `runtime_location.rs` 派生，**不在同步 helper 里临时查 DB/环境变量**。

| 位置 | 符号 |
|------|------|
| `runtime_location.rs:16` | `const MODULE_KEYS: [&str; 14]`（zcode 在 `:25`）—— WSL Direct 状态、缓存刷新、`get_wsl_direct_status_map_async` 的唯一来源 |
| `:199` | `normalize_module_key()` match（`"zcode" \| "zcode_cli" => Some("zcode")`） |
| `:276` | `get_runtime_location_async` dispatch match |
| `:1200-1253` | `get_<tool>_config_path_*` / `get_<tool>_mcp_config_path_*` / `get_<tool>_prompt_path_*` / **`get_<tool>_wsl_target_path_async(db, file_name)`** |
| `:1149` | `resolve_<tool>_root_dir_without_db()`（给 backup restore 用的无 DB 入口） |

`get_<tool>_wsl_target_path_async` 就是「Windows 路径 → WSL 内路径」的映射器，范式：

```rust
match get_<tool>_runtime_location_async(db).await {
    Ok(location) => location.wsl
        .map(|wsl| format!("{}/{}", wsl.linux_path.trim_end_matches('/'), file_name))
        .unwrap_or_else(|| format!("~/.<tool>/{file_name}")),
    Err(_) => format!("~/.<tool>/{file_name}"),
}
```

### 6.2 默认文件映射表

**WSL**（`tauri/src/coding/wsl/commands.rs`）：

| 位置 | 符号 | 说明 |
|------|------|------|
| `:957` | `const CURRENT_DEFAULTS_VERSION: u64` | **每次新增默认映射必须 +1** |
| `:992` | `const DEFAULT_MAPPING_IDS_ADDED_IN_V<N>: &[&str]` | 本次新增的 mapping id |
| `:1077` | `should_backfill_versioned_mapping(...)` | 老用户回填的判定调用 |
| `:2475` | `default_mappings()` 的 `FileMapping` 结构体 | `id` / `name` / `module` / `windows_path` / `wsl_path` / `is_directory` |
| `:1476` | `resolve_dynamic_paths_with_db()` match arm | 把默认 `~/.<tool>/...` 换成**实际解析出的**路径（自定义根目录时关键） |

ZCode 的 4 条映射：

```
<tool>-provider-config  module=<tool>  ~/.<tool>/v2/provider_config.json  (file)
<tool>-prompt           module=<tool>  ~/.<tool>/AGENTS.md                (file)
<tool>-cli-config       module=<tool>  ~/.<tool>/cli/config.json          (file)
<tool>-skills           module=<tool>  ~/.<tool>/skills                   (dir)
```

> ZCode 的 `credentials.json` **有意排除**：AES-GCM key 由平台 + home + 用户名派生，跨机无法解密。代码里有注释。

**SSH**（`tauri/src/coding/ssh/commands.rs`）：结构同构，但 `CURRENT_DEFAULTS_VERSION` 是**独立编号**（`:1148`），字段名是 `local_path` / `remote_path`，另有 `zcode_remote_target_path_from_location()`（`:1844`）。

### 6.3 MCP 同步（两条链路，各两个点）

| 位置 | 符号 | 漏改后果 |
|------|------|---------|
| `wsl/mcp_sync.rs:324` | `is_mapped_mcp_config_file()` 白名单 | 该 CLI 的 MCP 配置**完全不参与 WSL 同步**（静默） |
| `wsl/mcp_sync.rs:366` | `strip_cmd_c_from_wsl_mcp_file()` 的 match | **注册了 id 但没 arm → `_ => return Ok(())`，Windows `cmd /c` wrapper 原样复制到 Linux，WSL 里 server 起不来**（ZCode 实证） |
| `ssh/mcp_sync.rs:332` / `:386` | SSH 版同上 | 同上 |
| `mcp/command_normalize.rs` | 按需新增 `process_<tool>_json` | 无专用 processor 就用通用形状，CLI 可能读不懂 |
| `mcp/format_configs.rs:80` | `get_format_config()` match | `_ => None` → 用通用 key 写入，CLI 只认自己的 key 就**静默丢值**（zcode 的 `timeoutMs` vs `timeout` 实证） |

### 6.4 事件、reapply、状态

| 位置 | 符号 | 漏改后果 |
|------|------|---------|
| `lib.rs:1663` | `app.listen("wsl-sync-request-<tool>", ...)` | 保存配置后**不自动同步**（当前 14 个监听器，对照补） |
| `reapply_applied_runtime.rs:234` | `wsl_module_for_reapply_label()` match | **漏 arm → 该模块不进 `changed_modules` → 被 `unchanged_wsl_modules` 判为「未变」→ 恢复后 WSL sync 静默跳过它** |
| `reapply_applied_runtime.rs:264` | `const ALL_WSL_FILE_MODULES: &[&str]` | 漏项 → 该模块永远不被 skip（可能误覆盖本机运行时文件） |
| `reapply_applied_runtime.rs:105` | `reapply_cli(&mut summary, "<tool>", ...)` 调用点 | 恢复后不 re-apply |

### 6.5 前端注册（全部会静默失效）

| 文件 | 符号 | 漏改后果 |
|------|------|---------|
| `useWSLSync.ts:56` / `:72` | `TAB_TO_MODULE` / `ALL_CODING_MODULES` | 映射为 `undefined` → `.filter(Boolean)` 丢弃 → 塞进 `skipModules` → **文件永不参与 WSL 同步**（复发 2 次的头号坑） |
| `useSSHSync.ts:23` / `:40` | 同上 | 同上（SSH） |
| `WSLSyncModal.tsx:74` / `:82` | `MODULE_TO_TAB` / `ALL_MODULE_KEYS` | 模块 tab 在同步弹窗里**看不见** |
| `SSHSyncModal.tsx:104` / `:112` | 同上 | 同上 |
| `FileMappingModal.tsx:250` | `<Select.Option value="<tool>">` | 手动加映射时选不到该模块 |
| `SSHFileMappingModal.tsx:153` | 同上 | 同上 |
| `syncMessageTranslator.ts:182` | `BUILTIN_MAPPING_LABELS` 每个 mapping id 一条 | 同步结果消息里显示原始 id 而非本地化名 |

### 6.6 本节的静默失效陷阱汇总

1. **`default_mappings()` 加了结构体但没加进 `DEFAULT_MAPPING_IDS_ADDED_IN_V<N>` / 没 bump `CURRENT_DEFAULTS_VERSION`**：老用户永远拿不到新映射，**只有全新安装才有**。无任何提示。
2. **`resolve_dynamic_paths_with_db` 漏 arm**：映射保留字面 `~/.<tool>/...`，用户设了自定义根目录时同步的是**错误路径**（通常表现为 skipped 而非 error）。
3. **`TAB_TO_MODULE` 漏项**：复发 3 次的头号坑（`AGENTS.md:1111-1113` 有完整记录）。
4. **`runtime_location::MODULE_KEYS` 漏项**（issue #331）：模块不在 Direct 状态列表里 → 后端不知道它已是 WSL Direct → 把 Windows UNC 路径 `//wsl.localhost/...` 交给 Linux `cp`。
5. **`is_mapped_mcp_config_file` 注册了但 `strip_cmd_c` 没 arm**：`cmd /c` 原样进 Linux（ZCode 实证）。
6. **`wsl_module_for_reapply_label` 漏 arm**：恢复后 WSL sync 把该模块当「未变更」跳过。

---

## 7. 阶段七：Skills / MCP 管理

### 7.1 核心事实：工具枚举只有一个权威来源

`tauri/src/coding/tools/builtin.rs:14` 的 `BUILTIN_TOOLS: &[BuiltinTool]` 是**唯一**的工具枚举常量表：

```rust
pub struct BuiltinTool {
    pub key: &'static str,
    pub display_name: &'static str,
    pub relative_skills_dir: Option<&'static str>,
    pub relative_detect_dir: Option<&'static str>,
    pub mcp_config_path: Option<&'static str>,
    pub mcp_config_format: Option<&'static str>,  // "json"|"toml"|"jsonc"|"yaml"|"cordis"
    pub mcp_field: Option<&'static str>,
}
```

zcode 的 entry（`builtin.rs:384-392`）：

```rust
BuiltinTool {
    key: "zcode", display_name: "ZCode",
    relative_skills_dir: Some("~/.zcode/skills"),
    relative_detect_dir: Some("~/.zcode"),
    mcp_config_path: Some("~/.zcode/cli/config.json"),
    mcp_config_format: Some("json"),
    mcp_field: Some("mcp.servers"),   // 支持点分嵌套路径
},
```

**加一条 entry 就够**：`get_all_builtin_tools()` / `get_skills_builtin_tools()` / `get_mcp_builtin_tools()` / `get_all_tool_adapters()` / `get_mcp_runtime_tools()` 全是派生函数，MCP 页、Skills 页、托盘 MCP 区、托盘 Skills 区、以及它们的 WSL/SSH 同步**全部自动生效**。

**前端零改动**（除图标）：`useMcpTools.ts` 和 `SkillsSettingsModal.tsx` 的数据都来自后端命令。

> **路径必须与模块 Source of Truth 一致**：kimi 曾误写 `~/.kimi/*`，实际是 `~/.kimi-code/*`。
>
> **强制复制**：不支持 symlink 的 CLI 要在 `builtin_tool_forces_skill_copy()`（`builtin.rs:436`）加 key（当前只有 `cursor` / `antigravity_cli`）。

### 7.2 但路径解析有 6 处独立于 `BUILTIN_TOOLS` 的 match

这些是真正的「漏改静默失效」区，文件 `tauri/src/coding/tools/detection.rs`：

| 函数 | 行号 | 说明 |
|------|------|------|
| `resolve_special_mcp_config_path` | `:48` | OS 特殊路径（opencode / github_copilot_intellij / claude_desktop / hermes / dsh） |
| `is_tool_installed` 的 special 列表 | `:94` | 同上 5 个 key |
| `resolve_mcp_config_path_with_db` | `:163` | **DB 优先解析**：白名单 match |
| `resolve_mcp_config_path_with_db_async` | `:188` | 同上 |
| `resolve_skills_path_with_db` | `:211` | 同上 |
| `resolve_skills_path_with_db_async` | `:230` | 同上 |

配套的 backend resolver 在 `runtime_location.rs` 的 `get_tool_mcp_config_path_sync/_async`（`:2250` / `:2281`）和 `get_tool_skills_path_*`（`:1960` / `:2082`）。

> ⚠️ **已知未修 bug（zcode 至今未注册）**：`detection.rs` 的 4 个 `*_with_db*` 是**白名单 match**，zcode 不在其中，`_ =>` 走静态路径。后果：**用户在「自定义配置目录」里改了 root_dir 后，MCP 页读写的仍是默认路径，而不是用户自定义路径——无报错、无日志。**
>
> zcode 能「看起来正常」只是因为 `runtime_location.rs` 有 arm，但 `detection.rs` 根本不调用它。**新增 CLI 时必须同时改这两处**，否则自定义目录静默失效。

### 7.3 前端图标（MCP 与 Skills 共用）

`web/features/coding/shared/toolIcon/ToolIcon.tsx` 有 5 级解析顺序（见 `skills/AGENTS.md:76`）：

| 优先级 | 位置 | 形态 |
|--------|------|------|
| 0 | `:191` | `claude_code` / `pi` / `oh_my_pi` / `openclaw` 硬编码分支 |
| 1 | `:122` `RAW_SVG_MARKS` | `?raw` 内联 SVG |
| 2 | `:140` `PNG_ICON_URLS` | PNG `<img>` |
| 3 | `:84` `TOOL_ICON_RENDERERS` | LobeHub 组件（zcode: `Zhipu.Color`） |
| 4 | `:232` | 自定义工具的 `iconUrl` |
| 5 | `:257` | 两字母兜底徽标 |

**漏加 = 静默降级成兜底徽标，不报错。**

### 7.4 MCP 显示名覆盖

`tauri/src/coding/mcp/mod.rs:22` 的 `mcp_tool_display_name()` 只在需要改名时动（当前只有 `github_copilot`）。

---

## 8. 阶段八：备份 / 恢复

### 8.1 管线（三渠道共用）

```
generate.rs::generate_backup_file
  └─ utils.rs::create_backup_zip
       └─ utils.rs::write_backup_zip_contents
            └─ utils.rs::write_external_configs_to_backup_zip   ← 打包：改这里

restore.rs::restore_from_archive                                 ← 恢复：改这里
  ↑ local.rs / webdav.rs / repository.rs 全部只调用它
```

**重要**：`local.rs` / `webdav.rs` / `repository.rs` **没有** per-tool 的 restore 分支，三渠道只做薄包装。新增 CLI **不需要**动这三个文件。

### 8.2 必改的代码位置

#### A. 分类清单（决定「开关关闭时是否仍进包/恢复」）

| 位置 | 符号 | 说明 |
|------|------|------|
| `utils.rs:1477` | `const ALWAYS_BACKUP_CLI_TOOLS` | 运行时文件为真源、不可 re-apply → 开关关闭也进包 |
| `utils.rs:1487` | `const OPTIONAL_BACKUP_CLI_TOOLS` | SQLite 为真源、可 re-apply → 受 `backup_cli_config_files_enabled` 门控 |
| `utils.rs:1498` / `:1502` | `is_always_backup_cli_tool()` / `is_optional_backup_cli_tool()` | |
| `utils.rs:1593` | `wsl_module_for_external_config_tool()` match | **漏 arm → 恢复出的文件永远不会被 post-restore WSL sync 传播**（拿不到 module 就静默 return） |
| `utils.rs:1578` | `record_restored_external_config_wsl_module()` | 依赖上面那个 match |

#### B. 打包写入（`write_external_configs_to_backup_zip`，`utils.rs:3004`）

每个工具一段，标准形态是「写 `external-configs/<tool>/` 目录 entry + 写 `root-dir.txt` + 逐个文件调 `add_external_config_file_to_zip`」。

配套 helper：
- `utils.rs:2911` `add_external_config_file_to_zip(...)`
- `add_external_config_directory_contents_to_zip(...)`（目录递归）
- `utils.rs:390` `harden_restored_sensitive_file()`（敏感文件 0600）
- 每个工具的 `get_<tool>_*_path_from_db()`
- **无 DB 的 restore 期 fallback**：`utils.rs:425` → 调 `runtime_location::resolve_<tool>_root_dir_without_db()`

> ⚠️ `zip::ZipWriter` 不允许重复 entry。目录 entry 必须走 `add_directory_to_zip_once`（带 `added_zip_directories: &mut HashSet<String>` 幂等），否则自定义根目录 + 配置文件同时存在时报 `Duplicate filename`。

#### C. 恢复解压（`restore.rs`）

`restore.rs` 里是一长串 `else if file_name.starts_with("external-configs/<tool>/")`，**当前 14 个分支**。新增工具要加三处：

1. **root-dir override 读取**（`restore.rs:316` 的写法）
2. **解析 restore dir**（`restore.rs:408`）
3. **解压分支**（`restore.rs:776` 是 zcode 范本）。分支内必须：
   - 跳过空/目录/`root-dir.txt`（**漏了会在 CLI 数据根目录留下垃圾文件**）
   - `should_filter_external_config_entry(&filter_rules, "<tool>", relative_path)`
   - `resolve_external_config_restore_output_path(&<tool>_restore_dir, restore_relative_path)?`（**安全 helper，禁止直接 `join`**）
   - `record_restored_external_config_wsl_module(&mut restored_wsl_modules, "<tool>")`

#### D. 其余备份相关清单

| 位置 | 符号 | 漏改后果 |
|------|------|---------|
| `utils.rs:1035` | `backup_filter_option_path()` match | 过滤规则下拉显示裸相对路径而非 `~/.<tool>/...` |
| `utils.rs:1112` | `list_backup_file_filter_path_options()` | 「文件过滤规则」里没有该工具的任何可选项 |
| `utils.rs:2828` | `normalize_backup_filter_rule_path()` 的 `tool_prefixes` match | 用户写的 `~/.<tool>/AGENTS.md` **归一化不到** `AGENTS.md`，规则静默不匹配（**安全影响**） |
| `utils.rs:1876` | `get_custom_root_dir_path_info()` match | `_ => None` → 归档里不写 `root-dir.txt`，自定义根目录不被备份 |
| `utils.rs:1637` | `clear_restored_cli_custom_roots()` | `skip_cli_custom_roots=true` 时旧机器路径残留 |
| `restore.rs:1136` | `write_post_restore_flags(...)` | 恢复后不触发 WSL resync |
| `db/schema.rs` / `db/migrations.rs` | `DbTable` 枚举 + 建表 | **编译器会抓**（exhaustive match），非静默 |
| `BackupSettingsModal.tsx:72` | `TOOL_ORDER` | 该工具不出现在备份设置的排序里 |
| i18n ×2 | `settings.backupSettings.cliConfigFilesDesc` | **文案里点名工具列表**，中英各一份 |

### 8.3 备份侧的静默失效陷阱

1. **`should_skip_external_config_on_restore()` 的兜底是 `_ => true`（跳过）**。新增工具若**只写了打包段**、忘了登记进 `ALWAYS_`/`OPTIONAL_BACKUP_CLI_TOOLS`，那么在开关关闭的机器上恢复时，它的文件被**静默跳过**，恢复结果 `success=true`、0 warning。
2. **`wsl_module_for_external_config_tool` 漏 arm**：恢复出的文件永远不会被 post-restore WSL sync 传播。无日志。
3. **`restore.rs` 漏分支**：`else if` 链走到底什么都不做，entry 被丢弃，恢复「成功」。
4. **打包段漏写但没漏 `root-dir.txt`**：`root-dir.txt` 被当普通 entry 解压到 `<restore_dir>/root-dir.txt`，留下垃圾文件。
5. **归档路径扁平化**：zcode 曾把 `v2/provider_config.json` 存成 `provider_config.json`，恢复后文件落在 CLI **不读取**的位置（commit `312529b7` 修复，回归测试 `utils.rs:4560`）。**归档 entry 的相对路径必须保留 data-root 下的子目录层级。**
6. **`tool_prefixes` 漏项**：过滤规则「存在即生效」，归一化失败 = 用户以为排除了敏感文件，实际照打进去。

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

### 12.1 构建与测试

- [ ] `pnpm exec tsc --noEmit` + `pnpm i18n:check` + `pnpm test:web` 全绿
- [ ] `cargo check`（或 `cargo test`）通过
- [ ] `pnpm build` 成功
- [ ] 至少一条「表单提交 → 持久化 → 再读取」的往返用例
- [ ] 3.4 的全局 grep 兜底通过

### 12.2 端到端行为（逐条对照前面各节）

- [ ] **本机**：页面能加载配置、增删改供应商、应用默认、模型 CRUD
- [ ] **WSL Direct**：路径解析正确、CLI 调用走 `wsl -d <distro> --exec`（6.1）
- [ ] **托盘**：新 CLI 的供应商/提示词子菜单**真实出现**且点击生效；中英两种语言标题都不为空（5.2 第 3 点）
- [ ] **WSL/SSH 同步**：新映射在**老库**上也生效（`CURRENT_DEFAULTS_VERSION` 已 bump，6.2）；同步结果消息显示本地化名而非裸 id
- [ ] **MCP 页 / Skills 页**：新工具出现、图标正确、读写路径在自定义根目录下也对（7.2 的白名单）
- [ ] **备份/恢复**：打包 → 换机恢复 → 文件落在 CLI 能读到的位置；恢复后 WSL 自动 resync（8.3）
- [ ] 主窗口保存、托盘刷新、WSL 设置页状态三者一致
- [ ] 新 tab 的侧栏开关重启后保持
- [ ] 新 tab 的配置文件在 WSL/SSH 同步中**不被静默跳过**（`TAB_TO_MODULE` / `ALL_CODING_MODULES`）

### 12.3 视觉核对（**必做，不可用「编译通过」替代**）

**编译只保证类型正确，不保证形态正确。** 逐项在界面上指认，对照 §4.0.1 指定的参照 CLI：

- [ ] 供应商弹窗：标签在左（`layout="horizontal"`）、第一行是渠道、API Key 有显示/隐藏按钮
- [ ] 供应商弹窗：**没有**该 CLI 用不上的分区（非网关 CLI 不应出现计费/请求头/改写，见 4.0.3）
- [ ] 供应商卡片：详情**一行**、应用按钮是蓝色文字（`type="link"`）
- [ ] 模型列表工具栏：按钮**数量与参照一致**（逐个指着数，见 4.0.2-F）
- [ ] 空态 / 搜索空态 / 加载态文案与参照一致
- [ ] 失败路径有可见反馈，不是静默空列表（见 13.1 模式三）

> 若本次无法运行界面，**必须在提交说明里写明「未做视觉核对」**。ZCode 这一轮 8 次返工全部出在这一步——每一处都是「编译通过但形态不对」。

### 12.4 排查前先确认「跑的是当前构建」

UI 表现异常时，**先排除进程 stale**，再怀疑代码：

- [ ] 进程启动时间 vs `ai-toolbox.exe` 编译时间（进程更早 = 跑的是旧二进制）
- [ ] `PRAGMA user_version` vs `TARGET_SCHEMA_VERSION`（不相等 = 迁移没跑）

> 曾出现：前端 Vite 热更新到最新代码，后端进程却是 14 小时前启动的，DB 停在旧 schema。表现为「后端命令报错」，实际代码根本没生效，排查绕了很远。

### 12.5 建议加回归测试的位置

| 场景 | 参考 |
|------|------|
| 归档路径保留子目录层级 | `settings/backup/utils.rs:4560`（zcode 回归测试） |
| 业务 id 含冒号的往返读取 | `zcode/adapter.rs` 的 `managed_provider_id_survives_the_db_round_trip`（见 2.1） |
| 自定义根目录下 MCP/Skills 路径解析 | 需新写（当前 zcode 缺这个测试，所以 bug 没被发现） |
| 老库 backfill 默认映射 | `wsl/commands.rs` 的 versioned mapping 测试 |

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
| 13 | zcode：`detection.rs` 4 个 `*_with_db*` 白名单漏注册 | 自定义根目录下 MCP/Skills 页读写默认路径（静默） | **未修**，见 7.2 |
| 14 | zcode：备份 `root-dir.txt` 被当普通 entry 解压 | CLI 数据根目录留垃圾文件 | 分支内显式跳过 |
| 15 | zcode：`wsl_module_for_external_config_tool` 漏 arm | 恢复出的文件不被 post-restore WSL sync 传播 | 补 arm |
| 16 | zcode：`format_configs.rs` 无专用格式 | `timeoutMs` 写成通用 `timeout`，CLI 静默丢值 | 加 `ZCODE_FORMAT` |
| 17 | 共享组件：`omoNative.model.*` / `hermes.model.thinkingLevelHint` 整组键不存在 | 页面渲染字面键名 | 改为 `common.model.*` |
| 18 | 共享组件：`ModelFormModal` 覆盖值存 i18n key | `i18n:prune` 把 key 判为未使用并删除 | 覆盖值存 `t(key)` 的结果（已是终态文本） |
| 19 | zcode：迁移共享组件时保留 `layout="vertical"` | 标签在输入框上方，与其余 5 个 CLI 全不同 | 改 `horizontal` + `labelCol`/`wrapperCol`（见 4.0.2-A） |
| 20 | zcode：API Key 用朴素 `Input.Password` | 缺显示/隐藏按钮，4/5 的 CLI 都有 | 改 `masked input + addonAfter`（见 4.0.2-C） |
| 21 | zcode：渠道字段排在第 3、4 位且分两行 | 用户第一眼看到「名称」而非「选渠道」 | 提为第一行，左渠道 + 右格式（见 4.0.2-B） |
| 22 | zcode：用 `allowClear` + placeholder 表达「不选」 | 用户看不出「留空 = 自定义」 | 加显式「自定义」选项（见 4.0.2-D） |
| 23 | zcode：给非网关 CLI 加计费/请求头/改写分区 | 填了没有代码读，且写进 `meta` 造成"已生效"假象 | `show*=false` + 不合并 meta（见 4.0.3） |
| 24 | zcode：卡片详情渲染三行（含 provider id） | 与 Codex 的一行形态不符；id 是名称的 slug 副本 | 合并为一行并去掉 id（见 4.0.2-E） |
| 25 | zcode：卡片的「应用」用 default 按钮 | 其他 CLI 全是 `type="link"` | 改 `type="link"` + `CheckOutlined`（见 4.0.2-E） |
| 26 | zcode：`ModelListSection` 只传 1 个 handler | 工具栏只有「添加模型」，看起来像设计如此 | 补齐 test / fetch / batchDelete（见 4.0.2-F） |
| 27 | zcode：`db_clean_id` 剥离业务 ID 的 `custom:` 前缀 | 写库成功但读回 ID 被篡改，后续操作全报 not found | adapter 读原始 id + 回归测试（见 2.1） |
| 28 | zcode：模板加载 `.catch()` 只 `console.error` | 后端调用失败表现为「暂无数据」，排查绕远路 | 改成可见错误提示 |

### 13.1 静默失效的三种模式（归纳）

上表 28 条坑可以归成三类，识别出模式就能提前防：

**模式一：白名单 / 映射表漏项。** 用一个手工维护的列表去 gate 行为，新增实体时漏改一处 → 该实体永久静默失效。
> 例：#2 `TAB_TO_MODULE`、#13 `detection.rs` 白名单、#15 `wsl_module_for_reapply_label`、#5 kimi Gateway 注册。
>
> **对策**：优先让共享 resolver 自己处理未知 key（返回 `None` 后走兜底），而不是在调用侧维护副本列表。见 7.2 的结构性修法。

**模式二：可选 prop 决定渲染，漏传看起来像设计如此。** 组件按「传了才渲染」组织 UI，漏传一个 handler → 少一个按钮/分区，且**没有任何报错**。
> 例：#26 `ModelListSection` 工具栏、#23 `ProviderFormSections` 分区。
>
> **对策**：迁移时**枚举组件的全部可选 prop**，逐个决定传/不传并记录理由（4.0.2-F）。

**模式三：错误被吞掉。** `catch` 只打 console，UI 呈现为正常空态。
> 例：#28 模板加载失败显示「暂无数据」。
>
> **对策**：`catch` 必须给用户可见反馈，除非该失败确实无需用户知晓（此时要写明理由）。

**反向模式（不是坑但容易误判）：进程 stales。**
> 排查 UI 异常前，**先确认运行中的进程是当前构建**。曾出现：前端 Vite 热更新到最新代码，而后端进程是 14 小时前启动的旧二进制，DB 迁移也没跑 → 表现为「后端命令不存在/报错」，实际是代码根本没生效。
>
> **快速核对**：进程启动时间 vs `ai-toolbox.exe` 编译时间；`PRAGMA user_version` vs `TARGET_SCHEMA_VERSION`。

---

## 附录 A：ZCode 集成改动面（实证样本）

一个「最小完整集成」实际触达的文件分布（10 提交 / 63 文件）：

- **后端 30 个**：`tauri/src/coding/zcode/`（9 个模块文件）+ `runtime_location.rs` + `reapply_applied_runtime.rs` + `session_manager/` + `tools/builtin.rs` + `settings/{types,adapter}.rs` + `settings/backup/{utils,restore}.rs` + `tray.rs` + `lib.rs` + `db/{schema,migrations}.rs` + `wsl/`+`ssh/`（mcp_sync + commands）+ `mcp/`（3 个）
- **前端 33 个**：`web/features/coding/zcode/`（7 个）+ `web/services/`（3 个）+ `web/types/` + `constants/modules.tsx` + `app/routeConfig.ts` + `MainLayout` + `toolIcon` + `features/settings/`（7 个）+ i18n（2 个）

> ZCode **未做**：Gateway 接管、`CliManualPathSetting`（页面有「更多选项」但只是配置弹窗；ZCode 模块本身不 spawn CLI，故无 cli_resolver 需求）、`docs/plan` 文档。这些是可选阶段——反过来说，**如果新工具需要调用 CLI**，cli_resolver 与手动路径入口就是必需项。

### A.1 共享组件对照表

**参照 CLI 是 Codex，不是 ZCode**（见 4.0.1）。下表只说明「哪个关注点该用哪个共享组件」，具体形态必须按 §4.0.2 的清单对着 Codex 逐项核对。

| 关注点 | 用共享组件 | 备注 |
|--------|-----------|------|
| 页面头部 | `CodingPageHeader` | 无文案 props；所有标签走 `common.*` |
| 供应商列表 | `ProviderListSection` | `emptyTextHint` 是唯一的空态扩展点 |
| 供应商卡片 | 模块内卡片（参照 `CodexProviderCard`） | 字段区差异大，不强行用通用 `ProviderCard` |
| 模型列表 | `ModelListSection` | **枚举全部可选 prop 逐个决定传/不传**（4.0.2-F） |
| 供应商表单 | `ProviderFormSections` | 计费 / 自定义头 / 模型重写三块**只对网关 CLI 显示**（4.0.3） |
| 模型表单 | `ModelFormModal` | 差异走 `messageOverrides`（值是**已翻译文本**，不是 key） |
| 通用配置编辑 | `JsonEditor` | `onChange(parsed, isValid)` + `onRawChange(raw)` 配合使用 |

**「支持自定义模型」形态的供应商表单不含模型编辑**：模型在卡片上增删改，保存时要把已存模型原样带回去，否则会清空模型目录：

```tsx
// Models are edited on the provider card, not here. Reuse whatever the
// stored provider already has so saving the form does not wipe the catalog.
const existingModels = provider
  ? (parseZcodeProviderSettings(provider.settingsConfig)?.models ?? [])
  : [];
```

**ZCode 有三个配置文件**（新 CLI 若也是多文件，照此分工）：

| 文件 | 内容 | 管理方式 |
|------|------|---------|
| `v2/provider_config.json` | 供应商 + 模型目录 | 结构化 UI（provider/模型 CRUD） |
| `cli/config.json` | MCP 服务器 / hooks / plugins / 权限 | `JsonEditor` 原文编辑（`ZcodeCommonConfigModal`） |
| `v2/setting.json` | 桌面端偏好 | 只读 |

> `cli/config.json` 里含明文凭据（Tavily / Firecrawl / GitHub token / Context7 key）。做演示或截图时注意遮挡。

### A.2 ZCode 的已知遗留

| 项 | 状态 |
|----|------|
| `detection.rs` 4 个 `*_with_db*` 白名单漏 zcode | 未修，见 7.2 |
| ZCode 不在 `GatewayCliKey::supported_mvp()` | 设计如此（模块不 spawn CLI），故无 Gateway 接管与 cli_resolver |

---

## 何时更新本文件

- 新增 CLI 工具完成后，把新踩的坑补进第 13 节；若是**新类型**的坑，补进 13.1 的模式归纳
- 发现清单项过时或新增了硬编码清单，同步更新第 3 节并**同时**修正根 `AGENTS.md` 的对应章节
- 出现新的「编译通过但形态不对」返工，把漏掉的核对项补进 4.0.2
- 本文件与根 `AGENTS.md` 的「Tab / Page-Key Allowlist Rules」是配套关系：后者是规则，前者是流程
