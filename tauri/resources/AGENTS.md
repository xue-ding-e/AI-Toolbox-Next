# 模型资源说明

## 一句话职责

- `tauri/resources/` 存放会被后端在编译期嵌入的模型默认数据资源；这里的 JSON 不是运行时缓存副本，而是本仓随版本发布的 bundled defaults。

## Source of Truth

- `preset_models.json` 是预设模型默认数据的源码来源。`tauri/src/coding/preset_models.rs` 通过 `include_str!` 在编译期嵌入它；应用启动后前端先读 app data 里的缓存，再回退到这里的 bundled defaults，随后后台拉远端最新版本更新内存和缓存。
- `models.dev.json` 是 OpenCode 免费/官方模型默认数据的源码来源。`tauri/src/coding/open_code/free_models.rs` 同样通过 `include_str!` 在编译期嵌入它；运行时 app data 里的 `models.dev.json` 只是缓存，不是本仓编辑入口。
- 备份/恢复里读写的 `preset_models.json`、`models.dev.json` 都是 app data 缓存文件；不要把这些缓存链路误认为仓库内 `tauri/resources/*.json` 会被运行时直接原地改写。

## 核心设计决策（Why）

- 这两个 JSON 放在 `tauri/resources/`，是为了让应用在无网络、缓存缺失或远端拉取失败时仍有稳定的默认模型数据可用。
- `preset_models.json` 的数组顺序是用户可见语义，不只是排版。前端预设模型选择 UI 直接按分组数组顺序渲染，不会再做二次排序。

## 关键流程

```mermaid
sequenceDiagram
  participant Repo as tauri/resources/*.json
  participant Rust as Rust include_str! defaults
  participant Cache as App Data Cache
  participant UI as Frontend
  participant Remote as Remote JSON

  Repo->>Rust: compile-time embed
  UI->>Rust: load cached models
  Rust-->>UI: cache if valid, else bundled defaults
  UI->>Remote: background fetch latest preset models
  Remote-->>UI: latest JSON
  UI->>Cache: update cache
  UI-->>UI: refresh in-memory model lists
```

## 易错点与历史坑（Gotchas）

- 改 `preset_models.json` 时，不要只看“有没有这个模型”，还要看它在同组数组里的位置；顺序会直接影响预设标签展示顺序。
- 新增某个模型家族的新版本时，默认应放在对应旧版本前面，而不是机械地塞到整栏最前。比如 `qwen3.6-plus` 放在 `qwen3.5-plus` 前，`kimi-k2.6` 放在 `kimi-k2.5` 前。
- 给新模型补预设时，若需求是“参数和旧模型一样”，优先复制对应旧模型条目，只做 `id` / `name` 和必要顺序调整，不要顺手改能力字段。
- 不要在这里记录“远端缓存刷新后也许会覆盖本地顺序”之类推测；判断最终线上效果时，要先区分当前看到的是 bundled defaults 还是 app data / 远端缓存数据。

## 跨模块依赖

- `tauri/src/coding/preset_models.rs` 依赖 `preset_models.json` 作为编译期默认数据，并向前端暴露加载缓存与远端刷新命令。
- `tauri/src/coding/open_code/free_models.rs` 依赖 `models.dev.json` 作为 OpenCode 默认模型数据。
- `web/app/providers.tsx` 在启动时先加载 preset models 本地缓存，再异步拉远端并更新前端内存态。
- 备份恢复模块会单独备份和恢复 app data 下的 `preset_models.json` 与 `models.dev.json` 缓存文件。

## 典型变更场景（按需）

- 改 `preset_models.json` 时，至少检查：
  - 是否仍是合法 JSON。
  - 模型放在了正确分组。
  - 相对顺序是否符合用户可见展示语义。
- 改 `models.dev.json` 时，至少检查：
  - 变更是否真的是 OpenCode 默认模型数据，而不是应该改运行时缓存或远端源。
  - `tauri/src/coding/open_code/free_models.rs` 的默认读取路径和筛选语义是否仍成立。

## 最小验证

- 修改任一 JSON 后，至少做一次 JSON 合法性校验。
- 修改 `preset_models.json` 后，至少复核消费端是否仍按数组顺序直出，没有额外排序。
- 如果本轮同时改了缓存/远端刷新链路，还要额外区分 bundled defaults、app data cache 和 remote fetch 三条路径分别验证。
