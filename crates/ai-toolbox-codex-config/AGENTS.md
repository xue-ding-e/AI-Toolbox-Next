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
- `plan_context_patch` 是两个显式正 i64 根键的纯候选计划。分别校验 common/provider 主数据及最终 effective 阈值，避免 provider 暂时遮蔽无效 common 数据；未设置的 context 不从模型缓存虚构默认值。
- candidate 包含完整主数据文本，可能包含敏感字段，只供服务内部使用；未来 CLI 默认输出不得直接序列化它，须用 typed/redacted diff。
- 只读 override inspection 与 patch plan 共用同一层校验和 effective/source 解释 helper，继续调用唯一 merger；inspection 只返回两项显式值，不返回原文/candidate，不把空 patch 变成有效写请求，也不推断模型默认值。
- storage/apply/verify、profile owner 和 CLI/MCP 业务入口仍未完成；纯计划不能返回 saved/applied/verified 成功。
- `plan_context_projection` 仅在调用方提供的 live TOML 文本上计划两个 context 根键；复用 `plan_context_patch` 的 effective 值及同一个保留 decor 的 set，不能调用会清理认证字段的完整桌面投影函数。
- `ContextPatch` 的 `None` 表示不请求修改，不是删除；只有明确请求的键可以新增或更新。common 被 provider 遮蔽时仍按 effective provider 值投影，并保留 shadowed 说明；不能为纠正 live 漂移修改未请求键。
- 投影后的 live 两数值及阈值也须校验，错误时不返回部分候选。`master` 与 `live_candidate/live_changed` 分开表达主数据和 live 计划；后者同样可能包含敏感字段，不得默认输出。本函数不读写 auth.json，不接 storage、owner 或运行文件写入。
- live 中已有的 context 键须先通过正整数校验，即使本次明确请求该键，也不隐式修复非法值或替换整表丢失子项。已有两键都是合法正整数、仅阈值关系不合法时，可以由本次请求纠正，最终组合仍须通过校验。
