# Shared Components Development Guide

## 一句话职责

- 为多个页面提供编辑器和基础交互组件，并保证用户输入规模或内容形态不会阻塞前端主线程。

## 核心设计决策（Why）

- Monaco Monarch tokenizer 在 WebView 主线程执行。字符串规则必须保持线性时间；正则分支不能重叠消费同一字符，否则包含大量转义符的配置行会触发灾难性回溯并冻结整个主窗口。
- 编辑器组件一律 `import * as monaco from 'monaco-editor/esm/vs/editor/editor.api'`（核心 API 入口，不自动注册语言），按需 `import 'monaco-editor/esm/vs/language/json/monaco.contribution'` 只注册 JSON。**不要**改回 `from 'monaco-editor'`（`editor.main` 入口会全量注册 css/html/typescript 语言并拉入对应 worker bundle，~8.7 MB JS 常驻 webview 内存，而这些编辑器从不使用 css/html/ts 语言）。`web/app/monaco.ts` 的 `MonacoEnvironment.getWorker` 也只注册 `editor` 与 `json` 两个 worker；新增语言 worker 时需同步在 workerFactories 里登记，并确认确有编辑器用到该 language。

## 易错点与历史坑（Gotchas）

- TOML 双引号字符串的“未闭合”规则中，转义分支 `\\.` 与普通字符分支必须互斥。普通字符分支必须排除反斜杠，使用 `[^"\\]`，不能退回会同时匹配反斜杠的 `[^\"]`。
- 不要只用普通短配置验证 tokenizer。Codex `notify` 等配置会把 JSON 嵌入 TOML 字符串，形成包含大量反斜杠和转义引号的超长单行。
- `FetchModelsModal` 的展示顺序统一按 `sort.ts` 的 owner 分组排序（locale 钉死 `en` 保证确定性）；`priorityOwnedBy` 是可选 prop，消费方（如 Codex 置顶 openai）自选，**不得**把具体厂商偏好写进默认行为。`onSuccess` 的 `orderedModelIds` 是完整列表的显示顺序（含未勾选项），供消费方对齐自身列表/映射顺序；`selectedModels` 必须从**完整列表**的排序结果里过滤（现在 `handleConfirm` 的做法），不能从搜索过滤后的视图取——否则搜索状态下确认会静默丢弃被过滤隐藏的已勾选模型。
- `FetchModelsModal` 搜索只改变视图，跨搜索的选择必须保留；Ant Design Table 需要 `preserveSelectedRowKeys: true`，否则第二次勾选时就会丢掉隐藏行，确认阶段遍历完整列表也无法恢复。关闭、重新获取或切换连接后应重置选择，旧连接/旧弹窗的异步结果不能覆盖新结果。
- 模型导入弹窗可能一直挂载，不能只在首次 `useState` 初始化 SDK 对应的 API 类型；每次打开或切换 SDK 都要重置为正确的原生/兼容模式。Google Native 的可编辑 URL 必须与后端发现路径一致：无版本时仅在探测 URL 补 `/v1beta`，保留显式版本和用户手改 URL，不改写供应商保存的 Base URL。

## 最小验证

- 修改 TOML tokenizer 后，运行 `web/test/components/common/TomlEditor/invalidDoubleQuoteStringPattern.test.ts`。
- 语义覆盖要同时包含：未闭合串、普通 closed 串、真实 Codex `notify` 风格 Windows 路径 closed 串，以及会触发指数回溯的 adversarial closed 串。
- 指数回溯回归必须在可终止的 Worker 中执行（超时即失败），避免危险正则重新出现时把完整测试进程永久卡住。
- 修改 `FetchModelsModal` 排序或 `onSuccess` 契约后，运行 `web/test/components/common/FetchModelsModal/sort.test.ts`，并确认 `onSuccess` 新增字段对所有消费方（OpenCode/Grok/Pi/DSH/Hermes/OhMyPi/OpenClaw 页面与 Codex 表单）是纯增量。
- 搜索选择和协议切换使用 `pnpm test:codex-model-import` 验证真实共享弹窗；需要已安装 Chrome/Edge，可传 `--browser` 指定路径。模型接口和保存由隔离桩提供，不写用户配置。
