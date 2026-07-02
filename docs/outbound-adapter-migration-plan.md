# Provider Outbound Adapter & Orchestrator Filter Migration Plan

## 目标

把参考项目 AxonHub（`D:\GitHub\axonhub`，Go）和 cc-switch（`D:\GitHub\cc-switch`，Rust/Tauri）在协议转换 / 上游 provider 兼容路径上的**专用 outbound adapter**与**orchestrator filter**兼容逻辑，逐个迁移到 AI Toolbox 的 `proxy_gateway` runtime。

执行原则：

- 参考实现行为作为对照基准，但落地必须服从 AI Toolbox 现有模块边界。
- AI Toolbox 的 `transformer/` 模块**按设计 provider 无关**，不能依赖 DB、Tauri app handle、provider 表、runtime context。per-provider 兼容逻辑必须落在 runtime 层，不能塞回协议转换层。
- 新增 per-provider 识别层 + 新建 `runtime/outbound_compat.rs` 统一承载所有出站兼容改写。
- CLI/OAuth 专有逻辑（ClaudeCode system/user_id/tool 前缀、Codex store/include/headers、Copilot x-initiator/optimizer）按项目 AGENTS.md 的“官方订阅不参与代理 + CLI manifest 接管”设计，全部 out-of-scope，不迁。
- 逐个 provider 增量迁移，每一步配最贴近失败模式的回归测试，迁移完跑 `cd tauri && cargo test transformer --no-default-features` 和 `cargo test proxy_gateway::runtime::upstream`。
- 每完成一个 provider 自审 diff 再进下一个。

## 已确认的决策

1. **per-provider 识别**：新增 per-provider 识别层，复用 axonhub/cc-switch 的 base_url / model 名 / `provider_type` 关键字识别（含 `deepseek/moonshot/kimi/grok/longcat/bailian/modelscope/doubao/zai/xiaomi/mimo` 等）。
2. **C 类 CLI/OAuth 逻辑**：全部 out-of-scope，不迁。
3. **落地位置**：新建 `tauri/src/coding/proxy_gateway/runtime/outbound_compat.rs`，按 provider profile + target protocol 组织所有出站兼容；与现有 `apply_outbound_adapter_compat` 合并统一管理。
4. **节奏**：逐个增量迁移，每个配回归测试。

## 执行状态（2026-07-02）

- 本轮实际落地时先沿用 `runtime/upstream.rs` 里的既有 outbound adapter 入口，新增 `apply_outbound_adapter_compat_for_provider`，把 `provider.meta.providerType` 传入请求 body 兼容层；暂不新建 `runtime/outbound_compat.rs`，避免在同一批改动里同时做行为迁移和文件级重构。
- 当前运行时代码的 provider 识别只使用 `provider.meta.providerType + target_protocol`，不再按 base URL 或 model 关键字猜测 provider。下文保留的 base_url/model 关键字条件只作为参考项目行为来源和旧数据分析材料，后续不应把它们直接实现成运行时兜底识别。
- 已增量实现 B1-B9 的 OpenAI Chat request body 兼容、B10/B11 的 DeepSeek/Kimi/Mimo Anthropic thinking 历史归一化、以及通用私有字段过滤/无 tools 控制字段清理。B15-B17 的 provider SSE/stream filter 尚未实现，仍按独立阶段处理。

## 当前架构边界（AI Toolbox）

代码位置：

- `tauri/src/coding/proxy_gateway/transformer/`：4 协议互转，provider 无关，无 DB/runtime context。
- `tauri/src/coding/proxy_gateway/runtime/upstream.rs`：请求转发主链。
  - `build_upstream_body`（line 1282）是出站 body 编排核心：模型改写 → `[1M]` 剥离 → （协议转换）→ `apply_outbound_adapter_compat`（line 1352）→ `cache_injection`。
  - `apply_outbound_adapter_compat`（line 1352）：**只按 `target_protocol` 判断**，目前唯一规则是“无 tools 时清 `tool_choice`/`parallel_tool_calls`（OpenAI Chat/Responses）或 `tool_choice`（Anthropic）”，且 GeminiNative source 跳过（保留 Gemini-derived tool_choice）。
  - 反应式 rectifier：`thinking_signature_rectifier`、`thinking_budget_rectifier`、`cache_injection`——均 Anthropic target 专有、4xx 后触发。
- `tauri/src/coding/proxy_gateway/runtime/providers.rs`：`UpstreamProvider` 结构（已有 `cli_key/id/name/base_url/api_key/target_protocol/auth_strategy/meta/model_mapping`）。
- `tauri/src/coding/proxy_gateway/types.rs`：`ProviderGatewayMeta`（已有 `provider_type/api_format/api_key_field/is_full_url/prompt_cache_key`）。

per-provider 识别层可用输入：`UpstreamProvider.base_url` + `UpstreamProvider.name` + `ProviderGatewayMeta.provider_type` + 请求模型名（`upstream_model_id`）。与 cc-switch 的识别口径一致。

## 出站编排插入点

