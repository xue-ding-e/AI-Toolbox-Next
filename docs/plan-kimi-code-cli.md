# Kimi Code CLI 集成方案

> 评估日期：2026-07-15
> 基于 Kimi Code CLI v0.39.1（`@moonshot-ai/kimi-code`）
> 实现状态：**已落地**。阶段 1-3 全部完成；阶段 4 中 Gateway 接管、托盘接线、cli_resolver 在验收轮补齐，Billing/Usage 未实现。详见第 14 节 checkbox 与第 16 节偏差记录。

---

## 目录

1. [产品概述](#1-产品概述)
2. [集成范围与边界](#2-集成范围与边界)
3. [后端模块设计](#3-后端模块设计)
4. [前端页面设计](#4-前端页面设计)
5. [Gateway 代理集成](#5-gateway-代理集成)
6. [配置管理策略](#6-配置管理策略)
7. [官方 OAuth 登录](#7-官方-oauth-登录)
8. [会话管理](#8-会话管理)
9. [WSL/SSH 同步](#9-wslssh-同步)
10. [托盘集成](#10-托盘集成)
11. [Skills 同步](#11-skills-同步)
12. [Billing/Usage 额度展示](#12-billingusage-额度展示)
13. [插件市场](#13-插件市场)
14. [实施路线图](#14-实施路线图)
15. [风险与缓解](#15-风险与缓解)

---

## 1. 产品概述

### 1.1 基本信息

| 项目 | 内容 |
|------|------|
| **CLI 名称** | Kimi Code CLI |
| **npm 包** | `@moonshot-ai/kimi-code` |
| **CLI 命令** | `kimi` |
| **运行时** | Node.js ≥ 22.19.0 |
| **安装方式** | 官方脚本 / `npm install -g @moonshot-ai/kimi-code` |
| **配置格式** | TOML（`~/.kimi-code/config.toml`） |
| **数据根目录** | `~/.kimi-code/`（可被 `KIMI_CODE_HOME` 覆盖） |
| **许可证** | MIT |
| **GitHub** | `MoonshotAI/kimi-code` |

### 1.2 产品定位

Kimi Code CLI 是月之暗面（Moonshot AI）推出的终端 AI coding agent，与 Claude Code、Codex、OpenCode 属于同类产品。它支持：

- 多 provider 接入（Kimi 官方、Anthropic、OpenAI、Google Gemini、Vertex AI）
- 官方 OAuth 登录 + 自定义 API Key
- Skills（Markdown + YAML frontmatter，与 Claude Code 兼容）
- MCP Server 集成
- 插件市场（官方/精选/自定义）
- 子 Agent（Sub-agent）
- 会话管理（恢复/浏览/导出）
- 非交互模式（`-p`）

### 1.3 集成模式

Kimi Code CLI 属于"根目录模块"（与 Claude Code、Codex、Grok CLI、Gemini CLI 同类），即：

- 保存的是**配置根目录**（默认 `~/.kimi-code/`）
- 后续在该目录下派生 `config.toml`、`AGENTS.md`、`skills/`、`plugins/`、`sessions/` 等路径
- 环境变量 `KIMI_CODE_HOME` 可覆盖根目录

---

## 2. 集成范围与边界

### 2.1 本方案包含

| 模块 | 优先级 | 说明 |
|------|--------|------|
| 后端模块框架 | P0 | `tauri/src/coding/kimi/` 完整模块 |
| Provider 配置管理 | P0 | 多 provider 的 TOML 读写、应用、切换 |
| 配置页面 | P0 | 前端 Tab 页面，类似 Grok/Gemini CLI |
| 官方 OAuth 登录 | P0 | Device Code 登录流程 |
| 运行时路径解析 | P0 | `runtime_location` 注册 |
| 托盘集成 | P1 | Provider/Model/Prompt 切换 |
| 会话管理 | P1 | 浏览/恢复/导出 |
| WSL/SSH 同步 | P1 | 配置文件同步到远端 |
| Skills 同步 | P1 | 中央仓库 → Kimi skills 目录 |
| Gateway 代理 | P1 | CLI 网关接管 |
| Billing/Usage 展示 | P2 | 额度查询 |
| 插件市场展示 | P2 | 已安装插件列表 |

### 2.2 本方案不包含

- **Kimi 插件市场管理**：Kimi 有完整的 `/plugins` 命令管理插件，AI Toolbox 只读展示已安装列表
- **Kimi Web UI 嵌入**：`kimi web` 启动的是独立 web 服务，不嵌入 AI Toolbox 窗口
- **Kimi VS Code 扩展集成**：仅限 CLI 模式
- **Kimi Computer Use / WebBridge 管理**：属于插件层面，不单独管理

---

## 3. 后端模块设计

### 3.1 目录结构

```
tauri/src/coding/kimi/
├── mod.rs              # 公开子模块
├── commands.rs         # Tauri commands + apply_config_internal
├── types.rs            # 类型定义
├── adapter.rs          # DB adapter（SQLite JSONB ↔ Rust struct）
├── constants.rs        # 常量
├── official_accounts.rs # 官方 OAuth 登录
└── tray_support.rs     # 托盘集成
```

### 3.2 模块注册与白名单（Allowlist）同步

根据项目 `AGENTS.md` 的 **Tab / Page-Key Allowlist 铁律**，新增 `kimi` 模块必须在后端和前端的所有相关列表中全量注册，防止出现静默过滤、侧边栏折叠状态丢失或 WSL/SSH 同步遗漏：

#### 后端注册点
- `tauri/src/coding/mod.rs` — 添加 `pub mod kimi;`
- `tauri/src/coding/runtime_location.rs` — `MODULE_KEYS` 常量中添加 `"kimi"`
- `tauri/src/coding/reapply_applied_runtime.rs` — `ALL_WSL_FILE_MODULES` 中添加 `"kimi"`
- `tauri/src/settings/types.rs` — `AppSettings::default().visible_tabs` 和 `default_sidebar_hidden_by_page` 中添加 `"kimi"`
- `tauri/src/settings/adapter.rs` — `CURRENT_DEFAULT_VISIBLE_TABS` 和 `CURRENT_DEFAULT_TAB_SET` 中添加 `"kimi"`，并同步更新 `visible_tabs` 的迁移测试断言
- `tauri/src/settings/backup/utils.rs` — 添加到 `ALWAYS_BACKUP_CLI_TOOLS`（若为默认包含）或 `OPTIONAL_BACKUP_CLI_TOOLS`
- `tauri/src/tray.rs` — 注册 Kimi 的 tray section builder
- `tauri/src/coding/config_cleanup.rs` — 添加 Kimi 的受管配置清理规则

#### 前端注册点
- `web/services/settingsApi.ts` — `SIDEBAR_PAGE_KEYS` 添加 `"kimi"`，`defaultSettings.visible_tabs` 添加 `"kimi"`
- `web/features/settings/hooks/useWSLSync.ts` / `useSSHSync.ts` — `TAB_TO_MODULE`、`MODULE_TO_TAB`、`ALL_CODING_MODULES`、`ALL_MODULE_KEYS` 映射中添加 `"kimi"`
- `web/features/settings/components/WSLSyncModal.tsx` / `SSHSyncModal.tsx` — 同步弹窗的模块筛选清单中包含 `"kimi"`
- `web/features/settings/components/FileMappingModal.tsx` / `SSHFileMappingModal.tsx` — 模块选择下拉菜单中补充 `"kimi"`

### 3.3 类型定义

```rust
// types.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KimiProviderRecord {
    pub id: String,
    pub name: String,
    pub category: String,
    pub settings_config: String,  // JSON serialized provider config
    pub source_provider_id: Option<String>,
    pub website_url: Option<String>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub icon_color: Option<String>,
    pub sort_index: Option<i32>,
    pub meta: Option<Value>,
    pub is_applied: bool,
    pub is_disabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KimiProvider {
    pub id: String,
    pub name: String,
    pub category: String,
    pub settings_config: String,
    pub source_provider_id: Option<String>,
    pub website_url: Option<String>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub icon_color: Option<String>,
    pub sort_index: Option<i32>,
    pub meta: Option<Value>,
    pub is_applied: bool,
    pub is_disabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KimiOfficialAccount {
    pub id: String,
    pub account_name: String,
    pub user_id: Option<String>,
    pub token_expires_at: Option<i64>,
    pub last_refreshed_at: Option<i64>,
    pub is_applied: bool,
    pub plan_type: Option<String>,
    pub last_limits_fetched_at: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KimiUsageInfo {
    pub plan_type: Option<String>,
    pub weekly_remaining: Option<String>,
    pub weekly_reset_at: Option<i64>,
    pub monthly_remaining: Option<String>,
    pub monthly_reset_at: Option<i64>,
    pub extra_usage_balance: Option<String>,
}
```

### 3.4 常量定义

```rust
// constants.rs
pub const KIMI_HOME_ENV_KEY: &str = "KIMI_CODE_HOME";
pub const KIMI_LOCAL_PROVIDER_ID: &str = "__local__";
pub const KIMI_CONFIG_FILE: &str = "config.toml";
pub const KIMI_AUTH_FILE: &str = "auth.json";  // Not used for API Key, only OAuth
pub const KIMI_PROMPT_FILE: &str = "AGENTS.md";
pub const KIMI_SKILLS_DIR: &str = "skills";
pub const KIMI_PLUGINS_DIR: &str = "plugins";
pub const KIMI_SESSIONS_DIR: &str = "sessions";
pub const KIMI_CREDENTIALS_DIR: &str = "credentials";
pub const KIMI_TUI_CONFIG_FILE: &str = "tui.toml";
pub const KIMI_MODEL_ALIAS_SEPARATOR: &str = "/";
pub const KIMI_DEFAULT_BASE_URL: &str = "https://api.kimi.com/coding/v1";
pub const KIMI_MOONSHOT_BASE_URL: &str = "https://api.moonshot.ai/v1";

// Managed env keys in config.toml [providers.<name>.env]
pub const MANAGED_ENV_KEYS: [&str; 4] = [
    "KIMI_API_KEY",
    "KIMI_BASE_URL",
    "KIMI_CODE_BASE_URL",
    "KIMI_CODE_CUSTOM_HEADERS",
];

// Provider types supported by Kimi Code CLI
pub const KIMI_SUPPORTED_PROVIDER_TYPES: [&str; 6] = [
    "kimi",
    "anthropic",
    "openai",
    "openai_responses",
    "google-genai",
    "vertexai",
];
```

### 3.5 Config.toml 结构（Kimi 视角）

Kimi Code CLI 的 `config.toml` 结构：

```toml
# 顶层字段
default_model = "kimi-code/k3"
default_permission_mode = "manual"
default_plan_mode = false
merge_all_available_skills = true
telemetry = true

# Provider 定义
[providers."managed:kimi-code"]
type = "kimi"
base_url = "https://api.kimi.com/coding/v1"
api_key = ""

# 自定义 Provider
[providers.anthropic]
type = "anthropic"
api_key = "sk-ant-xxx"

[providers.openai]
type = "openai"
base_url = "https://api.openai.com/v1"
api_key = "sk-xxx"

# 模型定义
[models."kimi-code/k3"]
provider = "managed:kimi-code"
model = "k3"
max_context_size = 1048576
capabilities = ["thinking", "always_thinking", "image_in", "video_in", "tool_use"]
display_name = "K3"
support_efforts = ["low", "high", "max"]
default_effort = "max"

[models."kimi-code/kimi-for-coding"]
provider = "managed:kimi-code"
model = "kimi-for-coding"
max_context_size = 262144
capabilities = ["thinking", "always_thinking", "image_in", "video_in", "tool_use"]

# Thinking 配置
[thinking]
enabled = true
effort = "high"
keep = "all"

# 循环控制
[loop_control]
max_attempts_per_step = 10
reserved_context_size = 50000

# 后台任务
[background]
max_running_tasks = 4
keep_alive_on_exit = false

# 内置服务
[services.moonshot_search]
base_url = "https://api.kimi.com/coding/v1/search"
api_key = ""

[services.moonshot_fetch]
base_url = "https://api.kimi.com/coding/v1/fetch"
api_key = ""

# 权限规则
[[permission.rules]]
decision = "allow"
pattern = "Read"

[[permission.rules]]
decision = "deny"
pattern = "Bash(rm -rf*)"

# Hooks
[[hooks]]
event = "PreToolUse"
matcher = "Bash"
command = "node ~/.kimi-code/hooks/check-bash.mjs"
timeout = 5
```

### 3.6 配置管理核心规则

对齐 AI Toolbox 现有规则（参考 Grok 和 Codex）：

1. **AST 级 TOML 解析（`toml_edit`）**：配置文件读写必须使用 `toml_edit::DocumentMut` 进行 AST 级操作，严禁反序列化为有限的 Rust struct 后整文件重新序列化。这确保非受管配置（如 `hooks`、`permission`、`services`、`thinking`、`loop_control`）和原有注释、排版格式得到绝对保留。
2. **Provider 只管理自有字段**：`[providers.<name>]` 中只管理 AI Toolbox 写入的受管字段，保留用户自定义字段和未知字段。
3. **凭证优先级**：`api_key` 直接字段 > `[providers.<name>.env]` 子表 > 若都缺失则启动失败。
4. **OAuth 与 API Key 分离**：OAuth 凭证走 `credentials/` 目录（权限 0700/0600），API Key 走 `config.toml` 中的 `api_key` 字段。
5. **模型映射**：`[models."<alias>"]` 中的 `provider` 引用 `[providers.<name>]` 中的 key。
6. **切换 Provider 前置快照清理**：切换 Provider 时，必须先从数据库捕获上一条 Provider record 快照，精准清除旧投影的 `[models.*]` 与 `[providers.*]`，再写入新渠道投影。
7. **未知字段与节点保护**：`config.toml` 中 AI Toolbox 不管理的字段（如 `hooks`、`permission`、`services`、`extra_skill_dirs`）必须节点级保留。

---

## 4. 前端页面设计

### 4.1 设计系统与规范约束

前端页面开发必须严格遵循项目全局设计系统与工程规范：
1. **视觉规范（`DESIGN.md`）**：严格遵循 `DESIGN.md` 中的调性、布局密度、圆角、阴影和层级。所有颜色、边框和状态样式必须使用 `web/App.css` 的 CSS 变量或 Ant Design 6 design token，严禁硬编码颜色值（如 `#fff`、`#1890ff` 等）。
2. **主题适配**：完整适配 Light、Dark 和 System Theme，使用 CSS Modules（`.module.less`），主题覆盖使用 `[data-theme="dark"]` 选择器。
3. **国际化（i18n）**：所有文案必须支持 `zh-CN` 和 `en-US`。多语言 key 的添加必须使用 `pnpm i18n:set-key <key> --zh-CN "..." --en-US "..." --write`，严禁直接手改 `locales/*.json`。

### 4.2 页面结构

```
web/features/coding/kimi/
├── index.ts              # 公开导出
├── pages/
│   └── KimiPage.tsx      # 主页面（类似 Grok/Gemini CLI）
├── components/
│   ├── KimiHeader.tsx    # 顶部路径行（source tag + 路径显示）
│   ├── KimiProviderSection.tsx   # Provider 配置区
│   ├── KimiModelSection.tsx      # 模型选择区
│   ├── KimiPromptSection.tsx     # Prompt 配置区
│   ├── KimiCommonConfigSection.tsx # 通用配置区
│   └── KimiOfficialAccountList.tsx # 官方账号列表
├── hooks/
│   └── useKimiConfig.ts  # 配置状态管理
├── services/
│   └── kimiApi.ts        # Tauri command wrappers
└── types/
    └── index.ts          # 前端类型定义
```

### 4.3 页面 Tab 布局

参考 Grok CLI 页面，包含以下 Tab：

| Tab | 内容 | 参考 |
|-----|------|------|
| **Provider** | 供应商列表（增删改、应用、排序） | 复用现有 ProviderSection |
| **Common Config** | 通用配置（`default_model`、`thinking`、`loop_control` 等） | 参考 Grok Common Config |
| **Prompt** | Kimi 的 `AGENTS.md` 配置 | 复用现有 PromptSection |
| **Official Account** | 官方 OAuth 账号管理（登录/登出/刷新额度） | 参考 Grok 官方账号 |

### 4.4 顶部路径行与弹窗

显示当前生效路径的 `source` 标签和完整路径，标准行为：

- tag 只反映 `source`（`custom`/`env`/`shell`/`default`），不单独显示 WSL tag
- 自定义路径回填到 `RootDirectoryModal` 的输入框
- 与 Claude Code/Codex 共用同一个 `RootDirectoryModal` 组件
- 弹窗严格遵循 `DESIGN.md` 的 Viewport-safe 规范，内部滚动，禁止破坏 `--ai-modal-viewport-*` 约束

### 4.5 路由注册

在 `web/features/coding/index.ts` 中注册 `kimi` 路由，key 为 `"kimi"`。

---

## 5. Gateway 代理集成

### 5.1 现有资源

`gateway_provider_profiles.json` 中已有完整的 Kimi/Moonshot 配置：

- **Provider ID**: `moonshot`（类别 `cn_official`）
- **Provider Type**: `kimi_coding`（别名 `kimi`、`moonshot`）
- **支持的 Target Protocol**: Anthropic、OpenAI Chat、OpenAI Responses、Anthropic Messages
- **API base**: `https://api.kimi.com/coding/v1`
- **模型**: `kimi-k2.7-code`、`kimi-for-coding`

### 5.2 新增工作

1. **`GatewayCliKey::Kimi`**：在 `proxy_gateway/types.rs` 中新增枚举值，`as_str() = "kimi"`
2. **`cli_proxy` 接管**：实现 `provider_switch_locked_by_manifest` 检查
3. **`reapply_applied_runtime.rs`**：添加 Kimi 的网关锁定检查和重新应用链路
4. **`commands.rs` 中的 `ensure_kimi_gateway_direct`**：网关接管时拒绝直连操作

### 5.3 协议兼容

Kimi 的 gateway 协议兼容已在 `gateway-provider-compatibility.md` 中记录：

- OpenAI Chat target：`json_schema` 降为 `json_object`，assistant tool call 缺 `reasoning_content` 时补 `"tool call"`
- Anthropic target：规范化 tool thinking 历史
- Codex → Chat reasoning：使用 `thinking` + `reasoning_content`
- `prompt_cache_key` 对 `kimi` / `moonshot` allowlist 可 reinject

---

## 6. 配置管理策略

### 6.1 数据流

```
┌──────────────┐     ┌──────────────────┐     ┌──────────────────┐     ┌───────────────┐
│  前端页面    │ ──► │  kimiApi.ts      │ ──► │  commands.rs     │ ──► │  SQLite JSONB │
│  (React)     │ ◄── │  (Service Layer)  │ ◄── │  (Tauri Command) │ ◄── │  (DB)         │
└──────────────┘     └──────────────────┘     └────────┬─────────┘     └───────────────┘
                                                       │
                                                       ▼
                                              ┌──────────────────┐
                                              │  config.toml     │
                                              │  (Runtime File)  │
                                              └──────────────────┘
```

### 6.2 Provider 管理与前置快照清理

**增删改 Provider**：

1. 前端操作 → Tauri command → 写入 SQLite JSONB
2. 用户点击"应用" → `apply_kimi_provider_to_file` → 写入 `config.toml`
3. 写入规则：
   - 新增 `[providers.<name>]` section
   - 新增 `[models."<alias>"]` 条目
   - 更新 `default_model` 指向新 model
   - 清理上一渠道的 `[model.*]` 条目

**切换 Provider（核心状态一致性约束）**：

根据 `AGENTS.md` 的 Optional Field & Compatibility Rules，切换 Provider 时若直接更新数据库后再读取，会导致上一渠道的配置快照丢失，从而无法在 `config.toml` 中精确定位并删除上一个 Provider 的投影。因此必须执行**前置快照清理**：
1. **读取前置快照**：在更新 SQLite 数据库记录之前，先读取当前生效的 Provider 记录快照（包含前一个 `name`、`settings_config` 中的 models 投影别名等）。
2. **清理旧投影**：使用 `toml_edit::DocumentMut` 将旧快照中的 `[providers."<old_name>"]` 与所有旧 `[models."<old_alias>"]` 从 TOML AST 中显式移除。
3. **写入新投影**：将新选中的 Provider 及 Models 插入 TOML AST。
4. **持久化与更新 DB**：将修改后的 TOML 文本原子写回文件，并在 SQLite 事务中完成 `is_applied` 状态的切换。

### 6.3 凭证管理

Kimi Code CLI 的凭证管理有特殊规则：

- **不自动读取 shell 环境变量**：`export KIMI_API_KEY=xxx` 不会生效，必须写在 `config.toml` 的 `[providers.<name>.env]` 子表中
- **OAuth 与 API Key 分离**：OAuth 走 `credentials/` 目录，API Key 走 `config.toml`
- **`KIMI_CODE_BASE_URL` vs `KIMI_BASE_URL`**：前者是 OAuth 托管服务（`api.kimi.com/coding/v1`），后者是 Moonshot API Key 直连（`api.moonshot.ai/v1`）

### 6.4 字段保留与 AST 编辑机制

在 `config.toml` 读写时，统一采用 `toml_edit` 库解析为 `DocumentMut`。在修改受管部分时，必须完整保留以下非受管字段及注释：

- `[thinking]` — 用户配置的 thinking 参数
- `[loop_control]` — 循环控制参数
- `[background]` — 后台任务参数
- `[services.*]` — 内置搜索/抓取服务配置
- `[[permission.rules]]` — 权限规则
- `[[hooks]]` — 生命周期钩子
- `extra_skill_dirs`、`extra_agent_dirs` — 自定义目录
- `[identity]` — 自定义 agent 身份
- `[tools]`、`[image]` — 工具/图片配置
- `[secondary_model]` — 子 agent 模型池

**AST 变更范式（参考 Codex 模块）**：
- 查找/修改：通过 `doc.get_mut("providers")` 定位 table 进行受管 key 的增删改。
- 格式保持：避免重建整个 Table 造成原有换行和注释丢失。

---

## 7. 官方 OAuth 登录

### 7.1 流程设计与网络客户端约束

**网络请求规范（重要）**：
- 后端所有 OAuth 网络请求（Device Code 请求、Token 轮询、Token 刷新等）必须使用统一的 `crate::http_client::client(&state).await?`。
- 严禁直接使用 `reqwest::Client::new()` 或默认构造器，必须基于全局配置并显式启用 `rustls` TLS 后端，避免 Windows 环境下因 Schannel 导致 `SEC_E_NO_CREDENTIALS` 崩溃，并确保正确遵循全局代理设置。

流程示意：
```
1. 用户点击"登录" → 后端通过 http_client 调用 Kimi OAuth 端点
2. 获取 Device Code + user_code + verification_uri
3. 返回前端展示（验证码 + 链接）
4. 用户打开链接、输入验证码、授权
5. 后端通过 http_client 轮询 token 端点直至授权完成
6. 写入 credentials/<name>.json（原子写入，权限 0600）
7. 更新 SQLite 中对应账号记录
8. 触发 config-changed 事件
```

### 7.2 OAuth 端点

| 用途 | 端点 | 说明 |
|------|------|------|
| Device Code 请求 | OAuth 服务端点 | 标准 RFC 8628 |
| Token 轮询 | OAuth 服务端点 | 轮询直到授权完成 |
| Token 刷新 | OAuth 服务端点 | 刷新 access token |
| 退出登录 | OAuth 服务端点 | 吊销 token |

OAuth host 默认值：`https://auth.kimi.com`（可被 `KIMI_CODE_OAUTH_HOST` / `KIMI_OAUTH_HOST` 覆盖）

### 7.3 Token 刷新策略与后台调度

参考 Codex 与 Gemini CLI 的 refresh 模式：

- **Lead 检查**：access token 剩余有效期 ≤ 30 分钟视为临期。
- **`ensure_fresh`**：临期才真正刷新，非临期跳过。
- **`force_refresh`**：强制刷新（不看 lead）。
- **后台巡检调度**：通过共享模块 `coding::auth_refresh` 调度（启动首次 + 15m 周期）。
- **巡检范围与写入权限边界**：
  - 巡检对象为所有已入库官方 OAuth 账号（含 `is_applied = false`）。
  - **未应用账号（`is_applied = false`）**：刷新后**仅将新 Token 与元数据写回 SQLite 数据库**，绝不触碰或覆写 live 运行时凭证文件。
  - **当前应用账号（`is_applied = true`）**：刷新后写回 SQLite，同时原子重写 live `credentials/<name>.json` 凭据文件，并触发 `config-changed`（`"window"`）事件与 WSL 同步通知。

### 7.4 账号管理

| 操作 | 行为 |
|------|------|
| 登录成功 | 入库官方账号（`is_applied=false`），不写 `credentials/` 目录 |
| 应用账号 | 写入 `credentials/<name>.json`，配置 `config.toml` 中 provider，标记 `is_applied=true` |
| 登出 | 吊销 token，清理 `credentials/<name>.json`，标记 `is_applied=false` |
| 刷新额度 | 调用 CLI 解析 `/usage` 输出，更新额度字段（带超时与降级保护） |

---

## 8. 会话管理

### 8.1 会话浏览

参考 `session_manager/grok.rs` 实现 `session_manager/kimi.rs`：

- Kimi 的会话数据位于 `$KIMI_CODE_HOME/sessions/<workDirKey>/<sessionId>/`
- 会话索引文件：`session_index.jsonl`（每行一个 `{sessionId, sessionDir, workDir}`）
- 会话元数据：`state.json`（包含 title、lastPrompt、created_at、updated_at）
- 会话恢复命令：`kimi -S <sessionId>` 或 `kimi -c`

### 8.2 会话恢复与导出命令派发

派发恢复或导出命令时，必须遵循 `cli_resolver` 与运行时判断规则：
- 解析可执行文件路径（优先使用用户手动指定的覆盖路径）。
- 如果为 `WslDirect`，命令构造为 `wsl -d <distro> --exec kimi -S <sessionId>` 并处理工作目录映射。

```rust
// 恢复命令格式
format!("kimi -S {}", session_id)
// 或继续最近会话
"kimi -c".to_string()
```

### 8.3 会话导出

```rust
// 导出命令
format!("kimi export {} -o {}", session_id, output_path)
```

---

## 9. WSL/SSH 同步

### 9.1 运行时路径解析

在 `runtime_location.rs` 中注册 Kimi 的路径解析：

- 优先级：应用内 `root_dir` > `KIMI_CODE_HOME` 环境变量 > shell 配置 > 默认 `~/.kimi-code/`
- WSL Direct 判定：如果路径解析为 `\\wsl.localhost\...` UNC 路径，标记为 WslDirect

### 9.2 同步文件集合

| 文件 | 说明 |
|------|------|
| `config.toml` | 主配置文件 |
| `AGENTS.md` | 全局 prompt 文件 |
| `mcp.json` | MCP 服务器声明 |
| `skills/` | Skills 目录 |
| `plugins/installed.json` | 已安装插件记录 |
| `tui.toml` | TUI 偏好配置 |

### 9.3 注册同步

在以下文件中注册 Kimi 的同步 key：

- `tauri/src/coding/runtime_location.rs` 的 `MODULE_KEYS`
- `tauri/src/coding/reapply_applied_runtime.rs` 的 `ALL_WSL_FILE_MODULES`
- `tauri/src/settings/backup/utils.rs` 的 `ALWAYS_BACKUP_CLI_TOOLS`
- 前端 `useWSLSync.ts` / `useSSHSync.ts` 的 `TAB_TO_MODULE` / `ALL_CODING_MODULES`

---

## 10. 托盘集成

### 10.1 托盘数据结构

```rust
// tray_support.rs
pub struct TrayProviderData {
    pub title: String,       // Section title
    pub items: Vec<TrayItem>, // Provider items with checkmark
}

pub struct TrayModelData {
    pub title: String,
    pub current_display: String, // Current model display name
    pub items: Vec<TrayItem>,
}

pub struct TrayPromptData {
    pub title: String,
    pub items: Vec<TrayItem>,
}
```

### 10.2 托盘菜单

参考 Grok 的托盘实现，包含三个 section：

1. **Provider 选择**：列出所有已配置的 provider，选中项打勾
2. **Model 选择**：列出当前 provider 下的模型列表
3. **Prompt 选择**：列出已保存的 prompt 配置

### 10.3 事件驱动

- 托盘操作 → `from_tray: true` → 写入配置 → 发出 `config-changed` 事件（payload `"tray"`）
- 主窗口操作 → `from_tray: false` → 写入配置 → 发出 `config-changed` 事件（payload `"window"`）
- 全局监听器刷新托盘菜单

---

## 11. Skills 同步

### 11.1 兼容性

Kimi Code CLI 的 Skills 格式与 Claude Code 高度兼容：

| 特性 | Kimi Code CLI | Claude Code |
|------|--------------|-------------|
| 文件格式 | `SKILL.md`（YAML frontmatter + Markdown body） | 相同 |
| 目录结构 | 目录形式 + 扁平 `.md` | 相同 |
| 用户级别路径 | `$KIMI_CODE_HOME/skills/` | `~/.claude/skills/` |
| 通用 Skills 路径 | `~/.agents/skills/` | 不直接支持 |
| 项目级别路径 | `.kimi-code/skills/` | `.claude/skills/` |

### 11.2 同步策略

- **源目录**：AI Toolbox 的 Skills 中央仓库（`central_repo_path`）
- **目标目录**：`$KIMI_CODE_HOME/skills/`（或 `~/.kimi-code/skills/`）
- **同步时机**：Skills 变更时自动同步到 Kimi 目标目录
- **特殊行为**：Kimi 也扫描 `~/.agents/skills/`，该目录是跨工具共享的，不应被 AI Toolbox 独占

### 11.3 安全规则

- 同步前检查 `source == target`、target 在 source 内、source 在 target 内，拒绝循环同步
- WSL Direct 下目标目录解析为 UNC 路径

---

## 12. Billing/Usage 额度展示

> **证伪与落地结论**：方案 A（通过 `kimi -p "/usage"` 解析）前提已证伪——0.38.0 本机实测非交互模式不拦截 `/usage`（当成普通 prompt 发给模型），0.39.1 源码确认 TUI `/usage` 仅输出会话 token 统计、无会员额度字段、无 billing API。因此额度功能已落地为外链方案（跳转 `https://www.kimi.com/code/console`），详见 §16.2。

### 12.1 现状

Kimi **没有公开的 billing REST API**。额度查询只能通过：

1. CLI 内置命令 `/usage`（在 TUI 或通过 `-p "/usage"` 中）
2. Kimi Code Console 网页
3. Kimi 主站会员页面

### 12.2 方案选择与安全调用规范

**方案 A（推荐）**：通过 `kimi -p "/usage"` 解析 CLI 输出

根据项目的 **CLI 调用与 WSL Direct 铁律**，严禁使用裸 `Command::new("kimi")`。必须遵循以下规范：
1. **CLI 路径解析**：通过 `crate::coding::cli_resolver::resolve_cli_path_async("kimi")` 获取可执行文件绝对路径，支持用户在"更多选项"中手动配置的覆盖路径，避免 macOS GUI 下因缺 PATH 启动失败。
2. **WSL Direct 适配**：调用 `runtime_location` 判断当前是否为 WSL 模式。若是 WSL，必须转换为 `wsl -d <distro> --exec kimi -p "/usage"` 执行，并将 `KIMI_CODE_HOME` 转换为 Linux 格式。
3. **超时与降级控制**：子进程执行必须设置 `tokio::time::timeout`（如 10 秒超时限制）。若命令执行超时、CLI 未安装、或输出正则匹配失败，必须优雅捕获错误并降级返回 `None` / 错误提示，严禁阻塞 UI 渲染或引发 panic。

```rust
// 标准调用范式
pub async fn fetch_kimi_usage(
    state: &SqliteDbState,
    root_dir: &Path,
) -> Result<Option<KimiUsageInfo>, String> {
    let location = runtime_location::get_kimi_runtime_location_async(state).await?;
    let cli_path = cli_resolver::resolve_cli_path_async("kimi", &location).await?;

    let run_cmd = async {
        match location {
            RuntimeLocation::Local(_) => {
                tokio::process::Command::new(cli_path)
                    .args(["-p", "/usage"])
                    .env("KIMI_CODE_HOME", root_dir)
                    .output()
                    .await
            }
            RuntimeLocation::WslDirect { distro, linux_path, .. } => {
                tokio::process::Command::new("wsl")
                    .args(["-d", &distro, "--exec", &cli_path, "-p", "/usage"])
                    .env("KIMI_CODE_HOME", &linux_path)
                    .output()
                    .await
            }
        }
    };

    match tokio::time::timeout(std::time::Duration::from_secs(10), run_cmd).await {
        Ok(Ok(output)) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(parse_usage_output(&stdout))
        }
        Ok(Ok(output)) => {
            log::warn!("Kimi /usage exit non-zero: {:?}", output.status);
            Ok(None)
        }
        Ok(Err(e)) => {
            log::warn!("Failed to execute kimi usage: {}", e);
            Ok(None)
        }
        Err(_) => {
            log::warn!("Kimi /usage command timed out (10s)");
            Ok(None)
        }
    }
}
```

**方案 B（备选）**：引导用户到 Kimi Code Console 查看

```
// 在页面中嵌入链接
"https://console.kimi.com"
```

**方案 C（远期）**：如果 Kimi 未来开放 billing API，再切换为 REST 调用（通过 `crate::http_client` 发起）。

### 12.3 展示内容

在官方账号列表中展示：

| 字段 | 来源 |
|------|------|
| 套餐类型 | `plan_type`（如 `free`、`pro`、`max`） |
| 周剩余额度 | `weekly_remaining` |
| 周重置时间 | `weekly_reset_at` |
| 月剩余额度 | `monthly_remaining`（仅当 `monthlyLimit > 0`） |
| Extra Usage 余额 | `extra_usage_balance`（可选） |

---

## 13. 插件市场

### 13.1 现状

Kimi 有完整的插件管理体系：

- 市场 URL：`https://code.kimi.com/kimi-code/plugins/marketplace.json`
- 安装路径：`$KIMI_CODE_HOME/plugins/managed/<id>/`
- 记录文件：`plugins/installed.json`

### 13.2 集成范围

AI Toolbox **只读展示**已安装插件列表：

- 读取 `plugins/installed.json` 获取插件列表
- 展示插件名称、版本、启用状态
- 不实现安装/卸载/更新（这些操作在 Kimi CLI 内通过 `/plugins` 命令完成）

### 13.3 不在范围内

- Kimi 插件市场管理（新增/删除/更新）
- Kimi 插件 MCP 服务器管理
- Kimi 插件的 OAuth 凭证管理

---

## 14. 实施路线图

### 阶段 1：基础框架与配置管理（P0，预计 3-4 天）

```
[x] 创建 tauri/src/coding/kimi/ 模块框架
[x] 实现 types.rs、adapter.rs（SQLite JSONB）、constants.rs
[x] 实现 commands.rs 的基础 Tauri commands
[x] 基于 toml_edit::DocumentMut 实现 config.toml AST 级解析与写入（含字段保留与前置快照清理）
[x] 注册模块到 runtime_location、reapply_applied_runtime、mod.rs
[x] 补齐所有 Allowlist 白名单（settings types/adapter, visible_tabs, sidebar_hidden 等）
[x] 使用 pnpm i18n:set-key 初始化 Kimi 模块中英文翻译
[x] 实现前端 KimiPage（遵循 DESIGN.md 视觉与 token，Provider 配置区）
[x] 实现前端 kimiApi.ts（service layer）
[x] 编写 toml_edit 字段保留与 round-trip 单元测试
```

### 阶段 2：OAuth、HTTP 与托盘（P0-P1，预计 2-3 天）

```
[x] 基于 crate::http_client + rustls 实现 official_accounts.rs（Device Code OAuth）
[x] 对齐 coding::auth_refresh 调度器（区分 applied 与 non-applied 写入边界）
[x] 实现前端官方账号列表 + 登录 UI（遵循 DESIGN.md；登出由「删除账号」承担，独立 logout 命令在 review 轮确认为死代码并移除）
[x] 实现 tray_support.rs 并注册托盘菜单
[x] 前端 Common Config 配置区
```

### 阶段 3：会话、CLI 与同步（P1，预计 2 天）

```
[x] 实现 session_manager/kimi.rs
[x] 注册前端 useWSLSync / useSSHSync 白名单映射与弹窗选项
[x] 实现 Skills 同步（中央仓库 → Kimi skills 目录）
[x] 接入 cli_resolver 支持手动 CLI 路径解析与 WSL Direct 命令转换
```

### 阶段 4：增强功能与验收测试（P1-P2，预计 2-3 天）

```
[x] Gateway 代理集成（GatewayCliKey::Kimi + cli_proxy）
[x] 实现 Billing/Usage（已落地为外链方案，跳转 https://www.kimi.com/code/console）
[x] 前端 Prompt 配置区
[x] 实现插件列表展示（只读）
[x] 编写 tauri/tests/ 下的集成测试用例
[x] 全量校验跑通：pnpm test + cargo test + pnpm exec tsc --noEmit
```

### 总计：9-12 天

---

## 15. 风险与缓解

### 15.1 风险矩阵

| 风险 | 概率 | 影响 | 等级 | 缓解措施 |
|------|------|------|------|---------|
| Kimi CLI 输出格式变更 | 中 | 中 | 🟡 | 为 `/usage` 解析写测试，版本升级时回归 |
| Kimi OAuth 端点变更 | 低 | 高 | 🟡 | 参考 Grok 的 OAuth 实现，配置化端点 URL |
| Kimi 的 TOML 格式新增字段 | 中 | 低 | 🟢 | 字段保留策略，未知字段自动保留 |
| Kimi CLI 安装缺失 | 中 | 低 | 🟢 | 已有 cli_resolver 机制，缺失时引导安装 |
| Kimi 插件市场 API 不兼容 | 低 | 低 | 🟢 | 只读展示已安装列表，不依赖市场 API |
| WSL Direct 下 TOML 读写阻塞 | 低 | 中 | 🟢 | 复用 `coding::file_io` 的 spawn_blocking + 超时 |
| Kimi 的 Managed Provider 更新 | 低 | 低 | 🟢 | 仅维护 `[providers.*]` 和 `[models.*]`，managed 前缀保护 |

### 15.2 已知限制

1. **Billing/Usage 无公开 API**：只能通过 CLI 命令解析，无法像 Grok 那样优雅集成
2. **Plugin 深度管理不支持**：Kimi 的插件市场管理在 CLI 内完成，AI Toolbox 暂不实现
3. **OAuth 凭证不迁移**：旧版 `kimi-cli` 迁移时 OAuth 凭证不会复制，需重新 `/login`
4. **Kimi 的 `kimi` provider type 有特殊能力**：`video_in` 等非标准 OpenAI 能力，协议转换时需注意
5. **Kimi 的 Anthropic-compatible 模式**：thinking 参数使用 `reasoning_content` 而非原生 Anthropic 格式

### 15.3 验收标准

| 验收项 | 方法 |
|--------|------|
| Provider 配置 round-trip | `read → edit → save → apply → read` 后 fixture 只出现预期差异 |
| 非受管字段保留 | Provider 写入后 `hooks`、`permission`、`services` 等字段仍存在 |
| OAuth 登录 | 成功获取 token，写入 `credentials/` 目录，权限 0600 |
| OAuth 登出 | 清除 token，移除 `credentials/<name>.json` |
| 额度解析 | 调用 `kimi -p "/usage"` 后正确解析输出 |
| 会话浏览 | 列出 `sessions/` 目录下的所有会话 |
| 会话恢复 | 生成正确的 `kimi -S <sessionId>` 命令 |
| Gateway 接管 | 网关接管后直连操作被拒绝 |
| WSL 同步 | 配置文件正确同步到 WSL 远端目录 |
| Skills 同步 | 中央仓库内容正确写入 `$KIMI_CODE_HOME/skills/` |

---

## 16. 实现状态与偏差记录

> 本节在实施完成后回填，记录与上文方案的偏差和验收轮补齐项。

### 16.1 已完成范围

- 阶段 1-3 全部落地：`tauri/src/coding/kimi/` 后端模块（types/constants/adapter/commands/official_accounts/tray_support）、runtime_location、auth_refresh、session_manager/kimi.rs、前端 `web/features/coding/kimi/` 全套页面与组件、WSL/SSH 白名单、i18n 双语文案。
- 验收轮补齐：
  - **Gateway CLI 接管**：`GatewayCliKey::Kimi`、cli_proxy manifest（kind `kimi_config_toml`，受管字段为当前生效 provider 表的 `type/base_url/api_key`，按 `default_model → models.<key>.provider` 动态解析，回退 `managed:kimi-code`）、入站路由 `/kimi/v1/chat/completions`（OpenAI Chat source，`/kimi/v1` 仅 GET/HEAD 探测）、`ensure_kimi_gateway_direct` 直连拒绝、reapply 锁定重灌、前端接管入口。
  - **托盘菜单接线**：`tray.rs` 接入 `kimi_tray`（`kimi_provider_`/`kimi_model_`/`kimi_prompt_` 事件、`is_tab_visible("kimi")` 门控）。
  - **cli_resolver**：`resolve_local_kimi_program()`（npm 包 `@moonshot-ai/kimi-code`），KimiPage 补齐「更多选项」手动 CLI 路径入口。

### 16.2 未实现项

- **Billing/Usage 额度展示**：已落地为外链方案（跳转 `https://www.kimi.com/code/console`）；方案 A（`kimi -p "/usage"` 解析）前提证伪——0.38.0 实测非交互模式不拦截 `/usage`，0.39.1 源码确认 TUI `/usage` 仅输出会话 token 统计、无会员额度字段、无 billing API。

### 16.3 偏差与踩坑记录

- `tools/builtin.rs` 的 kimi skills/MCP/检测路径最初误写为 `~/.kimi/*`，与模块 Source of Truth（`~/.kimi-code/`，`KIMI_CODE_HOME` 覆盖）不一致，验收轮已修正为 `~/.kimi-code/skills`、`~/.kimi-code/config.toml`。
- 侧边栏曾静默不显示 Kimi：`visible_tabs` 与后端迁移均已含 `kimi`，但 `web/constants/modules.tsx` 的 `MODULES` subTabs 漏注册（`visibleTabs.map(...find)` 返回 undefined 被过滤）。已修复并把 `MODULES` 补进根 AGENTS.md 的 Tab allowlist 必查清单。
- Gateway 协议层早已支持 Kimi/Moonshot（provider profile、transformer 兼容、usage 定价），本文第 5 节的“新增工作”仅指 CLI 接管链路，两者不要混淆。
- 架构文档 `docs/gateway-protocol-conversion.md` 已同步 Kimi 入站路由、source protocol 推导与 target protocol 解析规则；渠道兼容事实未变化，`docs/gateway-provider-compatibility.md` 无需改动。
- Gateway 接管初版把受管字段写死为 `[providers."managed:kimi-code"].base_url/api_key`，但自定义 provider（如 axonhub）apply 后 `[models.<alias>].provider` 指向自定义 key，CLI 流量完全绕过网关（统计恒 0），且 `current_kimi_gateway_endpoint` 只看 managed 表导致状态误报绿色。已修复为按 `default_model → models.<key>.provider` 动态解析当前生效 provider key（回退 `managed:kimi-code`），patch / 状态判定 / WSL 副本改写三处共用同一 helper。同期补齐 Gateway 前端 allowlist 遗漏：统计页 CLI 筛选（`GatewayStatisticsView.tsx` `cliOptions`，连补 `claude_desktop`）、定价弹窗每 CLI 默认计费（`ModelPricingModal.tsx` `pricingCliKeys`）、`normalizeGatewayProviderTool` 白名单。接管修复后流量已正常落库，但请求列表/统计仍看不到 kimi：`usage_stats.rs` 的 `cli_key_from_app_type` 漏 `"kimi"` 导致已落库行在查询映射时被静默丢弃，`load_provider_names` 漏 `kimi_provider` 表导致名称无法解析；均已补注册并加回归测试（`kimi_usage_rows_appear_in_request_logs_and_resolve_provider_name`）。
- Review 轮修复（2026-08）：① `session_manager/kimi.rs` 导入快照的 `files[].path` 未走 `join_safe_relative`（兄弟模块均有防护），存在路径遍历，已修复并加恶意路径回归测试；② `logout_kimi_official_runtime` 前端不传 `account_id`（调用必失败）且无调用方，连同语义问题（登出却把账号设为 applied）整体移除；③ OAuth 刷新端点从登录时 discovery 结果持久化到账号快照（`token_endpoint`），刷新优先读快照，单账号失败记录 `last_error` 后继续不阻断其余账号；④ `json_value_to_toml` 手拼字符串在嵌套对象/特殊 key 下产出无效 TOML，重写为直接构造 `toml::Value` 由序列化器转义；⑤ 托盘补接 `kimi_prompt_` 子菜单（原先 tray_support 的 prompt 函数是死代码）；⑥ 官方账号重复登录改为同 provider 更新去重（重复记录会同名凭证文件互相覆盖）；⑦ 空 custom provider 不再投影悬空 `default_model`/无内容 `[providers.*]`；⑧ DeviceAuthModal 增加状态轮询兜底并修正状态机注释；⑨ KimiPage 删除/应用 provider 补错误提示。