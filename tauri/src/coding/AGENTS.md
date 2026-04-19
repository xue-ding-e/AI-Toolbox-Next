# Coding 模块说明

## 一句话职责

- `tauri/src/coding/` 是 4 个内置 coding 工具、Skills、MCP、WSL/SSH 同步和运行时定位的共享后端域。
- 这里最重要的不是某个工具单独怎么存，而是跨工具共享的路径决议、事件约定和跨平台执行语义。

## Source of Truth

- 各工具的业务配置主数据分别存于 SurrealDB 和对应运行时配置文件，两者都重要，但“当前生效路径”不是简单看页面输入框，而是由 `runtime_location` 统一决议。
- `runtime_location` 是 4 个 tab 当前运行时位置、WSL Direct 状态和派生文件路径的唯一共享规则源。
- 对这 4 个 tab，`source` 与 `is_wsl_direct` 是两个独立维度：`source` 只说明路径来源，`is_wsl_direct` 只说明当前生效路径是否为 WSL UNC；`module_statuses` 来自后者，不来自页面展示。
- 4 个 tab 分成两类：OpenCode/OpenClaw 是“配置文件路径模块”，Claude/Codex 是“根目录模块”。后续 prompt、auth、plugins、skills 等派生路径都必须先尊重这个分层。
- `config-changed`、`wsl-sync-request-*`、`skills-changed`、`mcp-changed` 是跨模块联动的主事件契约；事件本身不保存状态，只触发后续动作。

## 核心设计决策（Why）

- 4 个 tab 的运行时路径统一收敛到 `runtime_location.rs`，避免每个模块各自判断 WSL UNC、默认路径和派生路径，导致逻辑分叉。
- 托盘刷新采用全局 `config-changed` 事件，而不是每个模块各自直接操作托盘，这样主窗口和托盘入口可以共享一套刷新机制。
- WSL 自动同步用 `lib.rs` 里的事件监听器集中触发，而不是在每个业务命令里直接调用 WSL 同步实现；这样可把“是否开启自动同步”判断统一放在监听器层。

## 关键流程

```mermaid
sequenceDiagram
  participant UI as Frontend
  participant Cmd as Tool Command
  participant DB as SurrealDB
  participant Runtime as Runtime Files
  participant App as lib.rs Listeners

  UI->>Cmd: save/apply config
  Cmd->>DB: update records if needed
  Cmd->>Runtime: write config/prompt/auth file
  Cmd-->>App: emit config-changed / wsl-sync-request-*
  App->>App: refresh tray / trigger WSL auto sync
```

## 易错点与历史坑（Gotchas）

- 不要把“页面上显示的 `source`”和“WSL/SSH 设置页里的 `moduleStatuses.is_wsl_direct`”混为一谈。前者是路径来源标签，后者是对当前生效运行时路径的统一诊断结果。
- 不要把 OpenCode/OpenClaw 与 Claude/Codex 按同一种“自定义配置”处理。前两者改的是文件路径，后两者改的是根目录；一旦混写，后续所有派生路径都会偏掉。
- 对 OpenCode、Claude Code、Codex、OpenClaw 这 4 个模块，文件 I/O 能直接读写 UNC 路径，不代表 CLI 也能直接吃 UNC 路径。新增 CLI 能力时必须先经过 `runtime_location::*_runtime_location_async` 判定。
- 对 OpenCode、Claude Code、Codex、OpenClaw 这类用户自行安装的 CLI，不要默认 GUI 进程里 `PATH` 可用。尤其 macOS 从 Dock/Finder 启动时，新增调用应优先解析已知安装位置或显式配置路径，再回退到 `PATH`。
- 新增跨工具共享规则时，优先放在共享层，不要把通用逻辑塞进某个单独工具目录，否则后续很快出现“相邻工具修了一边，另一边继续错”。

## 跨模块依赖

- 被 `wsl/`、`ssh/`、`skills/`、4 个工具模块依赖：它们都会消费 `runtime_location` 的派生路径或 WSL Direct 状态。
- 被前端 `web/features/settings/` 和 4 个工具页面间接依赖：前端展示的路径来源、WSL Direct 提示和同步跳过逻辑最终都依赖这里的后端状态。

## 典型变更场景（按需）

- 新增需要调用工具 CLI 的能力时：
  先检查 4 个内置工具是否都存在同类调用点，并确认本机/WSL Direct 两套执行路径。
- 新增新的跨模块事件时：
  先判断是否应复用现有事件契约；如果新增，必须同时梳理监听端和前端刷新端。

## 最小验证

- 改 `runtime_location` 后，至少验证一个本机路径场景和一个 WSL UNC 路径场景。
- 改事件约定后，至少验证主窗口保存、托盘刷新、WSL 设置页状态三者是否仍一致。
