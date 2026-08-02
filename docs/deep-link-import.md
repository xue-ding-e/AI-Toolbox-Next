# Deep-Link Provider 导入使用说明

ai-toolbox 支持通过 `aitoolbox://` 自定义协议链接一键导入供应商（provider）。点击链接后，应用会被唤起并弹出确认对话框（API 密钥脱敏展示），用户确认后即写入对应工具的供应商表。本文同时介绍面向终端用户的用法与面向开发者/二次开发者的实现细节。

## 目录

- [快速开始](#快速开始)
- [面向用户](#面向用户)
  - [链接格式](#链接格式)
  - [各工具示例](#各工具示例)
  - [确认流程](#确认流程)
  - [常见问题](#常见问题)
- [面向开发者](#面向开发者)
  - [架构总览](#架构总览)
  - [URL 字段参考](#url-字段参考)
  - [各工具 settings_config 形态](#各工具-settings_config-形态)
  - [config / extra 高级覆盖](#config--extra-高级覆盖)
  - [错误处理与日志脱敏](#错误处理与日志脱敏)
  - [冷启动竞态与回放](#冷启动竞态与回放)
  - [平台差异](#平台差异)
  - [扩展指南](#扩展指南)
  - [验证清单](#验证清单)

---

## 快速开始

最简单的导入链接：

```
aitoolbox://v1/import?resource=provider&app=codex&name=OpenRouter&category=third_party&apiKey=sk-or-xxx&baseUrl=https://openrouter.ai/api/v1&model=gpt-5
```

把这段链接放进 HTML `<a href="...">`、Markdown、或直接在浏览器地址栏粘贴访问，即可唤起 AI Toolbox 并弹出导入确认框。

---

## 面向用户

### 链接格式

```
aitoolbox://v1/import?resource=provider&app=<工具>&name=<名称>&category=<类别>&<其它参数>
```

- **协议**：`aitoolbox`（固定）
- **版本**：`v1`（固定，放在 `://` 后第一段，用于后续不兼容升级）
- **路径**：`/import`（固定）
- **必填参数**：`resource`、`app`、`name`、`category`
- 其余参数全部可选，值需要 URL 编码（如空格 `%20`、冒号 `%3A`、斜杠 `%2F`）

#### 必填参数

| 参数 | 取值 | 说明 |
|---|---|---|
| `resource` | `provider` | v1 仅支持供应商导入。`mcp`/`prompt`/`skill` 留待后续。 |
| `app` | `claude` / `codex` / `gemini` | 目标工具。`grok` 暂不支持（见下方「平台差异」节）。 |
| `name` | 任意非空字符串 | 供应商显示名称。 |
| `category` | `official` / `third_party` / `custom` / `aggregator` | 类别。`aggregator` 会被规范化为 `third_party`，未知值默认 `custom`。 |

#### 可选参数（工具通用）

| 参数 | 说明 |
|---|---|
| `apiKey` | API 密钥 / 认证令牌。 |
| `baseUrl` | 基础地址，必须是 `http` 或 `https`。 |
| `model` | 默认模型 ID。 |
| `homepage` | 供应商主页，必须是 `http`/`https`。 |
| `notes` | 备注。 |
| `icon` | 图标名称。 |
| `iconColor` | 图标颜色（CSS 颜色值）。 |
| `sourceProviderId` | 来源 ID，用于去重（如 `ccs:codex:xxx`）。 |
| `config` | Base64 编码的工具特定 JSON/TOML，**直接覆盖** builder 产出的 `settings_config`。 |
| `extra` | Base64 编码的 JSON，仅 Claude 用，作为 `extra_settings_config`。 |

### 各工具示例

#### Claude Code

```
aitoolbox://v1/import?resource=provider&app=claude&name=My%20Claude&category=custom&apiKey=sk-ant-xxxxxxxx&baseUrl=https%3A%2F%2Fapi.example.com&model=claude-sonnet-4&homepage=https%3A%2F%2Fexample.com
```

导入后该供应商的环境变量为：`ANTHROPIC_AUTH_TOKEN`、`ANTHROPIC_BASE_URL`、`ANTHROPIC_MODEL`。

#### Codex

```
aitoolbox://v1/import?resource=provider&app=codex&name=OpenRouter&category=third_party&apiKey=sk-or-xxx&baseUrl=https%3A%2F%2Fopenrouter.ai%2Fapi%2Fv1&model=gpt-5&homepage=https%3A%2F%2Fopenrouter.ai
```

导入后 `auth.OPENAI_API_KEY` 取 `apiKey`，并生成最小 TOML `config`（含 `model_provider`、`model`、`[model_providers.<slug>]` 的 `name`/`base_url`）。`slug` 由 `name` 小写化、非字母数字替换为 `-` 得到。

#### Gemini CLI

```
aitoolbox://v1/import?resource=provider&app=gemini&name=Proxy%20Gemini&category=custom&apiKey=AIzaXXXX&baseUrl=https%3A%2F%2Fgemini-proxy.example.com&model=gemini-2.5-pro
```

导入后环境变量为：`GEMINI_API_KEY`、`GOOGLE_GEMINI_BASE_URL`、`GEMINI_MODEL`。

### 确认流程

1. 点击链接（应用未运行则先启动；运行中则聚焦窗口）。
2. 弹出「通过链接导入供应商」对话框，展示：工具、名称、类别、**脱敏 API 密钥（仅前 4 位 + 20 个星号）**、基础地址、模型、主页、备注。
3. 点「导入」才真正写入数据库；点「取消」或关闭对话框则什么都不发生。
4. 导入成功后弹出成功提示，对应工具页面（若已打开）自动刷新供应商列表，托盘菜单同步刷新。

> 安全设计：后端只负责解析链接并把请求发给前端，**绝不**在收到链接时自动写库。是否写入完全由用户在对话框里点「导入」决定。

### 常见问题

**点了链接没反应？**
- Windows/Linux 开发版（非安装包）需运行时注册协议，应用启动时会自动 `register_all()`；如果是从源码直接跑 `cargo run`，确保应用至少启动过一次。
- macOS 的自定义协议仅在**安装版**（`.app` 放入 `/Applications`）生效，开发版直接跑无法接收 deep-link。
- 浏览器可能拦截自定义协议，留意地址栏是否出现「打开 AI Toolbox?」的确认提示。

**为什么 Grok 不支持？**
Grok 的供应商配置形态（`defaultModelKey` + `modelCatalog`）与其它三个 env 型工具差异较大，且非官方类别一旦设了默认模型就必须带非空 `modelCatalog`，构造起来更复杂。v1 暂时只支持 `claude`/`codex`/`gemini`，Grok 留待后续单独打磨。

**链接里的密钥会被记录到日志吗？**
不会。后端日志里对 deep-link URL 做了脱敏：所有 query 参数的值一律替换为 `***REDACTED***`，只保留键名。

**冷启动时链接会丢失吗？**
不会。应用冷启动收到链接时前端对话框还没挂载监听，后端会把请求暂存到一个 pending slot。前端监听器挂载完成后调用 `mark_deeplink_frontend_ready`，原子标记 listener ready 并取走 pending 请求，确保冷启动也能弹出确认框。

---

## 面向开发者

### 架构总览

实现灵感来自 cc-switch（CCS），但适配 ai-toolbox 的**分表 provider 模型**（claude/codex/gemini/grok 各有独立 `*_provider` 表 + `create_*_provider` 命令）。

关键事实：`tauri-plugin-deep-link` 插件自身已把三个 URL 入口统一成一个 `deep-link://new-url` 事件——所以 ai-toolbox 只需在 `on_open_url` 一处接收即可，比 CCS 手写三入口更简洁。

```
用户点击 aitoolbox:// 链接
        │
        ▼
┌────────────────────────────────────────────────────────┐
│ OS 层：交给已注册的协议处理器                           │
│  • macOS: AppleEvent (kAEOpenURL) → RunEvent::Opened   │
│  • Win/Linux 冷启动: argv → 插件 init_deep_link         │
│  • Win/Linux 第二实例: argv 由 single-instance 的       │
│    deep-link feature 转发给运行中实例                   │
└────────────────────────────────────────────────────────┘
        │ 统一为 deep-link://new-url 事件
        ▼
install_deeplink_handlers → on_open_url 回调
        │
        ▼
handle_deeplink_url(app, url, focus_window)        [只解析，不写库]
        ├─ parse_deeplink_url(url) → DeepLinkImportRequest
        │     ├─ 校验 scheme/version/path/resource/app 白名单
        │     ├─ http/https 校验 baseUrl/homepage
        │     ├─ 明确拒绝 v1 尚不持久化的 endpoints
        │     └─ 容忍式 Base64 解码 config/extra
        ├─ 前端未 ready 时存入 DeepLinkState.pending（latest-wins）
        ├─ emit("deep-link-import", request) → 前端对话框
        └─ (失败) emit("deep-link-error", {url 脱敏, error})
        │
        ▼
前端 AppInitializer → useDeepLinkImport 监听
  → DeepLinkImportDialog 展示脱敏详情
        │ 用户点「导入」
        ▼
importFromDeeplinkUnified(request) → invoke("import_from_deeplink_unified")
        │
        ▼
build_and_create_provider(db_state, app, request)    [唯一写库点]
  ├─ build_claude/codex/gemini_settings(req) → settings_config JSON 字符串
  └─ create_*_provider_inner(state, app, input) → 写库 + emit config-changed
        │
        ▼
前端：dispatchEvent(DEEP_LINK_IMPORT_COMPLETED)
  → 对应工具页面 loadConfig(true) 刷新；refreshTrayMenu()
```

### URL 字段参考

完整字段见 `tauri/src/coding/deeplink/parser.rs` 的 `DeepLinkImportRequest`。该结构 `Serialize` 为 camelCase 后通过 Tauri 事件传给前端：

```rust
pub struct DeepLinkImportRequest {
    pub resource: String,
    pub app: String,
    pub name: String,
    pub category: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub homepage: Option<String>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub icon_color: Option<String>,
    pub source_provider_id: Option<String>,
    pub config: Option<String>,   // 解码后的字符串，前端永不收到原始 base64
    pub extra: Option<String>,    // 解码后的字符串
    pub raw_url: String,
}
```

`config` 与 `extra` 在 parser 内部解码为明文再序列化出去——原始 base64 不会越过 IPC 边界，保持密文材料显式可控。

### 各工具 settings_config 形态

四个工具的 `*ProviderInput.settings_config` 都是 **JSON 字符串**（不是对象）。其形态与 `tauri/src/coding/cc_switch.rs` 里 `extract_*_candidate` 产出的完全一致。

#### Claude（`build_claude_settings`）

```json
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "<apiKey>",
    "ANTHROPIC_BASE_URL": "<baseUrl>",
    "ANTHROPIC_MODEL": "<model>"
  }
}
```

`extra_settings_config` 默认 `"{}"`，可用 `extra` 参数覆盖。

#### Codex（`build_codex_settings`）

```json
{
  "auth": { "OPENAI_API_KEY": "<apiKey>" },
  "config": "<TOML 字符串>"
}
```

TOML 形如（`<slug>` 由 name 转换而来）：

```toml
model_provider = "<slug>"
model = "<model>"

[model_providers.<slug>]
name = "<name>"
base_url = "<baseUrl>"
```

例：`name=OpenRouter` → `slug=openrouter` → `[model_providers.openrouter]`。

#### Gemini（`build_gemini_settings`）

```json
{
  "env": {
    "GEMINI_API_KEY": "<apiKey>",
    "GOOGLE_GEMINI_BASE_URL": "<baseUrl>",
    "GEMINI_MODEL": "<model>"
  },
  "config": {}
}
```

#### Grok

v1 **不支持**。`app=grok` 在 parser 阶段即被 `DeepLinkError::UnsupportedApp` 拒绝。Grok 的 `settings_config` 形态是 `{defaultModelKey, auth.API_KEY, modelCatalog.models[], config?(TOML)}`，且 `grok/commands.rs:validate_provider_settings` 强制「非 official 类别 + 有 defaultModelKey ⇒ 必须带非空 modelCatalog」。当前 `cc_switch.rs` 里也没有 `extract_grok_candidate`，连现有 cc-switch 导入都绕开了它。后续如需支持，要在 `provider.rs` 新增 `build_grok_settings` 合成最小 modelCatalog 条目（`apiBackend: "responses"`、`envKey: "XAI_API_KEY"`），并放开 parser 的 `SUPPORTED_APPS`。

### config / extra 高级覆盖

`config` 参数提供一个 escape hatch：当扁平的 `apiKey/baseUrl/model` 不够用时，可直接传一个完整的 `settings_config`（Claude/Gemini）或完整 TOML `config`（Codex），builder 会**整段使用**它而非自行装配。

例（Claude，传一个自定义 env 块）：

```
aitoolbox://v1/import?resource=provider&app=claude&name=Custom&category=custom&config=<base64>{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-xxx","ANTHROPIC_BASE_URL":"https://api.x.com"}}
```

`extra` 仅对 Claude 有效，作为 `extra_settings_config`（用于存放 permissions 等 settings.json 顶级字段）。

> 注意：使用 `config` 覆盖时，builder 不会再注入 `apiKey/baseUrl/model`——你需要自行在 config 里包含它们。前端对话框展示的脱敏字段仍取自 URL 的扁平参数（可能为空），用户看到的可能与实际写入的不同。生产环境建议优先用扁平参数，仅在确有需要时用 `config`/`extra`。

### 错误处理与日志脱敏

解析失败时后端 emit `deep-link-error`，payload `{ url: 脱敏URL, error: 错误信息 }`，前端右上角 toast 提示。常见错误：

| 错误 | 触发条件 |
|---|---|
| `BadScheme` | scheme 非 `aitoolbox` |
| `BadVersion` | host 非 `v1` |
| `BadPath` | path 非 `/import` |
| `UnsupportedResource` | `resource` 非 `provider` |
| `UnsupportedApp` | `app` 不在 `claude/codex/gemini`（含 `grok`） |
| `UnsupportedParam("endpoints")` | `endpoints` 暂无明确持久化语义，v1 拒绝而不是静默丢弃 |
| `MissingParam("name")` | 缺 `name` 或为空 |
| `InvalidUrl { field, detail }` | `baseUrl`/`homepage` 非 http/https |
| `InvalidBase64("config"` / `"extra")` | base64 解码失败 |

日志脱敏由 `utils::redact_url_for_log` 实现：重解析 URL，把所有 query value 替换为 `***REDACTED***`，去掉 userinfo/fragment，只保留 `scheme://host/path?k=***REDACTED***&...`。

### 冷启动竞态与回放

冷启动时序：
1. OS 启动应用并把 URL 放 argv（Win/Linux）或通过 `RunEvent::Opened`（macOS）。
2. 插件在 `setup` 阶段 `init_deep_link` → `handle_cli_arguments` 立刻 emit `deep-link://new-url`。
3. `on_open_url` 收到 → `handle_deeplink_url` 把请求存进 `DeepLinkState.pending` 队列 + emit `deep-link-import`。
4. 但前端 `AppInitializer` 的 `deep-link-import` 监听尚未挂载 → 这次 emit 丢失。
5. 前端 listener attach 完成后调用 `mark_deeplink_frontend_ready` → 后端标记 frontend ready 并返回 pending 请求 → 对话框出现。

热启动时 listener 已 ready，后端只发 live `deep-link-import`，不再写 pending，因此不会在后续 ready 信号或重挂载时重复回放。pending 采用 **latest-wins**，即前端 ready 前多个 URL 先后到达只保留最后一个。如需多 URL 堆叠，将来改为 `Vec`。

### 平台差异

| 平台 | scheme 注册 | URL 入口 |
|---|---|---|
| Windows（安装版） | NSIS/MSI 从 tauri.conf.json 写注册表 | macOS 同理走 AppleEvent |
| Windows（dev/`cargo run`） | `app.deep_link().register_all()` 写 `HKCU\Software\Classes\aitoolbox` | argv（单参数 URL） |
| Linux（安装版） | bundle 写 `.desktop` mime | argv |
| Linux（dev） | `register_all()` 写 `~/.local/share/.../applications/*-handler.desktop` + `xdg-mime` | argv |
| macOS | `Info.plist` 的 `CFBundleURLTypes`（需安装版构建） | `RunEvent::Opened`（插件 on_event 转发） |

`tauri-plugin-single-instance` 的 `deep-link` cargo feature 让第二实例的 argv 在我们自己的回调运行之前先被插件 `handle_cli_arguments` 处理并转发给运行中实例，从而把「第二实例」也归到 `on_open_url`。故 `lib.rs` 里 single-instance 的 user callback 保持只做 show+focus，无需手动扫 argv。

### 扩展指南

#### 新增一个 app（如未来支持 opencode）

1. `parser.rs`：把 app 加入 `SUPPORTED_APPS`。
2. `provider.rs`：新增 `build_<app>_settings(req)` 产出该工具的 `settings_config`；在 `build_and_create_provider` 的 match 加一支，构造对应 `*ProviderInput` 并调用 `create_<app>_provider_inner`。
3. 若该 tool 的 `create_*_provider` 还未抽出 inner 函数，按 `create_claude_provider_inner` 的模式重构。
4. 前端 `deeplinkApi.ts` 的 `DeepLinkApp` 类型加新值，`DeepLinkImportDialog.tsx` 的 `APP_LABEL_KEYS` 加映射，i18n 加 `appXxx`。
5. 对应工具页面加 `DEEP_LINK_IMPORT_COMPLETED` 监听（`detail.app === '<app>'`）。

#### 新增一种 resource（如 mcp/prompt/skill）

1. `parser.rs`：放开 `SUPPORTED_RESOURCE` 校验或新增 `match` 分支，定义各自的必填字段与校验。
2. `DeepLinkImportRequest` 增补该 resource 专有字段。
3. `provider.rs` 新建对应模块文件（如 `mcp.rs`），在 `build_and_create_provider` 按 `resource` 分发。
4. `import_from_deeplink_unified` 命令的 `resource` 校验放开。
5. 前端 `DeepLinkImportRequest` 类型与 `DeepLinkImportDialog` 增分支展示；`DeepLinkImportResult.type` 增值并对应刷新逻辑。

涉及代码位置速查：

| 关注点 | 文件 |
|---|---|
| 协议注册 | `tauri/tauri.conf.json`、`tauri/capabilities/default.json` |
| 插件接线 | `tauri/src/lib.rs`（builder、setup、generate_handler!） |
| URL 解析/校验 | `tauri/src/coding/deeplink/parser.rs` |
| settings_config 装配/分发 | `tauri/src/coding/deeplink/provider.rs` |
| 漏斗/队列/命令/回放 | `tauri/src/coding/deeplink/mod.rs` |
| 内部写库复用点 | 各 `tauri/src/coding/<tool>/commands.rs` 的 `create_*_provider_inner` |
| 前端 API 封装 | `web/services/deeplinkApi.ts` |
| 前端事件监听 | `web/features/shared/deepLink/useDeepLinkImport.ts` |
| 前端确认对话框 | `web/features/shared/deepLink/DeepLinkImportDialog.tsx` |
| 全局挂载 | `web/app/providers.tsx`（`DeepLinkImportMount`） |
| 页面刷新事件 | `web/constants/configEvents.ts`、四个工具页面 |
| i18n | `web/i18n/locales/{zh-CN,en-US}.json` 的 `common.deepLink.*` |

### 验证清单

**前置**：`pnpm tauri dev`。Windows/Linux dev 下 `register_all()` 会自动注册 scheme；macOS 需装 installed 构建测 deep-link。

1. **热启动（每工具）**：应用运行中，浏览器/CLI 触发示例链接 → 窗口聚焦 → 确认弹窗显示脱敏密钥 → 确认 → `*_provider` 表新增行、`settings_config` 形态正确 → 成功 toast → 页面刷新。
2. **冷启动**：退出应用再触发链接 → 应用启动后弹窗出现（frontend listener ready command drain pending）→ 导入成功。
3. **第二实例（Win/Linux）**：运行中，终端执行 `ai-toolbox.exe "aitoolbox://v1/import?..."` → 第二实例退出、原窗口聚焦、弹窗出现。
4. **macOS 冷启动**：装 installed 构建，退出，浏览器点链接 → 应用启动、Dock 激活、弹窗出现。
5. **错误链接**：`v2`→`BadVersion`；`resource=mcp`→`UnsupportedResource`；`app=grok`→`UnsupportedApp`；`endpoints=https://x`→`UnsupportedParam`；缺 `name`→`MissingParam`；`baseUrl=ftp://x`→`InvalidUrl`；均走 `deep-link-error`、toast 提示、无弹窗。
6. **config 覆盖**：Claude 链接带 `config=<base64 {"env":{...}}>` → 导入后 `settings_config` 为解码内容；带 `extra=...` → `extra_settings_config` 为解码 JSON。
7. **日志脱敏**：触发带 `apiKey=secret` 的错误链接，查后端日志 → `apiKey=***REDACTED***`。
8. **不确认不写库**：触发链接后不点导入 → `*_provider` 表无变化。
9. **托盘刷新**：导入后托盘菜单含新供应商。
10. **回归**：现有 cc-switch 导入（Claude/Codex/Gemini 页 ImportFromCcSwitchModal）不受 inner 重构影响。
11. **自动化**：`cargo test --lib`（含 `deeplink::*` 的 23 个单测）、`pnpm test:web`、`pnpm i18n:check` 全绿。
