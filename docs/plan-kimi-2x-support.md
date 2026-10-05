# Kimi Code CLI 2.x 支持增强方案（issue #398）

> 立项日期：2026-10-05
> 上游基线：Kimi Code CLI **2.1.1**（npm `@moonshot-ai/kimi-code`，本机实测）
> 项目现状基线：`docs/plan-kimi-code-cli.md` 基于 **0.39.1**（2026-07-15）
> 关联 issue：https://github.com/coulsontl/ai-toolbox/issues/398
> 范围决策：**方案 B**——聚焦 issue 三诉求 + 高频字段，其余留给 Common Config 手写

---

## 1. 背景与问题

Issue #398 对 Kimi CLI 支持提出三点不足，均已在本机 2.1.1 实测确认：

1. **不能拉取模型**——Kimi provider 表单的模型目录只能手填
2. **没有预设模型的映射表**——新建 provider 只有一条硬编码默认模型
3. **不能定制蜂群模式的多模型自主选择**——`[secondary_model]` 无 UI 入口

此外，CLI 已从 0.39.1 迭代到 2.1.1，模型条目 schema 新增字段，其中两个由官方 `provider catalog add` 主动写入，当前投影链路不识别。

---

## 2. 上游事实（2.1.1 实测）

### 2.1 CLI 非交互能力

| 命令 | 作用 |
|------|------|
| `kimi provider catalog list [providerId]` | 列出 models.dev 目录（226 个 provider），带 providerId 时列出其模型 |
| `kimi provider catalog add <providerId> [--api-key]` | 导入 provider + **全部模型**（含 context/output/efforts/reasoning_key） |
| `kimi provider add <url> [--api-key]` | 导入自定义 registry（api.json） |
| `kimi provider list [--json]` | 列出已配置 provider 及模型数；`--json` 输出原始配置 |
| `kimi provider remove <providerId>` | 移除 provider 及引用它的模型别名 |
| `kimi session list` | 非交互列出会话 |
| `kimi doctor` | 校验配置文件（**仅语法**，不做语义校验） |

`provider catalog add` 实测写入格式（`KIMI_CODE_HOME` 隔离目录验证）：

```toml
[providers.moonshotai]
base_url = "https://api.moonshot.ai/v1"
type = "openai"
api_key = "<key>"

[models."moonshotai/kimi-k3"]
provider = "moonshotai"
model = "kimi-k3"
max_context_size = 1048576
max_output_size = 1048576
capabilities = [ "image_in", "video_in", "always_thinking", "tool_use" ]
display_name = "Kimi K3"
reasoning_key = "reasoning_content"
support_efforts = [ "low", "high", "max" ]
```

### 2.2 `[secondary_model]`：蜂群模型池

来自 `packages/agent-core-v2/src/session/subagent/configSection.ts`：

```ts
SecondaryModelConfigSchema = ModelAliasOverrideSchema.extend({
  model: string().min(1).optional(),        // v1 遗留键，作为 default_model 兜底
  default_model: string().min(1).optional(), // 子 agent 默认模型
  models: record(string(), string()).optional(), // 池：主 agent 可自主挑
  force: boolean().optional(),               // 钉死到 default_model，剥夺选择权
})
```

**运行时行为**（`AgentSwarmTool.parameters` getter）：

```js
exposesSubagentModelChoice(config) {
  if (isSubagentModelForced(config)) return false;          // force=true → 不暴露
  return resolveSubagentModelPool(config) !== undefined;    // 有池 → 暴露
}
```

- 池存在且未 force → `AgentSwarm` 工具**多出 `model` 参数**，主 agent 可自主为子 agent 选模型
- `model` 参数取值：池中别名，或保留字 `"primary"`（= 主 agent 自身模型与思考等级）
- 未配置池 → 工具参数剥离 model 字段，走全局默认

**校验约束**（`assertValidSubagentModelPool`）：

| 约束 | 错误消息 |
|------|---------|
| `force=true` 与 `models` 互斥 | `[secondary_model].force cannot be combined with [secondary_model].models` |
| `models` 非空时必须有 `default_model` | `[secondary_model].default_model is required when [secondary_model].models is configured` |
| `force=true` 时必须有 `default_model` | `[secondary_model].default_model is required when [secondary_model].force is set` |
| `models` 的 key `"primary"` 是保留字 | `[secondary_model.models] key "primary" is reserved` |

**已知限制**（沿用 `proxy_gateway/AGENTS.md:127`）：池中引用的其他 provider 不经过网关，与其他 CLI 的单点接管语义一致。

### 2.3 模型条目 schema（`ModelAliasBaseSchema`）

