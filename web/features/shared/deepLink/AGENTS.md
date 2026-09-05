# DeepLink 前端模块说明

## 一句话职责

- `deepLink/` 承载 `aitoolbox://` 供应商深链接的前端两侧：导入确认（`DeepLinkImportDialog` + `useDeepLinkImport`，挂在 `web/app/providers.tsx`）与分享链接生成（`providerShareUrl.ts`，被 `web/features/coding/shared/providerShare/ShareProviderModal` 消费）。

## Source of Truth

- URL 格式（scheme、version、path、合法参数集、app 白名单）的唯一事实源是后端 `tauri/src/coding/deeplink/parser.rs`。本目录只镜像 `aitoolbox://v1/import` 常量；后端改协议时必须同步这里。
- 导入的写入路径只有后端 `import_from_deeplink_unified`；前端导入侧不做任何持久化。
- 分享 URL 的生成是纯前端逻辑，无 Tauri 命令、无落库。

## 核心设计决策（Why）

- 分享链接只携带通用连接字段（name/category/apiKey/baseUrl/model/homepage/notes/icon/iconColor），**永不携带** `config`/`extra` base64 块。跨工具协议转换由接收方的导入 builder（后端 `deeplink/provider.rs` 的 `build_claude/codex/gemini_settings`）按 `app` 参数自动完成，因此同一个链接可以被导入三个工具中的任意一个。
- 不带 config blob 同时规避了两个问题：Windows 上 OS scheme URL 超长截断（全量 settings_config base64 后很容易超限），以及把源工具的 TOML/JSON 形状塞进错误 app 导致接收方写出非法配置文件。
- 同工具的无损复制不归分享链接管：卡片菜单已有的“复制”（`_copy` 克隆）承担同工具完整配置复制语义。

## Gotchas

- `URLSearchParams` 与 Rust `url::Url::query_pairs()` 都遵循 `application/x-www-form-urlencoded`（`+` 即空格），两侧编码语义兼容；不要换用手写的 `encodeURIComponent` 拼接，容易在保留字符上分叉。
- `homepage` 只在 `http(s)://` 开头时写入链接；后端 parser 对非 http/https 直接报 `InvalidUrl`，生成端过滤可以让接收方导入永不因主页字段失败。
- Claude 源的 `model` 提取走 `getClaudeConfiguredModelIds` 的 fallback 链（`ANTHROPIC_MODEL` → sonnet → opus → fable → haiku → legacy reasoning），并剥离 Claude 专属的 `[1M]` 上下文后缀。只读 `ANTHROPIC_MODEL` 是踩过的坑：很多 provider 只配角色模型不配默认模型，跨工具导入会得到空 model。
- `maskApiKey`（4 字符 + 20 星号）必须与 `DeepLinkImportDialog` 内的脱敏格式保持一致，用户在分享侧和导入侧看到的是同一约定。
- `providerShareUrl.ts` 导入 `codexConfigUtils` 用的是**相对路径**而不是 `@/` alias：node:test loader（`scripts/node-ts-extension-loader.mjs`）不解析 tsconfig paths，凡是被 `web/test/**` 直接或间接加载的文件都要用相对路径（先例：`coding/codex/utils/codexSettingsConfig.ts`）。
- v1 导入白名单只有 `claude/codex/gemini`（grok 被 parser 刻意拒绝）；分享弹窗的目标工具选择也只列这三个。扩展新 app 需要后端 parser 白名单 + provider builder + 前端 `ProviderShareApp`/`SHARE_TARGET_APPS` 三处同步。

## 最小验证

- `node --import ./scripts/register-node-ts-extension-loader.mjs --test web/test/features/shared/deepLink/providerShareUrl.test.ts`（或全量 `pnpm test`）覆盖 URL 生成、三种工具的字段提取和 Codex TOML 提取。
- 手工验证：任一渠道卡片 → 更多菜单 → 分享 → 复制链接 → 在浏览器地址栏粘贴回车，应弹出导入确认弹窗且字段与分享侧预览一致。
