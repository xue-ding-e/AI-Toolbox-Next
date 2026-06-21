# Pi 后端模块说明

## 一句话职责

- `pi/` 负责 Pi CLI 全局 root、`settings.json`、`auth.json`、`models.json`、全局 prompt 和页面 runtime view。

## Source of Truth

- Pi provider 的事实源是 Pi runtime 文件，不是 AI Toolbox 数据库 provider 表。
- `auth.json.<providerKey>` 是 API key / OAuth credential entry。
- `models.json.providers.<providerKey>` 是 custom provider 或 built-in provider override。
- `settings.json.defaultProvider/defaultModel/defaultThinkingLevel` 只表示默认启动选择，不表示唯一生效 provider。
- SQLite 只保存 Pi root 选择和 prompt presets；不要新增 `pi_provider` 或类似第二套 provider 主数据。

## 核心设计决策

- Pi 原生支持多 provider / model，产品形态按 OpenCode 的“运行时配置可视化”处理。
- 保存 provider 时只 upsert 当前 exact runtime key；如果 key 是 `anthropic`、`openrouter` 等官方内置 key，也是在原 key 上覆盖/补充，不生成 `ai-toolbox-*` 包装 provider。
- `defaultModel` 写 Pi 官方 settings 的裸 model id。model id 本身可能包含 `/`，不要拼成 OpenCode 风格的 `provider_id/model_id`。

## Gotchas

- 内置 provider 即使没有写入 `auth.json` 或 `models.json`，也可能通过环境变量或 Pi `/login` 可用；不要显示为 missing。
- `auth.json` OAuth token 是 Pi runtime-owned。AI Toolbox 可以识别和保留，但首版不编辑 token、不发起 `/login`。
- `models.json` 允许 unknown top-level 和 provider/model unknown fields。读写必须 preserve unknown fields。
- Pi MCP 暂不接入；不要在本模块里创建 MCP target 或写自造 `mcp.json`。

## 最小验证

- `settings.defaultProvider = "anthropic"` 且 `auth.json`/`models.json` 没有 `anthropic` 时，provider view 应标记 built-in/default，不是 missing。
- 同一个 key 同时存在 `auth.json` 和 `models.json.providers` 时，应合并成一条 provider view。
- 保存 `models.json.providers.<key>` 只覆盖该 key，其他 providers 和 unknown top-level 字段原样保留。