| 字段 | 类型 | 项目支持 |
|------|------|---------|
| `provider` / `model` | string 必填 | ✅ |
| `max_context_size` | int ≥1 **必填** | ✅ |
| `max_input_size` | int ≥1 | ❌ |
| `max_output_size` | int ≥1 | ❌ **catalog add 会写** |
| `capabilities` | string[] | ✅ |
| `display_name` | string | ✅ |
| `reasoning_key` | string | ❌ **catalog add 会写** |
| `support_efforts` / `default_effort` | string[] / string | ✅ |
| `protocol` / `adaptive_thinking` / `off_effort` / `beta_api` / `base_url` / `overrides` | — | ❌（本次不做） |

### 2.4 顶层 schema（`KimiConfigSchema`）

本次纳入范围：`secondary_model`。
本次不做（留给 Common Config 自由 TOML）：`default_provider`、`model_catalog`、`plan_mode`、`yolo`、`default_plan_mode`、`default_permission_mode`、`auto_session_title`、`experimental`。

---

## 3. 目标与非目标

### 3.1 目标

1. Kimi provider 表单支持从上游 API 拉取模型列表（复用共享 `fetch_provider_models`）
2. 新建 provider 时按 base_url 匹配 models.dev 目录，一键带出候选模型（**离线可用**，用仓库已有 `models.dev.json`）
3. 模型目录支持 `max_input_size` / `max_output_size` / `reasoning_key` 三个字段
4. 新增 `[secondary_model]` 结构化编辑入口（蜂群模型池）

### 3.2 非目标

- harness 解耦成插件包（issue 的架构建议，本次不处理）
- `protocol` / `adaptive_thinking` / `off_effort` / `beta_api` / `base_url` / `overrides` 等模型级低频字段
- `default_provider` / `model_catalog` 等顶层低频字段
- 调用 `kimi provider catalog add` CLI（只读仓库内嵌目录，不引外部进程）

---

## 4. 设计

### 4.1 需求 1：拉取模型

**复用**：`web/components/common/FetchModelsModal` + 后端 `fetch_provider_models`（`tauri/src/coding/open_code/models_api.rs:557`）。

**接入方式**（对齐 Codex 的卡片内按钮 + 页级 Modal 模式，`CodexProviderCard.tsx:809` + `CodexPage.tsx:2894`）：

- Kimi provider 的 API 协议统一是 OpenAI 兼容（`type = "openai"`），因此固定传 `apiType: 'openai_compat'`、`sdkType: '@ai-sdk/openai'`，URL 为 `{base_url}/models`
- provider 卡片模型区标题栏加「获取模型」按钮（`onFetchModels`），点击后打开共享 `FetchModelsModal`
- 返回的 `FetchedModel[]` 映射为 `KimiCatalogModel[]`：

| 来源 | 目标字段 |
|------|---------|
| `model.id` | `key`（前缀 `{providerKey}/`）与 `model` |
| — | `maxContextSize`：**无法从 API 获得，必须兜底** 262144 |
| — | `provider`：当前 providerKey |

- 已存在的 key 不重复添加；拉取结果与现有目录合并，用户可在表格中继续编辑
- `baseUrl` 为空时按钮禁用并提示（对齐 Gemini CLI 的 `fetchModels.baseUrlRequired`）

**边界**：
- 拉取是**纯读操作**，不写 `config.toml`，因此**不受 Gateway 接管门禁限制**
- 拉取失败不阻断表单（提示后用户仍可手填）

### 4.2 需求 2：预设模型映射表（离线候选）

**数据源**：仓库已有 `tauri/resources/models.dev.json`（1.9MB，119 provider），**与 kimi CLI `provider catalog` 同源**。

**新增后端命令**：`get_kimi_preset_models(state, base_url: String) -> Result<Vec<KimiPresetModel>, String>`

匹配规则（按优先级）：

1. 归一化 `base_url`（去尾斜杠、小写、去 `https://` 前缀）后，与 `models.dev.json` 各条目的 `api` 字段精确匹配
2. 无精确匹配时，用域名做模糊匹配（如 `api.moonshot.cn` → `moonshotai-cn`）
3. 仍无匹配返回空数组——**不做 provider 名称模糊匹配**（避免误匹配）

返回结构（只暴露 Kimi 需要的字段）：

```rust
pub struct KimiPresetModel {
    pub id: String,               // 来自 models.dev 的 model key
    pub name: Option<String>,     // models.dev 的 name（display_name）
    pub max_context_size: Option<i64>, // models.dev 的 limit.context
    pub max_output_size: Option<i64>,  // models.dev 的 limit.output
    pub reasoning: bool,          // models.dev 的 reasoning
    pub tool_call: bool,
    pub modalities: Option<Value>, // 用于推导 capabilities
}
```

**前端交互**：新建 provider 时，`base_url` 输入框失焦后自动查询（防抖 300ms）；命中则展示「已匹配到 N 个预设模型」提示 + 「导入全部」/「选择导入」按钮。