`build_upstream_body` 当前在 `apply_outbound_adapter_compat` 之后、`cache_injection` 之前。新 `outbound_compat` 模块应替换/扩展现有 `apply_outbound_adapter_compat`，签名需要补上 `provider: &UpstreamProvider` 和 `requested_model`/`upstream_model_id`，使其能做 per-provider 判断。

调用顺序约定（迁移后）：

1. 模型改写 + `[1M]` 剥离（runtime 既有）。
2. 协议结构转换 `convert_request_body`（transformer，既有）。
3. **`outbound_compat::apply_outbound_compat`**（新统一入口）：先 per-target-protocol 通用规则，再 per-provider profile 规则。
4. `cache_injection`（Anthropic target，既有，保持在其后，避免 per-provider 改写破坏 cache 断点）。

注意：`outbound_compat` 只做“规避具体上游严格校验”的窄范围 body 改写，不做协议结构转换（结构转换在 step 2 已完成），不做 URL/header/auth（由 runtime 既有路径/header/auth 决策负责）。

---

## 一、AxonHub 专用 outbound adapter 清单

AxonHub 机制：每个 provider 一个 `outbound.go`，嵌入 `openai.OutboundTransformer` 或 `anthropic.Outbound`，按需 override `TransformRequest/TransformStream`。

### 1A. Per-provider 出站请求改写

| Provider | 文件 | 兼容逻辑 | 作用（解决什么上游严格校验） |
|---|---|---|---|
| DeepSeek | `deepseek/outbound.go` | `json_schema`→`json_object`（剥 JSONSchema）；注入 `thinking:{type:enabled\|disabled}`（按 `reasoning_effort==none`）；非 disabled 时给 assistant 空 `reasoning_content=""`；支持 `/beta#` completion 端点 | DeepSeek 拒绝 json_schema；要求 thinking 字段；空 reasoning assistant 被拒 |
| Moonshot | `moonshot/outbound.go` | `json_schema`→`json_object` | Moonshot 不支持 json_schema |
| Zai (GLM) | `zai/outbound.go` | `json_schema`→`json_object`；`tool_choice` 强制改 `auto`；从 metadata 提取 `user_id/request_id`；`reasoning_effort`→`thinking:{type}`；剥离 metadata | GLM 端点只支持 auto tool_choice；不支持 metadata；私有 thinking 字段 |
| Doubao | `doubao/outbound.go` | 从 metadata 提取 `user_id/request_id`（缺则生成 `req_<ts>`）；剥离 metadata | 字节要求 request_id；不支持 metadata |
| xAI (Grok) | `xai/outbound.go` | 按模型名剥离：`grok-4` 去 `reasoning_effort/presence_penalty/frequency_penalty/stop`；`grok-3/3-mini` 去 penalty/stop | Grok 这些模型不支持上述参数 |
| Longcat | `longcat/outbound.go` | 强制所有 message content 为 array 格式；空 content 补 `""` | LongCat-Flash-Omni 拒绝 string content（报 json format error） |
| ModelScope | `modelscope/outbound.go` | 剥离 `metadata` | ModelScope 不支持 metadata |
| Fireworks | `fireworks/outbound.go` | 仅 `ReasoningFieldContent`，无 override | OpenAI 兼容，reasoning_content 字段（无需改写） |
| Bailian (百炼) | `bailian/outbound.go` + `stream_filter.go` | 合并连续 assistant tool-call message；流式 filter：tool-call 后缓冲文本、丢弃空 `{}` tool args | 百炼拒绝分散的 tool-call message；流式文本/工具交叉乱序 |
| NanoGPT | `nanogpt/outbound.go` | `ReasoningFieldReasoning`（用 `reasoning` 而非 `reasoning_content`）；流式过滤上游 `[DONE]` 并自加 done | NanoGPT reasoning 字段名不同 |
| OpenRouter | `openrouter/outbound.go` | `ReasoningFieldReasoning`；流式过滤 `[DONE]` 自加 done；自定义 error 解析（`error.metadata.raw`） | OpenRouter reasoning 字段名；流式 done 重复；错误结构特殊 |
| Ollama | `ollama/outbound.go` | 完全独立协议 `/api/chat`：`options.{temperature,top_p,top_k,num_predict,stop}`、`messages[].images`（剥 data URL 前缀）、`thinking` 字段 | Ollama 原生协议非 OpenAI |
| OpenAI 基座 | `openai/outbound_convert.go` + `google.go` | `len(tools)==0` 时清 `parallel_tool_calls`；过滤非 function tool；`stripUnsupportedToolCallExtraContent`（剥 Google thought_signature） | OpenAI Chat 只接受 function tool；无 tool 时不应留 parallel 标记；Google 私有 signature 不能发给 OpenAI |
| Claude Code | `anthropic/claudecode/*` | 注入 system message+cache_control；fake user_id；official 时加 `proxy_` tool 前缀（响应剥离）；billing CCH；强制禁 thinking 当 tool_choice=any/named；beta/version headers | **C 类，out-of-scope，不迁** |
| Codex | `openai/codex/outbound.go` | 剥 `max_tokens/max_completion_tokens`；`store=false`；`parallel_tool_calls=true`；`include:[reasoning.encrypted_content]`；`reasoning_summary=auto`；`array_inputs=true`；Originator/Session/Chatgpt-Account-Id headers | **C 类，out-of-scope，不迁** |

