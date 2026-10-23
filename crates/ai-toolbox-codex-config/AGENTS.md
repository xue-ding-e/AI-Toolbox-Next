# Codex 纯配置组合模块

## Source of Truth

- 本 crate 是从桌面 Codex commands 原样抽出的 TOML parse 和 common/provider merge 唯一实现。保留原项目作者归属与 AGPL-3.0-or-later 许可证。
- 桌面 commands 直接导入这里的实现；不得为 CLI、MCP 或 GUI 再写另一份合并器。

## Why

- 让无 Tauri、无显示服务的调用方复用相同纯算法，先分离可验证的配置组合，再抽真实业务服务。
- common 是默认值，provider 是显式 overlay；仅普通 table 递归合并，其他 item（包括 array 和 inline table）整体取 provider。

## Gotchas

- 这是纯文本组合，不是运行文件投影服务。runtime-owned 清理、previous-managed 差分、auth、catalog、数据库、事件、Gateway 和 owner 生命周期仍由产品业务层负责。
- 为保持桌面行为，此次抽取保留单侧为空时 trim 后直接返回的快捷分支，它不校验另一侧 TOML。未来 typed patch plan 必须先独立校验两份输入，不能把该 helper 当输入验证器。
- toml_edit 保留未被覆盖 item 的格式和注释；不要承诺所有来源注释逐字保留，overlay 替换 item 以及现有 trim 都有明确语义。
- 不得增加文件 I/O、环境读取、日志打印、网络、auth 或 Tauri 依赖。纯函数结果不代表已保存、已应用或已生效。

## 最小验证

- 纯库：`cargo test --manifest-path crates/ai-toolbox-codex-config/Cargo.toml`。
- 桌面原有回归：`cargo test --manifest-path tauri/Cargo.toml --lib append_toml_configs`；修改组合行为还须运行完整 Codex 回归与仓库全量闸门。
- storage/apply/verify、profile owner 和 CLI/MCP 业务入口仍未完成；纯计划不能返回 saved/applied/verified 成功。