**注意**：`models.dev.json` 是 OpenCode 的运行时缓存文件，由 `open_code/free_models.rs` 管理（含远端刷新）。本方案**只读**该文件，不改变其生命周期。缓存文件路径由 `free_models::get_models_cache_path()` 暴露。

### 4.3 需求 3：模型目录新增三字段

**核心结构调整**：模型目录从 provider 编辑弹窗的**内联表格**，改为 Codex 模式的**卡片内列表 + 独立单模型编辑弹窗**。这是本次最大的结构改动，理由：

- 3 个新字段会把内联表格推到 7 列，720px 宽度无法容纳
- Codex 已验证该模式（`CodexProviderCard` 的模型区 + `CodexModelFormModal`），交互一致性好
- 单模型弹窗有充足空间，后续加字段无需再改布局

**改造内容**：

1. **`KimiProviderFormModal.tsx` 精简**：移除模型目录表格与相关状态（`catalogModels`、`handleUpdateModel`、`modelColumns` 等），只保留 name / category / apiKey / baseUrl / defaultModelKey / notes / 高级 JSON
2. **`KimiProviderCard.tsx` 新增模型区**（对齐 `CodexProviderCard.tsx:760-860`）：
   - 标题行：`模型 (N)` + 「获取模型」「添加模型」「批量删除」按钮
   - 展开区：模型行列表（key / displayName / 上下文大小），点击行打开编辑弹窗
3. **新增 `KimiModelFormModal.tsx`**（对齐 `CodexModelFormModal.tsx`）：
   - 字段：`key` / `model` / `displayName` / `max_context_size` / `max_input_size` / `max_output_size` / `capabilities` / `reasoning_key` / `support_efforts` / `default_effort`
   - 保存走 provider 的 catalog 持久化链路
4. **新增 `utils/kimiCatalogModels.ts`**（对齐 `codex/utils/codexCatalogModels.ts`）：catalog 的 upsert / 删除 / 行 key 生成等纯函数

**后端**（`tauri/src/coding/kimi/commands.rs`）：

- `insert_known_model_fields`（约 1860 行）增加三个字段的写回：
  - `maxInputSize` → `max_input_size`
  - `maxOutputSize` → `max_output_size`
  - `reasoningKey` → `reasoning_key`
- `__local__` 收编读取路径（约 1255 行的 `target_field` match）增加对应映射，使 CLI 写入的值能被读回
- 三者都是可选字段：**值为空/非正数时不得写入**（CLI schema 是 `int().min(1)`，写 0 或负数会让 `kimi doctor` 报错）

**前端数据层**（`web/types/kimi.ts` + `settingsConfig.ts`）：

- `KimiCatalogModel` 增加 `maxInputSize?: number` / `maxOutputSize?: number` / `reasoningKey?: string`
- `parseKimiSettingsConfig` / `normalizeKimiCatalogModels` / `buildKimiSettingsConfig` 三处同步透传

### 4.4 需求 4：`[secondary_model]` 蜂群模型池

**UI 位置**：Kimi Common Config 弹窗（`KimiCommonConfigModal.tsx`）内分 Tab——「通用配置」（现有自由 TOML 编辑器）与「蜂群模型池」（新增结构化表单）。两者都写 `config.toml`，归属一致。

**表单字段**：

| 字段 | 控件 | 校验 |
|------|------|------|
| 启用蜂群模型池 | Switch | — |
| `default_model` | Select（选项 = 当前 provider 的模型目录 key） | 启用时必填 |
| `force` | Switch | 开启时禁用 `models` 池编辑并清空 |
| `models` 池 | 多选 Select（选项 = 当前 provider 的模型目录 key + 手动输入） | `force` 开启时禁用；不得含保留字 `primary` |

**写回语义**（`config.toml`）：

- 启用：写入 `[secondary_model]` 表；`force=true` 时不写 `[secondary_model.models]`
- 禁用：**整段移除** `[secondary_model]`（含 `models` 子表）
- 沿用 `CONFIG_WRITE_LOCK` + 字段级保留：`[secondary_model]` 从「未知字段保留」转为「受管字段」，需在 `remove_matching_unmanaged_config` 的 protected 列表外显式处理，避免与用户手写值冲突

**与 Gateway 接管的关系**：`[secondary_model]` 引用的 provider 不经过网关（已有已知限制）。接管期间编辑该段**不触发门禁**（它不改 `providers`/`models`/`default_model`），但需在 UI 上给出提示。

**模型 key 来源**：池中的 key 必须是 `[models.<key>]` 中已存在的 key，否则 CLI 校验失败。因此 Select 选项必须来自当前 applied provider 的模型目录。

---

## 5. 改动清单

### 后端（`tauri/src/coding/kimi/`）