### 1B. AxonHub orchestrator / pipeline 过滤

| 过滤 | 位置 | 作用 | 迁移归类 |
|---|---|---|---|
| `stripUnsupportedToolCallExtraContent` | `openai/outbound.go:181` | 出站剥 Google thought_signature | A 类（target-protocol 级，OpenAI Chat target） |
| `RequestFromLLM` 内 `len(tools)==0 → ParallelToolCalls=nil` | `openai/outbound_convert.go:90` | 无 tool 时清 parallel | A 类，**AI Toolbox 已有等价 `apply_outbound_adapter_compat`** |
| `RequestFromLLM` 过滤非 function tool | `openai/outbound_convert.go:63` | OpenAI Chat 只接受 function tool | A 类（需确认 transformer 出站是否已过滤） |
| `xai.IsValidResponse` stream filter | `xai/outbound.go:128` | 丢弃空 delta 事件 | B 类（per-provider，Grok） |
| `bailian` stream filter | `bailian/stream_filter.go` | tool-call 文本缓冲 + 空 args 丢弃 | B 类（per-provider，百炼），**流式** |
| `nanogpt/openrouter` stream filter | 各 outbound.go | 过滤上游 `[DONE]`、自加 done | B 类（per-provider），**流式** |
| `pipeline/cc` | `llm/pipeline/cc` | Claude Code 专用编排 | C 类，不迁 |
| `pipeline/maxtoken` | `llm/pipeline/maxtoken` | max_tokens 兼容改写 | 评估：部分可并入 per-provider（Grok/Longcat 等） |
| `pipeline/stream` | `llm/pipeline/stream` | 流式适配 | 评估：并入 per-provider 流式 filter |

---

## 二、cc-switch 兼容逻辑清单

cc-switch `src-tauri/src/proxy/` 有完整 filter 链。`forwarder.rs` 请求管道顺序与作用：

| 步骤 | 模块 | 作用 | 触发条件 | 迁移归类 |
|---|---|---|---|---|
| 1 | `body_filter::filter_private_params_with_whitelist` | 递归剥离 `_` 前缀私有字段（保留 schema `properties/$defs` 内的 `_` 名） | 所有请求 | B 类（通用，所有 provider） |
| 2 | `model_mapper` + `strip_one_m_suffix` | 模型族映射 + `[1M]` 剥离 | 所有请求 | **AI Toolbox runtime 已有** |
| 3 | `normalize_anthropic_messages_for_provider` | DeepSeek/Kimi/Mimo Anthropic 端点：tool_use 历史补 `thinking` placeholder、redacted_thinking→thinking、剥 signature；DeepSeek 官方 `thinking:disabled` 时剥 `output_config.effort`/`reasoning_effort` | 按 base_url/model 名识别 vendor | B 类（per-provider，DeepSeek/Kimi/Mimo，Anthropic target 直通路径） |
| 4 | `media_sanitizer` | text-only 模型把 image block 替换为 marker；4xx unsupported_image 时重试剥图 | 按 modelCatalog/modalities 或启发式 | 评估：可选后续项 |
| 5 | `transform_claude_request_for_api_format` | Anthropic↔OpenAI Chat/Responses/Gemini 转换；Kimi/DeepSeek/Mimo 保留 reasoning_content；流式注入 `stream_options.include_usage`；注入 `prompt_cache_key` | api_format 配置 | **转换本体已迁**；reasoning_content 保留 + include_usage 注入为 B 类 |
| 6 | `thinking_optimizer` | 按模型注入/调整 thinking：haiku 跳过、opus/sonnet-4.x adaptive、其他 enabled+budget | 配置开关 | 评估：与 thinking_budget_rectifier 重叠，可选 |
| 7 | `cache_injector` | 注入 cache_control 断点 + 升级 TTL | 配置开关 | **AI Toolbox runtime 已有 `cache_injector.rs`** |
| 8 | `copilot_optimizer` | Copilot 专有：`x-initiator` user/agent 分类、orphan tool_result 清理、tool_result 合并、strip thinking、warmup 降级、deterministic request/interaction id | Copilot provider | **C 类，out-of-scope，不迁** |
| 9 | `apply_codex_chat_upstream_model` | Codex chat 模型解析 | Codex | **C 类，不迁** |
| 反应式 | `thinking_rectifier` | 4xx signature 错误时剥 thinking/redacted/signature 重试 | Anthropic 4xx | **AI Toolbox runtime 已有** |
| 反应式 | `thinking_budget_rectifier` | 4xx budget 错误时调整 budget_tokens/max_tokens 重试 | Anthropic 4xx | **AI Toolbox runtime 已有** |

cc-switch 的 per-provider 检测口径（`proxy/providers/claude.rs`）：

- `provider_type`：`github_copilot`/`codex_oauth`/`openrouter`。
- base_url 关键字：`githubcopilot.com`、`openrouter.ai`、`api.deepseek.com/anthropic`、`api.moonshot.cn`、`api.kimi.com`、`api.xiaomimimo.com`。
- model 名关键字（`REASONING_VENDOR_HINTS`）：`moonshot/kimi/deepseek/mimo/xiaomimimo`。

