# Oh My Pi 后端模块说明

## 一句话职责

- `oh_my_pi/` 只负责 OMP 运行时根目录和 `models.yml` provider 配置；`config.yml` 与认证数据库继续由 OMP 自己管理。

## Source of Truth

- OMP provider 的事实源是当前运行时根目录的 `models.yml`。
- OMP MCP server 主数据仍属于全局 MCP 模块，派生文件是当前运行时根目录的 `mcp.json`。
- 自定义根目录复用 `pi_settings_config` 表的独立 `oh_my_pi` 记录，与 Pi 的 `common` 记录隔离。

## Gotchas

- OMP 与 Pi 都识别 `PI_CODING_AGENT_DIR`，但应用内自定义根目录必须分别保存，不能切换 OMP 时覆盖 Pi root。
- `models.yml` 允许 override-only provider 和未知字段。按 provider key 写入时必须保留其他 provider 及未知字段。
- 不要接管 `config.yml`；默认模型、角色和 OMP 其他设置由 OMP TUI/CLI 维护。

## 最小验证

- 新增、修改、删除一个 provider 后，其他 provider 和未知字段保持不变。
- custom root 指向 WSL UNC 目录时，`models.yml` 与 MCP `mcp.json` 都从该目录派生。
