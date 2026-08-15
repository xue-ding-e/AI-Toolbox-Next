# DeepSeek Harness (dsh) Backend Module 说明

## 一句话职责

- `dsh/` 负责 DeepSeek Harness 的配置目录解析与 `settings.yaml` / `.credentials.yaml` 的可视化管理：provider 增删改、`agent-default-model` 默认模型、Other Settings，以及全局提示词（写入 `<root>/AGENTS.md`）。

## Source of Truth

- dsh 用**单个 namespaced YAML 文件** `settings.yaml`（位于配置目录内），栈内按插件短名分节。本模块负责三类 section：
  1. `llm-pi-ai.providers.<route>`：供应商字典（key 即 route id），对应 Hermes 的 custom_providers。
  2. `agent-default-model`：默认模型 `{ provider, model, reasoningEffort }`。
  3. 其它 section（未知）保留。
- 凭据放在**独立的** `.credentials.yaml`（`REF: secret`，REF 为 POSIX 环境变量名，如 `DEEPSEEK_API_KEY`）。供应商只存 `apiKeyEnv` 引用，key 本体在凭据文件。
- 配置目录解析优先级：应用 DB `dsh_settings_config` 的 `common.config_dir`（source=`custom`）> 环境变量 `DSH_HOME`（`env`）> shell 配置（`shell`）> 平台默认（`default`）。平台默认：mac/Linux `~/.dsh`，Windows `%USERPROFILE%\.dsh`。
- SQLite 只保存配置目录选择（`common` 记录）与全局提示词预设（`dsh_prompt_config`）；**不要**新增 `dsh_provider` 之类第二套 provider 主数据。
- 本模块路径解析**不经过** `runtime_location`（dsh 尚未登记进该模块），而是内置在 `commands.rs`。source 语义与 `runtime_location` 对齐（`custom`/`env`/`shell`/`default`）。

## 核心设计决策

- provider 写入时只 upsert `llm-pi-ai.providers.<route>` 的 exact route；`models` 保持数组（`[{ id, contextWindow?, maxTokens? }]`）。
- 默认模型 `agent-default-model` 采用字符串字段「空串=删除键」语义（同 pi/hermes）。
- Other Settings 编辑器隐藏并保留托管键：`llm-pi-ai`、`agent-default-model`。
- 凭据写盘使用 0600 权限（参照 pi 的 `set_credentials_file_permissions`）。`save_dsh_credential` 传空 value 相当于删除该 ref。
- WSL/SSH 侧把 dsh 视为「配置文件路径模块」：`dsh-config`（settings.yaml）、`dsh-credentials`（.credentials.yaml）、`dsh-prompt`（AGENTS.md）三个默认文件映射，模块名 `dsh`。

## Gotchas

- 删除 provider 只删 `llm-pi-ai.providers.<route>`/空容器，不回滚 `agent-default-model` 默认选择；本地生效配置只在用户显式切换/应用时改写。
- 删除 prompt 预设只删 SQLite 记录，不改写/清空当前运行时 `AGENTS.md`。
- `settings.yaml` 允许未知 top-level 与 provider 未知字段；读写必须 preserve unknown fields。
- 保存 Other Settings 时不要把托管键（`llm-pi-ai`、`agent-default-model`）带回文件。
- 内置 provider 即使没有写进 `llm-pi-ai.providers`（凭 env/默认可用）也不应显示为 missing；凭据缺失显示为未配置而非 missing。
- dsh MCP 在部署层 cordis.yml，本模块首版把工具检测路径指向 `~/.dsh/settings.yaml`（`mcp_field = None`），不作为 MCP 配置主数据管理。

## 最小验证

- `settings.yaml` 已有 `llm-pi-ai.providers.<route>` 时，编辑该 route 后其它 provider 与未知顶层键保持不变。
- 默认 provider 不在 `llm-pi-ai.providers` 又非内置时，view 应标记 missing，`save`/`delete` 返回明确错误。
- 保存默认模型后，`agent-default-model` 的 `provider`/`model` 正确写入，`reasoningEffort` 空串时被删除。
- Provider 卡片编辑 apiKey 即写 `.credentials.yaml`，且文件权限为 0600（Unix）。
- 删除已保存 prompt 后，磁盘 `AGENTS.md` 内容保持不变。