AI Toolbox 识别层应复用这套口径，但去掉 C 类（copilot/codex_oauth/openrouter 的 OAuth 专有部分），保留纯 body 兼容部分。

---

## 三、迁移归类总表

### A 类：per-target-protocol 级（不依赖具体 provider，落到 outbound_compat 通用层）

| 编号 | 兼容逻辑 | 来源 | 当前状态 | 目标落点 |
|---|---|---|---|---|
| A1 | 无 tools 时清 `tool_choice`/`parallel_tool_calls`（OpenAI Chat/Responses）或 `tool_choice`（Anthropic）；GeminiNative source 跳过 | axonhub `openai/outbound_convert.go:90` + cc-switch | **已实现** `apply_outbound_adapter_compat`（upstream.rs:1352） | 迁入 `outbound_compat` 通用层（行为不变，仅搬迁） |
| A2 | OpenAI Chat target 出站不泄漏 Google thought_signature（历史 message tool_calls 与 tools 定义都不能带） | axonhub `openai/google.go:stripUnsupportedToolCallExtraContent` | **已隐式实现**：transformer 出站 message tool_calls 只输出 `id/type/function.{name,arguments}`（`openai/chat.rs:514-521`），thought_signature 存 `transformer_metadata["gemini_thought_signature"]` 不进入 Chat body；tools 定义里 Google native tool 因无 `function` 字段被丢弃（`openai/chat.rs:417`） | **仅补回归测试**，不实现。测试断言：Gemini source 转 OpenAI Chat target 后，body 内任何 tool_calls/tools 都不含 `thought_signature`/`google` 字段 |
| A3 | OpenAI Chat target 过滤非 function tool（image_generation / google native 等） | axonhub `openai/outbound_convert.go:63` | **已隐式实现但有语义差异**：AI Toolbox 出站过滤掉 Google native tool（无 function 字段），但**有意保留** `responses_custom_tool` 兼容扩展（`openai/chat.rs:400-416`、`497-512`，AGENTS.md 05 节明确这是 roundtrip 语义）。axonhub 则把 custom tool 也过滤掉 | **不按 axonhub 迁移**（AI Toolbox 行为是有意设计）。严格 OpenAI Chat provider 不接受 `responses_custom_tool` 扩展时，归入 per-provider：DeepSeek 等严格 profile 在 `outbound_compat` 里额外剥离 `responses_custom_tool` 兼容扩展（见 B1 扩展项） |

### B 类：per-provider 级（需要 per-provider 识别层，落到 outbound_compat 的 profile 分支）

每条给出：识别条件、改写规则（字段路径 + 判断）、测试 shape 来源。

