# Hermes Agent 后端模块说明

## 一句话职责

- `hermes/` 负责 Hermes Agent 的配置目录解析与 `config.yaml` 的可视化管理:provider 增删改、顶层 `model:` 默认模型、Other Settings、全局提示词(写入 `<root>/SOUL.md`)、以及 memory 记忆文件(`memories/MEMORY.md`/`USER.md` 内容编辑 + `memory:` 段启用开关)。

## Source of Truth

- Hermes 用**单个 YAML 文件** `config.yaml`(位于配置目录内),它是 provider、model、agent 设置等的唯一运行时事实源;凭据(api_key)也写在其中的 `custom_providers` 里。
- 配置目录解析优先级:应用 DB `hermes_settings_config` 的 `common.config_dir`(source=`custom`) > 环境变量 `HERMES_HOME`(`env`) > shell 配置(`shell`) > 平台默认(`default`)。平台默认:Windows `%LOCALAPPDATA%\hermes`,macOS/Linux `~/.hermes`。
- provider 事实源是 `config.yaml` 的两处:可写的 `custom_providers:` 列表(累加式,按 `name` 键)与只读的 `providers:` 字典(Hermes v12+,本模块只合并展示、不写)。
- MCP server 主数据属全局 MCP 模块,`mcp_servers:` 段由 `mcp::hermes_mcp` 适配器同步(serde_yaml round-trip,merge-on-write 保留 Hermes 专有字段 `enabled`/`timeout`/`tools`/`sampling`/`roots`/`auth`,import 时剥离);本模块不直接写 `mcp_servers:`。
- Hermes 有一等 skills 系统:`<hermes_root>/skills` 是单源目录,SKILL.md 兼容 agentskills.io 标准(与 Claude Code 同一份 `name`+`description` frontmatter)。skills 路径由 `tools::detection::resolve_special_skills_path` 解析,复用 `config.yaml` 同一平台根目录(Windows `%LOCALAPPDATA%\hermes\skills`),不直接用静态 `~/.hermes/skills`(Windows 会误解析到 `%USERPROFILE%\.hermes`)。
- SQLite 只保存配置目录选择(`common` 记录)与全局提示词预设(`hermes_prompt_config`);**不要**新增 `hermes_provider` 之类第二套 provider 主数据。
- 配置目录优先级由 `commands.rs` 的同一解析器提供给文件操作与 `runtime_location`；WSL UNC 识别、`module_statuses` 和同步跳过集合统一由 `runtime_location` 产出。保留 `custom`/`env`/`shell`/`default` 优先级，避免为同步另建一套路径规则。
- 文件式预览由 `read_hermes_runtime_config` 返回原始 `config.yaml`/prompt 文件内容（`configContent`/`promptContent`），前端按文件 Tab 展示，与 Codex 一致。

## 核心设计决策

- `config.yaml` 的 `model:` 顶层段写 provider/default/base_url/context_length/max_tokens;保存时只动这些托管键,保留 `model:` 段内未知键。字符串字段按「空串=删除键」处理,数字字段用显式 `clear_*` 标记删除。
- Other Settings 编辑器隐藏并保留托管键:`model`、`custom_providers`、`providers`、`mcp_servers`、`_config_version`,其余顶层键(如 `agent`)可编辑。
- provider 写入时把 `models` 从 UI 数组归一化为 Hermes 的 dict(按 id 作 key),并把首个模型 id 写入单数 `model:` 字段(Hermes 运行时与 `/model` 选择器都读它)。
- `providers:` 字典里但不在 `custom_providers:` 里的 `providers:` 条目视为只读 overlay,`save`/`delete` 一律拒绝并提示走 Hermes UI。
- 删除 provider 只改 `custom_providers`/DB 记录,不回滚顶层 `model:` 默认选择;本地生效配置只在用户显式切换/应用时改写。

## Gotchas

- 删除 prompt 预设只删 SQLite 记录,不改写/清空当前运行时 `SOUL.md`,避免把本地生效提示词一并清掉。
- 保存/清除 `config_dir`（`save_hermes_settings_config`）后，必须先刷新 Hermes 的 runtime location 缓存，再发 `wsl-config-changed`、`emit_config_changed`（含 `wsl-sync-request-hermes`）和 `skills-changed`。Hermes 的 Skills 目标随配置根变化（`<root>/skills`）；遗漏刷新或事件会让状态与实际文件路径分叉。
- `config.yaml` 允许未知 top-level 与 provider/model 未知字段;读写必须 preserve unknown fields。
- 不要接管 `providers:`(v12+ dict);它由 Hermes 自身的 overlay 语义维护,写了会被 Hermes 覆盖或破坏。
- 保存 Other Settings 时不要把托管键(model/custom_providers/providers/mcp_servers/_config_version)带回文件;并且**只 upsert 提交的键,保留未提交的顶层键**——不再做"全量替换非保护键",避免过期前端/并发写把 `memory`/`display` 等新键静默删掉(编辑器里删掉某个顶层键不会持久化删除)。
- 写入 `config.yaml` 统一走 `write_yaml_sections_with_backup`:**段落级文本替换**(`replace_yaml_section`,只改目标顶层段,保留其余文件字节原样含注释),结果与原文相同则 no-op 不落盘;每次写前在 app data `backups/hermes/` 留时间戳备份(保留 10 份)+ 进程级 `hermes_write_lock` 防 TOCTOU。读取(及写入目标段定位)前用 `deduplicate_top_level_keys`/`remove_all_sections` 愈合并发清理旧工具留下的重复顶层键(serde_yaml 会拒绝)。
- 内置 provider 即使不在 `custom_providers`/`providers` 里(凭 env/默认可用)也不应显示为 missing。
- `models` dict 的 key 即 model id;读写 dict↔array 转换必须保留顺序与上下文(context_length 等),`id` 不能泄漏进 dict value。
- WSL Direct 时文件、MCP、Skills 的 WSL 自动同步都要跳过 Hermes 的重复目标处理；中央 Skills 到当前 UNC 运行时目录的本地同步仍然有效。SSH 不因 WSL Direct 跳过，其配置与 Skills 继续使用远端默认 `~/.hermes` 布局。
- Hermes 有一等 skills 系统(`<root>/skills`,SKILL.md 兼容 agentskills.io 标准),已注册进 `BUILTIN_TOOLS` 的 `relative_skills_dir`,路径经 `resolve_special_skills_path` 复用 `config.yaml` 同一平台根目录。MCP 同步由 `mcp::hermes_mcp` 独立处理。

## 最小验证

- `cargo test --test coding wsl_direct_status` 覆盖保存/清除目录后的 WSL 状态往返、环境变量优先级与 Hermes 自定义 Skills 路径；WSL MCP 过滤回归另见 `cargo test --lib coding::wsl::mcp_sync::tests`。
- `config.yaml` 只有一个 `custom_providers` 条目时,新增/修改/删除该条目后,其他未知顶层键(如 `agent`、`_config_version`)保持不变。
- 默认 provider 不在 `custom_providers` 又在 `providers:` 字典里时,view 应标记 read-only,`save`/`delete` 返回明确错误。
- 保存默认模型后,`model:` 段的 `default`/`provider` 正确写入,`context_length`/`max_tokens` 在 `clear_*` 时被删除。
- 写 `custom_providers[].models` 为 dict、写回后仍能还原为按 id 排序的数组。
- 删除已保存 prompt 后,磁盘 `SOUL.md` 内容保持不变。