| 文件 | 改动 |
|------|------|
| `commands.rs` | `insert_known_model_fields` 加 3 字段；`__local__` 读取路径加 3 字段映射；`remove_matching_unmanaged_config` / `merge_common_config` 处理 `secondary_model` 受管化 |
| `types.rs` | 新增 `KimiPresetModel`、`KimiSecondaryModelConfig` |
| `constants.rs` | 新增 `KIMI_SECONDARY_MODEL_SECTION`、`KIMI_SECONDARY_MODEL_PRIMARY_RESERVED` |
| `mod.rs` / `lib.rs` | 注册新命令 `get_kimi_preset_models` |
| 新增单测 | 3 字段 round-trip；`secondary_model` 写入/移除；preset 匹配（精确 + 域名 + 无匹配） |

### 前端（`web/features/coding/kimi/`）

| 文件 | 改动 |
|------|------|
| `components/KimiProviderFormModal.tsx` | **精简**：移除模型目录内联表格与相关状态，只留基础字段 |
| `components/KimiProviderCard.tsx` | **新增模型区**：标题行按钮（获取模型/添加模型/批量删除）+ 展开的模型行列表 |
| `components/KimiModelFormModal.tsx`（新增） | 单模型编辑弹窗（key/model/displayName/三个尺寸/capabilities/reasoning_key/efforts） |
| `components/KimiCommonConfigModal.tsx` | **改为 Tab 结构**：通用配置 + 蜂群模型池 |
| `utils/kimiCatalogModels.ts`（新增） | catalog upsert / 删除 / 行 key 等纯函数 |
| `utils/settingsConfig.ts` | 3 字段透传 |
| `utils/secondaryModelForm.ts`（新增） | `secondary_model` TOML ↔ 表单状态互转 |
| `services/kimiApi.ts` | 加 `getKimiPresetModels` |
| `types/kimi.ts` | 3 字段 + 预设模型 + 蜂群配置类型 |
| `pages/KimiPage.tsx` | 接线：`FetchModelsModal`、`KimiModelFormModal` 的页级状态与 handler |

### 其他

- `web/i18n/locales/{zh-CN,en-US}.json`：新增文案（必须走 `pnpm i18n:set-key`）
- `tauri/src/coding/kimi/AGENTS.md`：更新 Source of Truth（新增受管段 `[secondary_model]`、新字段）
- `docs/plan-kimi-code-cli.md`：追加 2.x 差异章节
- `tauri/tests/coding/kimi/`：新增集成测试

---

## 6. 实施顺序（增量交付）

| 阶段 | 内容 | 可独立验收 |
|------|------|-----------|
| **0** | 模型管理结构改造（卡片模型区 + 单模型弹窗 + 纯函数工具），**功能等价迁移，不加新字段** | ✅ 迁移后现有 provider 的 catalog 读写行为不变 |
| **1** | 模型条目 3 字段（后端 + 单模型弹窗 + 测试） | ✅ |
| **2** | 拉取模型（卡片按钮 + 共享 `FetchModelsModal`） | ✅ |
| **3** | 预设模型映射表（`get_kimi_preset_models` + UI） | ✅ |
| **4** | `[secondary_model]` 蜂群模型池（Common Config 分 Tab） | ✅ 独立于 1-3 |

阶段 0 单列为一步：它是纯重构，不引入新功能，便于验证「迁移本身没坏」后再叠加新特性。每阶段完成后跑 `pnpm test` + `cargo test` + `pnpm exec tsc --noEmit`。

---

## 7. 风险

| 风险 | 等级 | 缓解 |
|------|------|------|
| `models.dev.json` 是 OpenCode 的缓存文件，远端刷新可能改变结构 | 🟡 | 只读关键字段并做防御性解析；结构不符时返回空数组 |
| `[secondary_model]` 受管化后与用户手写值冲突 | 🟡 | 沿用现有 `remove_matching_unmanaged_config` 语义；禁用时整段移除 |
| 拉取的模型缺 `max_context_size`，CLI 拒绝启动 | 🟢 | 兜底 262144（已有 `DEFAULT_MODEL_MAX_CONTEXT_SIZE` 先例） |
| **模型管理迁移破坏现有数据** | 🔴 | `settingsConfig.ts` 的解析/构建契约**不改**，只改 UI 承载方式；迁移后必须验证已有 provider 的 catalog 完整读回 |
| 蜂群池 key 与 `[models]` 不同步 | 🟡 | Select 选项限定为当前 provider 目录的 key；保存时校验 |

---

## 8. 已确认决策

1. **模型目录布局**：改为 **Codex 模式**——从 Provider 编辑弹窗中移除内联表格，改到 provider 卡片的模型区（列表 + 单模型编辑弹窗）
2. **`[secondary_model]` 位置**：Common Config 弹窗内**分 Tab**
3. **预设模型匹配**：只做 base_url 精确匹配 + 域名模糊匹配，**不做名称模糊匹配**