| 编号 | Provider profile | 识别条件 | 改写规则 | 测试 shape 来源 |
|---|---|---|---|---|
| B1 | DeepSeek（OpenAI Chat target） | base_url 含 `deepseek.com` 或 model 含 `deepseek` | `response_format.type=="json_schema"` → 改 `json_object` 并删 `response_format.json_schema`；**扩展项**：同时剥离 `responses_custom_tool` 兼容扩展（严格 provider 不接受，对应 A3 语义差异） | axonhub `deepseek/outbound_test.go` + cc-switch |
| B2 | DeepSeek（OpenAI Chat target） | 同 B1 | 顶层注入 `thinking:{type: "enabled"\|"disabled"}`，按 `reasoning_effort=="none"` 决定；`thinking!=disabled` 时遍历 `messages[]`，role==assistant 且 `reasoning_content` 缺失则补 `""` | axonhub `deepseek/outbound.go:114-129` |
| B3 | Moonshot（OpenAI Chat target） | base_url 含 `moonshot.cn` 或 model 含 `moonshot`/`kimi` | 同 B1 的 json_schema→json_object | axonhub `moonshot/outbound.go:87` |
| B4 | Zai/GLM（OpenAI Chat target） | base_url 含 `zai`/`glm`/`chatglm`/`open.bigmodel` 或 model 含 `glm` | json_schema→json_object；`tool_choice` 非空时强制改 `auto`（剥 named/required）；`reasoning_effort` 非空 → 顶层 `thinking:{type: enabled\|disabled}`（none→disabled） | axonhub `zai/outbound.go:120-164` |
| B5 | Doubao（OpenAI Chat target） | base_url 含 `doubao`/`volces`/`ark.cn-beijing` 或 model 含 `doubao` | 从 `metadata.user_id`/`metadata.request_id` 提取到顶层 `user_id`/`request_id`；`request_id` 缺则生成 `req_<timestamp>`（注意：脚本里 `Date.now` 受限，需用请求自带的稳定 id 或 counter）；删 `metadata` | axonhub `doubao/outbound.go:145-157` |
| B6 | xAI/Grok（OpenAI Chat target） | base_url 含 `x.ai` 或 model 含 `grok` | 按模型分支剥参数：`grok-4` 删 `reasoning_effort`+`presence_penalty`+`frequency_penalty`+`stop`；`grok-3`/`grok-3-mini` 删 `presence_penalty`+`frequency_penalty`+`stop`；其他不动 | axonhub `xai/outbound.go:111-123` |
| B7 | Longcat（OpenAI Chat target） | base_url 含 `longcat` 或 model 含 `longcat` | 遍历 `messages[]`：content 为 null/缺失且无 multiple_content 时补 `""`；content 序列化强制 array 格式（string → `[{type:"text",text:"..."}]`） | axonhub `longcat/outbound.go:64-89` |
| B8 | ModelScope（OpenAI Chat target） | base_url 含 `modelscope` 或 model 含 `modelscope` | 删顶层 `metadata` | axonhub `modelscope/outbound.go:62` |
| B9 | Bailian/百炼（OpenAI Chat target） | base_url 含 `bailian`/`aliyuncs`/`dashscope` 或 model 含 `qwen`（且非 OpenRouter） | 合并连续 assistant tool-call message：相邻多个 `role:assistant` 且仅含 `tool_calls`（无 content/name/refusal/reasoning/cache_control/message_index）的 message 合并成一条，`tool_calls` 拼接 | axonhub `bailian/outbound.go:68-113` |
| B10 | DeepSeek/Kimi/Mimo（**Anthropic target 直通**） | base_url 含 `deepseek.com/anthropic`/`kimi.com`/`xiaomimimo` 或 model 含 `deepseek`/`kimi`/`mimo`；**且 target==AnthropicMessages 且 source==AnthropicMessages（直通）** | 遍历 `messages[]` role==assistant 且 content 含 `tool_use` block：thinking block 若 `thinking` 为空则补 placeholder `"tool call"` 并删 `signature`；`redacted_thinking` block → `thinking` block 文本 `[redacted thinking]`；若无任何 thinking block 则在 content 头部插入 `{type:thinking,thinking:"tool call"}` | cc-switch `claude.rs:234-302` 测试 |
| B11 | DeepSeek 官方 Anthropic 端点（**Anthropic target 直通**） | base_url == `https://api.deepseek.com/anthropic`（精确匹配，trim 尾斜杠）；**且 target==AnthropicMessages 且直通** | `thinking.type=="disabled"` 时：删 `output_config.effort`（`output_config` 空则删整个 `output_config`）+ 删 `reasoning_effort`；非 disabled 不动 | cc-switch `claude.rs:179-217` 测试 |
| B12 | Kimi/DeepSeek/Mimo（OpenAI Chat target，经转换路径） | 同 B10 vendor 识别；**且 target==OpenAIChat 且 source!=OpenAIChat（转换路径）** | transformer 出站默认对 assistant message 输出 `reasoning_content`（`openai/chat.rs:489-491`），本项是**确认+测试**而非新增实现：断言这些 vendor 转 OpenAI Chat 后 assistant message 保留 `reasoning_content`，不被剥离。若未来出站加了"非 vendor 剥 reasoning"开关，则在此 profile 显式开启保留 | cc-switch `claude.rs:304-328` 测试 |
| B13 | 通用（所有 provider，所有 target） | 无（总是应用） | 递归剥离 `_` 前缀顶层/嵌套私有字段；但 schema `properties`/`patternProperties`/`definitions`/`$defs` map 内的 `_` 名是用户字段名，**不剥**；白名单机制保留（如 `_stream_options`） | cc-switch `body_filter.rs` 测试 |
| B14 | OpenAI Chat target 流式 | target==OpenAiChat 且流式 | **已实现**：transformer 出站 `openai/chat.rs:384-391` 在 `stream==true` 时自动注入 `stream_options.include_usage`（默认 true，尊重入站已有值）。**仅补回归测试**：断言 Anthropic/Gemini source 转 OpenAI Chat 流式后 body 含 `stream_options.include_usage==true`，非流式不含 `stream_options` | 已有 chat.rs 出站逻辑 |

### B 类流式 filter（per-provider，需在 SSE 透传链插入，复杂度较高，单独阶段）

| 编号 | Provider profile | 兼容逻辑 | 来源 |
|---|---|---|---|
| B15 | Bailian/百炼 | 流式：tool-call 后缓冲文本、丢弃空 `{}` tool args | axonhub `bailian/stream_filter.go` |
| B16 | xAI/Grok | 流式：丢弃空 delta 事件（无 content/tool/role/finish/refusal/reasoning） | axonhub `xai.IsValidResponse` |
| B17 | NanoGPT/OpenRouter | 流式：过滤上游 `[DONE]`、自加 done；error 事件解析 | axonhub nanogpt/openrouter |

### C 类：out-of-scope，不迁

- ClaudeCode：system message 注入、fake user_id、`proxy_` tool 前缀、billing CCH、forced tool 时禁 thinking、beta/version headers。
- Codex：剥 max_tokens、`store=false`、`parallel_tool_calls=true`、`include:[reasoning.encrypted_content]`、`reasoning_summary=auto`、`array_inputs=true`、Originator/Session/Chatgpt-Account-Id headers。
- Copilot：OAuth token、`x-initiator` user/agent 分类、orphan tool_result 清理、tool_result 合并、strip thinking、warmup 降级、deterministic id（copilot_optimizer 整体）。
- Ollama：完全独立协议（AI Toolbox 当前不支持 Ollama native target，属于新增协议，不在本次迁移范围）。

