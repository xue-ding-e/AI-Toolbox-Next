# Codex 后端模块说明

## 一句话职责

- `codex/` 负责 Codex provider/common config、`config.toml`、`auth.json`、prompt、plugin 和官方账号相关运行时文件。

## Source of Truth

- 当前生效根目录优先级是：应用内 `root_dir` > 环境变量 `CODEX_HOME` > shell 配置 > 默认根目录。
- Codex 是“根目录模块”，`config.toml`、`auth.json`、`AGENTS.md`、`skills/` 都从当前根目录派生。
- prompt 的运行时事实源是当前根目录下的 `AGENTS.md`，而不是数据库记录本身。

## 核心设计决策（Why）

- `config.toml` 不能靠字符串拼接合并 common/provider 配置，必须结构化 merge，避免顶层键被吞进 provider 表作用域。
- `auth.json` 与 `config.toml` 混有 Codex runtime 自有字段；AI Toolbox 只能改受管字段，不能整文件覆盖运行时状态。
- `apply_config_internal` 统一负责写文件、更新 `is_applied`、发 `config-changed` 和 `wsl-sync-request-codex`。

## 关键流程

```mermaid
sequenceDiagram
  participant UI as Codex Page
  participant Cmd as codex::commands
  participant File as config.toml / auth.json / AGENTS.md
  participant DB as SurrealDB

  UI->>Cmd: apply provider/common config
  Cmd->>File: rewrite managed parts of config.toml and auth.json
  Cmd->>DB: update is_applied
  Cmd-->>UI: emit config-changed
  Cmd-->>UI: emit wsl-sync-request-codex
```

## 易错点与历史坑（Gotchas）

- 不要对 `config.toml` 做纯文本拼接。遇到 table 合并必须走结构化 TOML merge。
- 改写 `config.toml` 时要显式保留 runtime-owned sections，例如 `mcp_servers`、`features`、`plugins`。
- 改写 `auth.json` 时不要覆盖运行时 OAuth 字段；AI Toolbox 只应管理自己负责的 auth 键。
- WSL 自动同步是事件驱动，不是“数据库写成功就等于已经同步到 WSL”。

## 跨模块依赖

- 依赖 `runtime_location`：统一得到根目录、`config.toml`、`auth.json`、prompt、skills 路径与 WSL 目标路径。
- 被 `web/features/coding/codex/` 依赖：页面通过 `get_codex_root_path_info()` 和 provider/prompt API 管理状态。
- 被 `wsl/`、`ssh/`、`mcp/` 间接依赖：它们都受 `config.toml` 路径和保留段语义影响。

## 典型变更场景（按需）

- 改 `config.toml` 落盘逻辑时：
  同时检查结构化 merge、runtime-owned sections 保留、WSL 同步事件和最小回归测试。
- 改 root_dir 逻辑时：
  同时检查 `auth.json`、`config.toml`、`AGENTS.md`、Skills 路径和前端 path info 展示。

## 最小验证

- 至少验证：common/provider 合并后顶层键仍在根级，表结构未错位。
- 至少验证：编辑已应用配置后仍会发出 `wsl-sync-request-codex`。
- 至少验证：prompt 应用会改写当前根目录下的 `AGENTS.md`。
