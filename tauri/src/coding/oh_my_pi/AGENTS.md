# Oh My Pi 后端模块说明

## 一句话职责

- `oh_my_pi/` 负责 OMP(Oh My Pi)运行时根目录、`models.yml` provider 配置与 `config.yml` 设置的可视化管理;`config.yml` 与认证数据库继续由 OMP 自己管理。

## Source of Truth

- OMP provider 的事实源是当前运行时根目录的 `models.yml`(YAML),自定义根目录存 `oh_my_pi_settings_config` 表的 `common` 记录。
- OMP 设置的事实源是当前运行时根目录的 `config.yml`(YAML,点分 camelCase 键)。
- OMP MCP server 主数据仍属于全局 MCP 模块,派生文件是当前运行时根目录的 `mcp.json`。
- 全局提示词预设存 `oh_my_pi_prompt_config` 表,写入运行时根目录的 `AGENTS.md`。
- 文件式预览由 `read_omp_runtime_config` 返回原始文件内容（`configContent`/`modelsContent`/`mcpContent`/`promptContent`），前端按文件 Tab 展示，与 Codex 一致。

## 与 Pi 的差异

- OMP 没有 `auth.json`/`models.json`/`settings.json`;凭据(apiKey)直接写在 `models.yml` 的 provider 配置里,默认模型用 `modelRoles.default`(格式 `provider/modelId`)表达,思考级别用 `defaultThinkingLevel`。
- OMP 扩展是 `omp plugin` 系统(plugins),不是 Pi 的 `extensions` 命令;本地扩展目录是 `<root>/extensions`。
- OMP 的 skills 由 native 能力(priority 100)从 `<agentDir>/skills`(即 `~/.omp/agent/skills`)发现,应用把 skills 同步到该目录;不是 agents 能力(priority 70,可被 `skills.enableAgentsUser` 关闭)的 `~/.agents/skills`。
- OMP 与 Pi 都识别 `PI_CODING_AGENT_DIR`,但应用内自定义根目录分别保存。

## Gotchas

- `models.yml` 允许 override-only provider 和未知字段。按 provider key 写入时必须保留其他 provider 及未知字段。
- 写入 `modelRoles.default` 时必须是 `provider/modelId`(OMP `parseModelString` 按首个 `/` 拆分),裸 provider 无效。
- 不要接管 `config.yml` 全部字段;其他设置编辑器隐藏并保留 `modelRoles`/`defaultThinkingLevel`/`extensions`/`enabledModels` 等受管键。
- `defaultThinkingLevel` 的“清除”由 `OmpModelSettingsInput.clear_thinking_level` 显式驱动；前端空字符串不代表清除，避免用户在切换 provider/model 时误删全局思考级别。
- OMP 的 `thinking.mode` 是其 schema 的必填字段(`ThinkingControlModeSchema`:effort/budget/google-level/anthropic-adaptive/anthropic-budget-effort)。生成带 `thinking` 块的模型时若缺 mode,整个 models.yml 校验失败、所有自定义 provider 被禁用。前端 `buildOmpThinkingFromPreset(variants, api)` 按 api 推断 mode(google 系→google-level、anthropic-messages/bedrock→anthropic-adaptive、其余→effort);后端 `normalize_omp_provider_for_omptype` 对旧数据/手写 JSON 缺 mode 时同样兜底补上。
- provider `api` 的合法词表是 omp `ApiSchema` 的 9 个值(见前端 `web/features/coding/oh_my_pi/utils/ompApiOptions.ts`,镜像 oh-my-pi `models-config-schema-bundle.ts`);未知 `api` 值会让整个 models.yml 校验失败、禁用所有自定义 provider。但 omp 的 `Api` 类型对扩展开放(可注册自定义 API),因此前后端都不对 `api` 做枚举硬校验,保持字符串透传;provider 表单 Select 只提供词表选项,自定义值走原始 JSON 编辑。
- WSL 场景下选中 `~/.omp` 目录且其 `agent` 子目录为有效运行时布局时,归一化为 `~/.omp/agent`。
- OMP 的持久化端点与共享诊断端点分开：Anthropic 在诊断时补 `/v1`，Gemini 补版本路径，不能反写 `models.yml`。`openai-codex-responses` 诊断显式携带 apiFormat，使用 Codex 请求/终态契约；Azure、Bedrock、Gemini CLI、Vertex 及自定义 API 暂无对应诊断适配，界面禁用并说明，不能降成 Chat Completions。模型连接一致时可用模型覆盖值，不同连接混用时禁用供应商级诊断。
- 新建供应商的默认地址由表单记录自动填值来源，API 切换只更新仍由表单自动填入的地址；用户编辑或主动清空后停止自动修改。编辑/复制现有供应商不自动填地址，重新打开新建弹窗才重置自动填值状态。

## 最小验证

- 新增、修改、删除一个 provider 后,其他 provider 和未知字段保持不变。
- 保存默认模型后 `config.yml` 的 `modelRoles.default` 为 `provider/modelId`。
- 安装 `omp` 后运行 `omp plugin list --json` 可列出插件。