理由：AI Toolbox 的 `category=official` provider 代表 CLI 原生 OAuth 官方订阅，明确跳过代理；CLI 接管另有 manifest 机制。C 类逻辑与现有 CLI 接管设计冲突，迁移会破坏托管字段/备份恢复语义。

---

## 四、实施方案

### 阶段 0：搭骨架（outbound_compat 模块 + per-provider 识别层）

**新建** `tauri/src/coding/proxy_gateway/runtime/outbound_compat.rs`，模块结构：

```rust
// 1. profile 枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderCompatProfile {
    Generic,
    DeepSeek,
    Moonshot,   // Kimi 归入 Moonshot（同 ReasoningFieldContent + json_object）
    Zai,        // GLM
    Doubao,
    Grok,
    Longcat,
    ModelScope,
    Bailian,
}

// 2. 识别层：复用 cc-switch 口径
pub(crate) fn detect_profile(
    provider: &UpstreamProvider,
    upstream_model_id: &str,
) -> ProviderCompatProfile { /* base_url/model 名 contains 匹配 */ }

// 3. 统一入口
pub(crate) fn apply_outbound_compat(
    body: Vec<u8>,
    provider: &UpstreamProvider,
    target_protocol: AiProtocol,
    conversion_route: Option<ConversionRoute>,
    upstream_model_id: &str,
    is_streaming: bool,
) -> Result<Vec<u8>, GatewayForwardError> {
    // 3a. A 类通用规则（迁自 apply_outbound_adapter_compat）：
    //     - GeminiNative source 跳过
    //     - OpenAI Chat/Responses target：无 tools 清 tool_choice+parallel_tool_calls
    //     - Anthropic target：无 tools 清 tool_choice
    // 3b. B13 私有字段过滤（通用，所有 profile）
    // 3c. B 类 per-profile dispatch（仅当 target 匹配该 profile 适用 target 时）
    //     - OpenAI Chat target：B1/B2/B3/B4/B5/B6/B7/B8/B9/B12
    //     - Anthropic target 直通：B10/B11
}
```

**改造点**：

1. `runtime.rs` 加 `mod outbound_compat;`。
2. `upstream.rs`：
   - `build_upstream_body` 签名加 `provider: &UpstreamProvider`（当前签名无 provider）。
   - 两个调用点都能拿到 provider：`send_upstream_request`（upstream.rs:575 已有 `provider` 参数，line 598 调用）、`build_thinking_signature_rectified_upstream_body`（line 1408，rectifier 路径，调用上下文 `send_upstream_request` 也有 provider，需向上透传一层）。
   - 把 `apply_outbound_adapter_compat(upstream_body, conversion_route, target_protocol)`（line 1345）替换为 `outbound_compat::apply_outbound_compat(upstream_body, provider, target_protocol, conversion_route, upstream_model_id, route_streaming)`。
   - 删除旧 `apply_outbound_adapter_compat`（line 1352）和 `remove_tool_controls_without_tools`（line 1392），迁入 `outbound_compat`。
3. **直通路径启用 B10/B11**：当前 `apply_outbound_adapter_compat` 在 `conversion_route.is_none()` 时直接 return body（直通不改写）。B10/B11 必须对直通路径生效，所以新 `apply_outbound_compat` 要在直通路径下也运行 B10/B11 分支（仅 DeepSeek/Kimi/Mimo profile + Anthropic target），其余直通仍不改写。这是对"直通不重写"原则的**显式窄范围例外**，必须在代码注释和 AGENTS.md 标注。

**回归**：现有 `outbound_adapter_*` 4 个测试（upstream.rs:2401-2527）必须保持通过，只是函数位置迁移。补 1 个 `detect_profile` 单元测试覆盖各 profile 关键字命中与不命中。

### 阶段 1：A 类去重确认（A2/A3/B14 只补测试）

核查结论已写入总表：A2、A3（Google tool 部分）、B14 在 transformer 出站已隐式实现。本阶段**不写实现代码**，只补回归测试锁定行为：

- A2 测试：Gemini source fixture 转 OpenAI Chat target，断言 body 内 `messages[].tool_calls[]` 和 `tools[]` 都不含 `thought_signature`、`google`、`thoughtSignature` 字段。
- A3 测试：Gemini source 含 `googleSearch` native tool 转 OpenAI Chat target，断言 `tools[]` 不含 google native tool（已被过滤）；同时断言 `responses_custom_tool` 兼容扩展**仍保留**（锁定 AI Toolbox 有意行为，防止误改）。
- B14 测试：Anthropic source 流式请求转 OpenAI Chat target，断言 `stream_options.include_usage==true`；非流式断言无 `stream_options`。
- A3 语义差异归 per-provider：在 B1 DeepSeek profile 里加"剥离 `responses_custom_tool` 兼容扩展"测试（严格 provider 不接受）。

### 阶段 2：B 类 per-provider 请求体改写（逐个）

