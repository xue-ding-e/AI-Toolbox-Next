# Antigravity Frontend Module Guide

## Source of Truth

- Antigravity 前端页面入口为 `AntigravityPage.tsx`，路由对应 `/coding/antigravity`。
- 页面只面向 Google 官方 OAuth 账号管理，不向用户暴露自定义 API Key、Base URL 或供应商列表；根目录设置、全局提示词和会话管理仍保留。
- 类型定义位于 `web/types/antigravity.ts`。
- API 调用集中在 `web/services/antigravityApi.ts` 与 `web/services/antigravityPromptApi.ts`。

## Key Decisions

1. 视觉风格严格遵守根目录 `DESIGN.md`，使用 Ant Design 原生 Modal chrome 与 CSS Modules。
2. 内部官方 provider 仅作为账号归属和运行时状态载体，前端不提供新增、编辑、删除、排序或 API Key 设置入口。
3. 官方账号 OAuth、配额、凭证导入和应用仍通过 `AntigravityProviderCard.tsx` 展示。
4. 账号列表始终保留“默认配置”虚拟账号；它代表设备当前 Antigravity CLI 配置，不要求已有登录，且可在 OAuth 账号之间切换回来。
   默认登录快照由后端在每次离开默认配置前更新；在其他账号下新建 OAuth 账号不会覆盖它。旧版空快照无法确认原始登录时，切回默认会返回错误并保留当前登录，前端必须展示该错误，不能把失败当成切换成功。
5. 全局提示词写入 `agy` 真正的全局规则文件 `~/.gemini/config/GEMINI.md`（不跟随 `settings.json` 的 `context.fileName`，也不跟随自定义根目录）；本页不展示 `oauth_creds.json`，登录凭证实际存放在操作系统凭据库 `gemini:antigravity`。
6. `disableConfig` 会向 `disable_antigravity_prompt_config` 传入 `configId`（不是 `id`），这是后端强校验的参数名。
7. “Antigravity CLI 登录”按钮在 OAuth 期间进入 loading 并禁用，期间再点不再发请求（后端对同一时刻的并发 OAuth 也是拒绝的）；回调页只提示“已收到授权，请返回应用完成登录”，最终成功/失败以后端命令返回为准，避免在换取令牌前误报成功。
8. 页面头部必须与 Claude Code / Codex 保持同构：第一行是标题 + 内联的“官方文档”“预览配置”链接与右侧“更多选项”，第二行是配置路径 + “自定义配置目录 / 打开文件夹 / 刷新配置”。不要再给页面 body 套 `maxWidth` / `padding` 容器，`MainLayout` 的 `.contentArea` 已提供内边距。
9. 虚拟默认账号的显示名必须在前端本地化：后端把它的 `name` 硬编码为中文 `"Antigravity CLI 默认配置"`，因此 `account.isVirtual` 为 true 时要渲染 `t('antigravity.provider.officialAccountLocal')`，不能直接用 `account.name`。