按总表 B1–B14 顺序，但跳过 B12（仅测试）、B14（仅测试）。每个流程：

1. 在 `outbound_compat.rs` 加该 profile 的改写函数 + `detect_profile` 关键字。
2. 在 `apply_outbound_compat` dispatch 里挂上（注意 target 匹配）。
3. 配单元测试，断言依据来自总表"测试 shape 来源"列对应 axonhub/cc-switch 测试用例的脱敏 shape。
4. 跑 `cd tauri && cargo test proxy_gateway::runtime::upstream` + `cargo test transformer --no-default-features`。
5. 自审 diff，进下一个。

建议顺序（按风险从低到高）：

1. B13 私有字段过滤（通用，最独立，无 target 限制）。
2. B14/A2/A3/B12 补测试（阶段 1，无实现）。
3. B1/B3 json_schema→json_object（DeepSeek/Moonshot，最简单，含 B1 的 custom tool 剥离）。
4. B8 ModelScope 剥 metadata、B5 Doubao 提取 request_id+剥 metadata。
5. B6 Grok 按模型剥参数。
6. B7 Longcat 强制 array content。
7. B4 Zai tool_choice→auto + thinking 字段。
8. B2 DeepSeek thinking 字段 + 空 reasoning_content。
9. B9 Bailian 合并 tool-call message。
10. B10/B11 DeepSeek/Kimi/Mimo Anthropic 直通 thinking 历史归一化 + thinking:disabled 剥 effort（最后做，因为是直通路径例外，风险最高）。

### 阶段 3：B 类流式 filter（B15–B17，单独阶段，先设计后实现）

流式 filter 需要在 SSE 透传链插入 per-provider stream wrapper，与现有 `transformer` stream 转换链协同。本阶段**先只设计子计划，不写代码**。

设计要点（待阶段 2 完成后细化）：

1. **插入点定位**：确认 SSE 透传链结构。`upstream.rs` 流式分支把上游 chunk 边读边写回客户端；`transformer` 的 `convert_sse_stream` 在 source≠target 时做协议级 stream 转换。per-provider stream filter 要在协议转换**之后**、写回客户端**之前**插入（对已转成 target 协议 SSE 的事件做 per-provider 过滤）。
2. **B15 Bailian stream filter**：tool-call 模式下缓冲 tool-call 后的文本 delta，finish 时输出；丢弃重复空 `{}` tool args。需维护 per-(choice,call) 的 tool args 累积状态。
3. **B16 Grok stream filter**：丢弃无 content/tool_calls/role/finish_reason/refusal/reasoning 的空 delta 事件（`xai.IsValidResponse`）。
4. **B17 NanoGPT/OpenRouter stream filter**：过滤上游重复 `[DONE]`、自加 done 事件；error 事件按 provider 私有结构解析。
5. **约束**：必须边读边改、bounded buffer，遵守 AGENTS.md 的 SSE 边读边写约束；不能 full-buffer 整个上游流。完成事件保持幂等。
6. **风险**：流式 filter 与 transformer stream 状态机叠加，状态正确性测试成本高，单独评估 ROI 后再决定是否实现。

### 阶段 4：文档与 AGENTS.md 更新

- 把迁移完成的 per-provider 兼容规则补进 `proxy_gateway/AGENTS.md` 的 outbound adapter 规则段（当前 line 109 附近只描述了 tool_choice 清理这一条），列出各 profile 的改写。
- 在 `transformer/AGENTS.md` 的“非目标范围”反向说明：per-provider 兼容由 `runtime/outbound_compat.rs` 承担，transformer 自身保持 provider 无关。
- 更新本 plan 文档各条目的状态标记（已实现/已迁移/待办）。

---

## 五、per-provider 识别关键字清单（detect_profile 实现依据）

复用 cc-switch `proxy/providers/claude.rs` 口径，base_url / model 名做 `contains`（小写化后匹配），`provider_type` 做精确匹配。**未命中一律 `Generic`，不改写**。

| Profile | base_url 关键字（小写 contains） | model 名关键字（小写 contains） | provider_type |
|---|---|---|---|
| DeepSeek | `deepseek.com` | `deepseek` | — |
| Moonshot | `moonshot.cn`、`kimi.com` | `moonshot`、`kimi` | — |
| Zai (GLM) | `zai`、`chatglm`、`open.bigmodel`、`bigmodel.cn` | `glm` | — |
| Doubao | `doubao`、`volces.com`、`ark.cn-beijing` | `doubao` | — |
| Grok | `x.ai` | `grok` | — |
| Longcat | `longcat` | `longcat` | — |
| ModelScope | `modelscope` | `modelscope` | — |
| Bailian | `bailian`、`aliyuncs.com`、`dashscope.aliyuncs.com` | `qwen`（需排除 OpenRouter 聚合的 qwen） | — |
| Generic | （未命中以上） | — | — |

注意：

- `qwen` 关键字同时可能出现在 OpenRouter 聚合模型名里（如 `openrouter/qwen-...`），Bailian 识别必须**先排除 OpenRouter**（base_url 含 `openrouter.ai` 时归 Generic，因为 OpenRouter 已支持 Claude Code 兼容接口，cc-switch 也默认对其关转换）。
- B11 DeepSeek 官方 Anthropic 端点用**精确匹配** `https://api.deepseek.com/anthropic`（trim 尾斜杠），不用 contains，避免误命中其他 deepseek 路径。
- model 名匹配前必须先剥离 `[1M]` 标记（runtime 既有 `strip_one_m_context_marker`），用剥离后的 `upstream_model_id` 匹配。
- 关键字误命中风险：`glm`/`grok`/`kimi` 较短，需评估是否要求 base_url 命中优先于 model 名命中（base_url 更可靠）。建议优先级：`provider_type` 精确 > base_url contains > model 名 contains。

---

## 六、风险与注意事项

1. **per-provider 识别的误判风险**：base_url/model 名关键字 contains 匹配可能误命中（如 `glm` 命中非 GLM 模型名）。识别层优先级：`provider_type` 精确 > base_url contains > model 名 contains；未命中走 Generic 不改写。`qwen`/`glm`/`grok`/`kimi` 等短关键字需在测试里覆盖反例（如 `openrouter/qwen-...` 不应命中 Bailian）。
2. **直通路径例外（B10/B11）**：当前 `apply_outbound_adapter_compat` 在 `conversion_route.is_none()`（直通）时直接 return body。B10/B11 必须对 Anthropic target 直通路径生效——这是对“直通不重写”原则的**显式窄范围例外**，必须限定到 DeepSeek/Kimi/Mimo profile + AnthropicMessages target，在代码注释和 AGENTS.md 标注，不能泛化到其他 profile 或其他 target。
3. **cache_injection 顺序**：per-provider 改写在 `cache_injection`（upstream.rs:1346）之前。B7 Longcat 改 content 格式、B9 Bailian 合并 message 都会改变 message 结构，可能影响 cache 命中。需在测试里覆盖“per-provider 改写 + cache_injection 开启”组合，确认 cache 断点位置仍正确。
4. **流式 filter（阶段 3）**不能 full-buffer，必须边读边改，遵守 AGENTS.md 的 SSE 边读边写约束；完成事件保持幂等。
5. **去重确认（A2/A3/B14）**：已在总表标注“已隐式实现，仅补测试”。实现时不得在 `outbound_compat` 重复剥 Google thought_signature 或重复注入 stream_options——这些由 transformer 出站负责。B1 DeepSeek 剥 `responses_custom_tool` 是 A3 语义差异的 per-provider 补丁，不与 A3 冲突。
6. **reasoning_content 保留（B12）**：transformer 出站默认对 assistant message 输出 `reasoning_content`（`openai/chat.rs:489-491`），所以 B12 是“确认+测试”而非新增实现。若未来出站加了“非 vendor 剥 reasoning”开关，B12 profile 需显式开启保留。
7. **回归测试规则**：每个 provider 迁移完必须补最贴近失败模式的测试（参考总表“测试 shape 来源”列对应 axonhub/cc-switch 测试用例的脱敏 shape），没有测试不能宣称完成。测试不得包含真实 API key 或敏感输入。
8. **B5 Doubao request_id 生成**：axonhub 用 `time.Now().Unix()`。runtime 是真实 Rust 代码可用 `chrono::Utc::now()`，但 request_id 应优先用请求自带 id（metadata.request_id 或 trace id），仅缺失时才生成，且生成值要稳定可复现（测试用固定输入）。

## 七、不迁项明确清单（防止遗漏反向追问）

- Ollama native 协议（`/api/chat`、`options.*`、`messages[].images`）：AI Toolbox 无 Ollama target 协议，属新增协议，不在本次范围。
- Doubao/Zai 的 image/video generation：非聊天能力，out-of-scope。
- OpenRouter image generation via modalities：非聊天能力，out-of-scope。
- NanoGPT XML parser、Copilot model map：CLI 专有，out-of-scope。
- cc-switch `media_sanitizer`、`thinking_optimizer`：评估为可选后续项，不列入本次必迁范围（media_sanitizer 需 modelCatalog 数据，thinking_optimizer 与现有 rectifier 重叠）。
- C 类全部（ClaudeCode/Codex/Copilot 的 OAuth/header/optimizer）：out-of-scope，理由见总表。

## 八、迁移工作量预估

| 类别 | 条目数 | 实现工作量 | 测试工作量 |
|---|---|---|---|
| 阶段 0 骨架 | 1 模块 + 2 调用点改造 | 中 | 低（迁移现有 4 测试 + profile 识别测试） |
| 阶段 1 A 类去重 | A2/A3/B14 | **零实现**（已隐式实现） | 中（3 组回归测试） |
| 阶段 2 B 类请求体 | B1–B13（B12/B14 仅测试） | 11 条实现 + 2 条仅测试 | 高（每条 1–3 个测试） |
| 阶段 3 B 类流式 | B15–B17 | 高（待设计） | 高 |
| 阶段 4 文档 | AGENTS.md ×2 | 低 | — |

净实现：阶段 0 骨架 + 阶段 2 的 11 条 per-provider 改写。A2/A3/B14/B12 是去重确认，大幅减少了原本以为要做的工作。
