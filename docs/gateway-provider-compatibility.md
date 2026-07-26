# Gateway Provider 兼容细节

本文记录 Proxy Gateway 逐 provider/channel 的 wire 兼容事实，覆盖当前源码可证实的入参兼容、出参兼容、触发条件、默认行为、开关和回归测试。

本文不是架构主文档。协议转换架构、统一 IR、SSE 生命周期、runtime/transformer/pipeline/side store 边界、参考项目同步流程和 baseline commit 仍以 [`docs/gateway-protocol-conversion.md`](gateway-protocol-conversion.md) 为准。后续同步 `../cc-switch` 或 `../axonhub` 时，先按架构主文档读取 baseline 和参考项目查询入口；如果吸收结果改变 provider/channel 兼容事实，再同步更新本文。

本文只写当前实现能证明的行为。参考项目已有但 AI Toolbox 还没有实现的能力，不能在本文写成“已支持”。

## 1. 事实源与触发模型

### 1.1 方向术语

- 请求侧：客户端 CLI -> Gateway -> 上游 provider。
- 响应侧：上游 provider -> Gateway -> 客户端 CLI。
- source protocol：Gateway 从入站 route 推导出的客户端协议。
- target protocol：Gateway 从 provider effective meta/settings 推导出的上游协议。
- 同协议直通：source protocol 与 target protocol 相同，不创建 `ConversionRoute`，不调用结构转换器；runtime 仍会执行 URL/header/auth、模型名、provider body、stream filter、rectifier 等兼容。
- 跨协议转换：source protocol 与 target protocol 不同，先由 `transformer` 做公共协议结构转换，再由 runtime 对最终上游 body 做 provider 兼容。

### 1.2 生产触发不是 `compat` 字段

`tauri/resources/gateway_provider_profiles.json` 里的 `compat` 字段是 catalog 描述和 schema 校验材料；`tauri/src/coding/proxy_gateway/provider_profiles.rs::SUPPORTED_COMPAT_RULES` 只对白名单名称做校验。生产请求不会因为某个 profile 声明了 `compat` 字符串就直接执行兼容逻辑。

真正触发来自 runtime 解析后的 effective meta：

- `providerType` / `provider_type`
- `apiFormat` / `api_format`
- `apiKeyField` / `api_key_field`
- `reasoningField` / `reasoning_field`
- `defaultMaxTokens` / `default_max_tokens`
- `codexChatReasoning` / `codex_chat_reasoning`
- `imageInputPolicy` / `image_input_policy`
- `textOnlyModels` / `text_only_models`
- `imageCapableModels` / `image_capable_models`
- `allowTextOnlyModelHeuristic` / `allow_text_only_model_heuristic`
- provider record `category` legacy fallback

源码入口：`tauri/src/coding/proxy_gateway/runtime/providers.rs::provider_meta_from_record()`。

### 1.3 `gatewayProfile` 动态解析

内置渠道保存的是 `data.meta.gatewayProfile={tool,profileId,endpointId}` 引用。runtime 每次读取 provider 时从当前 `gateway_provider_profiles.json` 动态解析 profile/endpoint：

- `providerType` 来自 profile。
- `apiFormat` 来自 endpoint，决定 `UpstreamProvider.target_protocol`。
- `apiKeyField`、`reasoningField`、`defaultMaxTokens`、图片策略字段优先 endpoint，再 fallback profile。
- profile 中的 `codexChatReasoning` 只在 `gatewayProfile.tool == "codex"` 时解析；Claude/Grok/Gemini 不从 profile 解析这个 Codex-only 字段，但后续 fallback inference 仍可由明确 effective `providerType/apiFormat` 触发。
- `gatewayProfile.tool` 必须匹配当前 CLI；不匹配时忽略引用，继续使用 legacy meta。
- profile/endpoint 缺失或解析失败时保留 legacy meta；如果最终 `providerType` 仍为空，fallback 到 provider record 的 `category`。

源码入口：`runtime/providers.rs::apply_gateway_profile_reference()`。

### 1.4 target protocol 推导

- Claude：effective `apiFormat` -> settings `api_format/apiFormat` -> `openrouter_compat_mode=true` -> 默认 `AnthropicMessages`。
- Codex：effective `apiFormat` -> settings `api_format/apiFormat` -> `config.toml` 的 `wire_api/api_format` -> base URL 是否 `/chat/completions` -> 默认 `OpenAiResponses`。
- Grok：effective `apiFormat` -> settings `api_format/apiFormat` -> selected model/backend config -> 默认 `OpenAiResponses`。
- Gemini：effective `apiFormat` -> settings `api_format/apiFormat` -> 默认 `GeminiNative`。
- Copilot：请求级动态特例，模型名 `gpt-<major>` 且 major >= 5 但不是 `gpt-5-mini` 时，本次请求切到 `OpenAiResponses`；其它走 `OpenAiChat`。这只改变本次 effective provider，不改 provider 记录。

源码入口：`runtime/providers.rs`、`runtime/upstream.rs::effective_upstream_provider_for_request()`。

### 1.5 source protocol 和 conversion route

`runtime/upstream.rs::source_protocol_from_route()` 当前规则：

| CLI/route | 条件 | source protocol |
|---|---|---|
| Claude | `/v1/messages` 或 `/messages` | `AnthropicMessages` |
| Codex | `/v1/chat/completions` 或 `/chat/completions` | `OpenAiChat` |
| Codex | `/v1/responses`、`/responses`、`/v1/responses/compact`、`/responses/compact` | `OpenAiResponses` |
| Grok | `/v1/responses` | `OpenAiResponses` |
| Gemini | path 包含 `:generateContent` 或 `:streamGenerateContent` | `GeminiNative` |

`runtime/upstream.rs::conversion_route()` 只在 `source_protocol != provider.target_protocol` 时创建 `ConversionRoute`。同协议路径不进入 transformer。

Grok 还有 `/grok/v1` 的本地探测路由；正式模型请求当前只接受 `/grok/v1/responses`。`/grok/v1/chat/completions` 和 `/grok/v1/responses/compact` 会在 `runtime/routes.rs::match_gateway_route()` 被拒绝，不能按 Codex 的 source path 推导规则理解成 Grok 可达接口。

## 2. 通用请求侧兼容

请求 body 的事实源是 `runtime/upstream.rs::build_upstream_body_for_provider()` 和 `runtime/middleware.rs`。当前顺序：

1. 解析入站 JSON。
2. 构造 request-scoped pipeline：`OutboundAdapterCompatMiddleware`、`BillingHeaderCchMiddleware`、必要时 `EnsureMaxTokensMiddleware`。
3. 运行 inbound middleware。`BillingHeaderCchMiddleware` 会从 Claude Code system 文本开头剥离动态 `x-anthropic-billing-header:` / `cch=...` 并保存到 `PipelineContext.billing_cch`。
4. 如果是 Codex Responses 转 Chat/Anthropic，转换前用 `CodexHistoryStore` 补回上一轮缺失的 call item。
5. 只在存在 conversion route 时执行有损检测；默认放过并写 `X-Transformer-Lossy`，用户开启 `lossy_rejection_enabled` 后才拒绝，且 `X-Allow-Lossy: true` 可绕过。
6. 写入或改写最终上游 `model`，剥离 `[1M]` / `[1m]`。
7. Gemini source 转非 Gemini target 且 route streaming 时写 `stream=true`。
8. thinking rectifier 重试路径才执行 `strip_thinking_blocks`；正常请求不会预先删除 thinking。
9. `/responses/compact` 走 compact 专项 compat；普通跨协议请求调用 `convert_request_body_with_context()`；同协议请求直通当前 body。
10. target Gemini 时可由 `GeminiShadowStore` 回放上一轮带 `thoughtSignature` 的 model functionCall。
11. target OpenAI Responses 时执行 `prompt_cache_key` fallback。
12. target OpenAI Chat 时先缓存被 strip 前的 `prompt_cache_key`，再跑 provider pipeline。
13. target OpenAI Chat 后置 `prompt_cache_key` allowlist reinject。
14. xAI native Responses passthrough gate 命中时执行 namespace flatten 和 sanitize。
15. target Anthropic 且 `cache_injection_enabled=true` 时注入 cache_control。

### 2.1 outbound adapter 顺序

`runtime/upstream.rs::apply_outbound_adapter_compat_value()` 当前顺序：

1. `filter_private_outbound_fields()` 递归移除 `_` 开头内部字段，但在 JSON Schema `properties`、`patternProperties`、`definitions`、`$defs` 下保留属性名。
2. 用 `ProviderBodyCompat::from_provider_meta()` 识别 provider 方言。
3. 用 `ReasoningFieldPolicy::from_provider_meta()` 计算 OpenAI Chat assistant reasoning 字段策略。
4. 读取 explicit/legacy/inferred `codexChatReasoning`。
5. provider body compat before generic。
6. target OpenAI Chat 时执行 Codex Chat reasoning 配置。
7. target OpenAI Chat 时执行通用第三方 Chat 兼容清理。
8. 有 conversion route 且非 Gemini source 时，无 tools 清理 `tool_choice/parallel_tool_calls` 或 Anthropic `tool_choice`；Anthropic target 且 tool_choice 强制 tool_use 时移除顶层 `thinking`。
9. provider body compat after generic。
10. target OpenAI Chat 时执行 reasoning field policy，再执行 DeepSeek final reasoning gate。
11. 执行预测式图片/多模态兼容策略。
12. Ollama target 最后投影到 Ollama `/api/chat` wire format。

### 2.2 OpenAI Chat 通用兼容

`normalize_openai_chat_for_provider_compat()` 是发往 OpenAI Chat-compatible provider 前的通用清理：

- 删除顶层 `verbosity`、`prompt_cache_key`。
- 非 DeepSeek 且没有显式 `codexChatReasoning` 要保留 effort 时删除 `reasoning_effort`。
- 过滤 tools，只保留 `type=function` 且有 `function.name` 的工具，移除 `response_custom_tool`。
- `developer` role 改成 `system`。
- system content parts 压成 string。
- 多个 system 合并到首条。
- tool call arguments 空值补 `"{}"`。
- 删除 Google 私有 `thought_signature/thoughtSignature`，以及 `google`、`extra_content/extra_fields` 中包含 signature 的容器。
- 删除不支持 tool call 及对应 tool result。
- 删除空 assistant message。

这些是 runtime provider compat，不属于 transformer roundtrip 语义。

### 2.3 prompt cache

- OpenAI Responses target：最终 body 没有 `prompt_cache_key` 时，从稳定 session 线索 fallback；显式值不覆盖。
- OpenAI Chat target：默认 strip `prompt_cache_key`；只有 allowlist providerType 才 reinject，当前包括 `openai`、`openai-chat`、`kimi`、`kimi-coding`、`moonshot`、`moonshot-v1`、`moonshot-coding`。优先使用 explicit pre-strip key，其次 session hint；没有线索不写默认值或随机值。

测试：`responses_prompt_cache_key_falls_back_to_session_header`、`responses_prompt_cache_key_keeps_explicit_request_value`、`chat_prompt_cache_key_strips_by_default_without_allowlist`、`chat_prompt_cache_key_reinjects_explicit_for_allowlisted_provider`、`chat_prompt_cache_key_reinjects_session_when_allowlisted_and_no_explicit`。

### 2.4 图片/多模态兼容

发送前预测式替换由 provider meta 或 model catalog 显式能力驱动：

- `imageInputPolicy=strip/replace/text_only/unsupported` -> 替换图片块为 `[Unsupported Image]`。
- `preserve/keep/vision/multimodal/image/images` -> 保留。
- `imageCapableModels` 优先保留。
- `textOnlyModels` -> 替换。
- model catalog 里的 `supportsImage=false`、`vision=false`、`attachment=false`、`modalities.input` 不含 `image` 等会触发替换。
- `allowTextOnlyModelHeuristic=true` 才启用模型名启发式；默认不猜。
- 启发式 exact tails 包含 `glm-5.1`、`glm-5.2`（以及 `GLM-5.2[1M]` / `vendor/GLM-5.2` 归一化后的 tail）；不能用 `glm-5.2` 前缀，避免误伤多模态 `glm-5.2v`。

上游错误后的反应式 rectifier：

- 只在 HTTP 400/415/422/501 时尝试同 provider 重试。
- 触发条件二选一：
  1. 错误文本明确 image/media/vision/attachment unsupported；
  2. 自证性 text-only 短语 `only support text` / `only supports text` / `text only` / `text-only`（无需提到 image；覆盖火山 `Model only support text input`）。
- 替换 OpenAI/Anthropic image/image_url、Responses `input_image`、Gemini image `inlineData/fileData` 为文本占位。
- 保留 `cache_control`。

测试：`unsupported_media_rectifier_*`、`predictive_media_policy_*`、`known_text_only_model_matches_glm_5_2_exact_tail_not_multimodal_variant`、`predictive_media_policy_replaces_images_for_glm_5_2_when_heuristic_enabled`。

### 2.5 middleware

- `BillingHeaderCchMiddleware`：request inbound 剥离 Claude Code 动态 billing CCH；target Anthropic 时在 outbound body 和 client-facing JSON/SSE reverse 阶段回填，非 Anthropic target 不泄漏。
- `EnsureMaxTokensMiddleware`：只有 provider meta 显式 `defaultMaxTokens > 0` 时加入。按 target 协议写或截断：
  - Anthropic：`max_tokens`
  - OpenAI Responses：`max_output_tokens`
  - Gemini：`generationConfig.maxOutputTokens`
  - OpenAI Chat：`max_completion_tokens` 优先，否则 `max_tokens`

测试：`provider_pipeline_caps_default_max_tokens_in_upstream_body`、`provider_pipeline_strips_billing_cch_for_non_anthropic_target`、`provider_pipeline_restores_billing_cch_for_anthropic_target`。

## 3. 通用响应侧兼容

响应事实源是 `runtime/upstream.rs::build_gateway_response()`。

### 3.1 streaming 判定

`should_stream_response()` 只对 2xx/3xx 生效：

1. `Content-Type: text/event-stream` 优先进入 SSE 路径。
2. 明确非 SSE Content-Type，例如 `application/json`，不走 SSE wrapper，即使 request body 写了 `stream:true`。
3. 缺少 Content-Type 时，才 fallback 到 request `stream:true` 或 Gemini route streaming。
4. Ollama `application/x-ndjson` / `application/x-json-stream` 是专用例外，只对 Ollama provider_kind 转换。

### 3.2 SSE wrapper 顺序

当前流式路径顺序：

1. xAI native Responses namespace restore，且只在 HTTP 2xx restore。
2. Gemini shadow record。
3. Bailian OpenAI Chat SSE filter。
4. xAI/Grok OpenAI Chat SSE filter。
5. Ollama NDJSON -> OpenAI Chat SSE。
6. protocol SSE conversion。
7. Codex Responses SSE record。
8. request-scoped reverse pipeline middleware。

provider raw stream filter 必须发生在 protocol SSE conversion 之前。

### 3.3 非流 JSON 顺序

非流 JSON 路径：

1. Ollama 2xx/3xx JSON 先转 OpenAI Chat JSON。
2. compact success/error 走 compact 专项转换。
3. protocol response/error conversion。
4. xAI native Responses 2xx restore。
5. reverse pipeline response middleware。
6. 用最终 body 和原始 upstream body 共同做 failure/empty response 分类。

### 3.4 rectifier 默认行为

当前 `ProxyGatewaySettings` 默认：

| 设置 | 默认 | 行为 |
|---|---:|---|
| `thinking_rectifier_enabled` | `true` | Claude/Anthropic target 非流 4xx thinking/signature 兼容错误后，清理 thinking/signature 并同 provider 重试一次 |
| `responses_encrypted_content_rectifier_enabled` | `true` | OpenAI Responses target 非 compact、非流 4xx 且明确 encrypted_content 无法验证/解密时，删除失效 reasoning item 并重试一次 |
| `thinking_budget_rectifier_enabled` | `true` | Anthropic target 非流 4xx thinking budget 类问题时走预算修正重试 |
| `cache_injection_enabled` | `false` | target Anthropic 时才注入 cache_control |
| `lossy_rejection_enabled` | `false` | 有损转换默认放过并写 warning；显式开启后才硬拒绝 |

xAI native Responses passthrough 不是用户开关控制，而是严格自动门控，详见 5.2。

## 4. 当前 profile 概览

以下由当前 `tauri/resources/gateway_provider_profiles.json` 抽取。`*` 表示该 tool 的默认 endpoint。

| profile | providerType | compat 声明 | 当前 endpoint 摘要 |
|---|---|---|---|
| `deepseek` | `deepseek` | DeepSeek Chat/Anthropic | Claude `anthropic*`/`openai_chat`；Codex/Gemini/Grok `openai_chat*`/`anthropic_messages` |
| `zai_cn` / `zai_en` | `zai` | Z.ai Chat | Claude `anthropic*`/`openai_chat`；Codex/Gemini/Grok `openai_chat*`/`anthropic_messages` |
| `doubao` | `doubao` | Doubao metadata | Claude `anthropic*`/`openai_responses`；Codex/Gemini/Grok `openai_responses*`/`anthropic_messages` |
| `bailian` / `bailian_coding` | `bailian` | Bailian tool merge/SSE filter | Claude `anthropic*`/`openai_responses`；Codex/Gemini/Grok `openai_responses*`/`anthropic_messages` |
| `moonshot` / `kimi_coding` | `moonshot` | Moonshot Chat/Anthropic | Claude `anthropic*`/`openai_chat`；Codex/Gemini/Grok `openai_chat*`/`anthropic_messages` |
| `modelscope` | `modelscope` | remove metadata | Claude `anthropic*`/`openai_chat`；Codex/Gemini/Grok `openai_chat*`/`anthropic_messages` |
| `longcat` | `longcat` | Chat content array | Claude `anthropic*`/`openai_responses`；Codex/Gemini/Grok `openai_responses*`/`anthropic_messages` |
| `mimo` / `mimo_token_plan` | `mimo` | Anthropic tool thinking | Claude `anthropic*`/`openai_responses`；Codex/Gemini/Grok `openai_responses*`/`anthropic_messages` |
| `openrouter` | `openrouter` | reasoning object/field | Claude/Codex/Gemini/Grok 都是 `openai_chat*` |
| `siliconflow_cn` / `siliconflow_en` | `siliconflow` | Codex Chat `enable_thinking` | Codex/Gemini/Grok `openai_chat*` |
| `stepfun_cn` / `stepfun_ai` | `stepfun` | Codex Chat low/high effort | Codex/Gemini/Grok `openai_chat*` |
| `minimax_cn` / `minimax_global` | `minimax` | Codex Chat `reasoning_split` | Claude `anthropic*`/`openai_chat`；Codex/Gemini/Grok `openai_chat*`/`anthropic_messages` |
| `ollama` | `ollama` | Ollama `/api/chat` | Claude/Codex/Gemini/Grok 都是 `openai_chat*` |
| `github_copilot` | `github_copilot` | Copilot headers/token/dynamic route | Codex/Gemini/Grok `openai_chat*`，runtime 可请求级切 Responses |
| `xai` | `xai` | xAI Chat/Responses | Codex `openai_chat*` + `openai_responses`；Gemini/Grok `openai_chat*` |

注意：

- 有些 profile 当前默认 endpoint 不是 OpenAI Chat，但 runtime 仍存在 Chat 兼容分支；只有 provider target protocol 实际为 Chat 时才会触发。
- SiliconFlow、StepFun、MiniMax 没有独立 `ProviderBodyCompat` 分支；它们的当前兼容主要通过 Codex Chat reasoning meta/inference 和通用 Chat transformer/parser 覆盖。

## 5. 逐 provider/channel 兼容

### 5.1 OpenAI-like 与 Codex official

触发条件：

- 普通 OpenAI-like Chat/Responses 走通用 target protocol 兼容。
- Codex official adapter 触发于 `providerType=codex|openai-codex|chatgpt-codex|codex-official` 且 target protocol 为 `OpenAiResponses`。
- `category=official` provider 仍不参与 Gateway 候选；Codex official adapter 不改变这个安全边界。

请求侧：

- Codex official Responses body 强制 `stream=true`、`store=false`、`parallel_tool_calls=true`。
- 移除 `max_tokens`、`max_completion_tokens`、`metadata`。
- 确保 `include` 包含 `reasoning.encrypted_content`。
- 确保 `reasoning.summary="auto"`。
- headers 补 `Accept: text/event-stream`。
- 缺 `Originator` 时写 `Originator: ai-toolbox`。
- 缺 `Session_id` 时从 body `session_id` 或 `x-codex-turn-metadata.session_id` 推导。
- 保留客户端已有 `Originator`、`Session_id`、`Chatgpt-Account-Id`。
- 缺 `Chatgpt-Account-Id` 时尽力从 bearer JWT payload 的 OpenAI/ChatGPT account claim 解析。

响应侧：

- 官方 Codex 上游可能被强制流式。客户端非流时 runtime 聚合 Responses SSE 为 Responses JSON，再按需要做 response conversion。
- 聚合必须等待 terminal event；缺 terminal event 按连接错误进入 retry/failover。

源码：

- `runtime/upstream.rs::apply_codex_official_responses_body_compat()`
- `runtime/upstream.rs::inject_codex_official_headers()`
- `runtime/upstream.rs::aggregate_sse_stream_for_non_streaming_client()`

测试：

- `provider_body_compat_codex_official_responses_forces_required_fields`
- `codex_official_headers_set_originator_accept_and_session`
- `codex_official_headers_preserve_client_originator_and_session`
- `codex_official_headers_derive_account_id_from_jwt_when_missing`
- `codex_official_sse_aggregate_*`

### 5.2 xAI / Grok

触发条件：

- Chat 兼容：`providerType=xai|x-ai|grok`，通常 target OpenAI Chat。
- native Responses passthrough：必须同时满足：
  - source protocol 是 `OpenAiResponses`
  - conversion route 为 `None`
  - target protocol 是 `OpenAiResponses`
  - provider kind 是 Xai，即 effective `providerType=xai|x-ai|grok`
- `compat` 里的 `xai_responses_passthrough` 只是 catalog 登记，不是生产开关。

请求侧，OpenAI Chat：

- 对 `grok-4.5` / `grok-4` 删除 `reasoning_effort`、`presence_penalty`、`frequency_penalty`、`stop`。
- 对 `grok-3` / `grok-3-mini` 删除 `presence_penalty`、`frequency_penalty`、`stop`。
- 支持 `xai/grok-*` 前缀归一后判断。

请求侧，native Responses：

- 从原始 namespace tools 建 request-local restore map。
- namespace children function 提升为顶层 function，名称使用 `flatten_namespace_tool_name()`。
- 同步改写 input history 的 `function_call.name/namespace`。
- namespace `tool_choice` 降为 `"auto"`；其它指向 namespace child 的 choice 改写为 flat name。
- flat name 与顶层 function/custom 或其它 namespace child 冲突时 fail closed，返回本地 RequestSchema。
- 删除顶层 `prompt_cache_retention`、`safety_identifier`。
- 对 `grok-4.5` 清理 presence/frequency penalty、stop。
- 递归删除 `external_web_access`。
- `input[].type=additional_tools` 提升到顶层 `tools` 并去重，carrier item 从 input 删除。
- reasoning item 的 `content:null` 删除 content。
- tools 只保留 allowlist：`function`、`web_search`、`x_search`、`image_generation`、`collections_search`、`file_search`、`code_execution`、`code_interpreter`、`mcp`、`shell`。
- `tool_choice` 指向被删除/unsupported tool 时删除。

响应侧：

- xAI Chat SSE filter 丢弃 choices 中全是空 delta、无 finish_reason、无 usage 的 chunk。
- native Responses JSON/SSE 只在 HTTP 2xx 恢复 function_call namespace；3xx/4xx/5xx 不恢复。
- SSE restore 对 `[DONE]` 原样透传。

开关/默认：

- native Responses 兼容是默认自动严格门控，不是用户开关。

源码：

- `runtime/compat/xai_responses.rs`
- `runtime/upstream.rs::should_apply_xai_responses_passthrough()`
- `runtime/upstream.rs::maybe_filter_xai_openai_chat_sse_stream()`

测试：

- `xai_responses_passthrough_gate_accepts_xai_provider_aliases`
- `xai_responses_passthrough_gate_requires_explicit_responses_source`
- `xai_responses_passthrough_scrubs_native_responses_body`
- `xai_responses_passthrough_skips_non_xai_provider`
- `xai_stream_filter_drops_empty_delta_chunks`
- `provider_body_compat_xai_chat_strips_model_specific_unsupported_fields`
- `provider_body_compat_xai_chat_strips_grok_45_and_prefixed_model_ids`
- `runtime/compat/xai_responses.rs` 内 namespace flatten/restore/sanitize 单测

### 5.3 DeepSeek

触发条件：

- `providerType=deepseek`。
- Chat、Anthropic target 和 legacy Completion path 各有不同 runtime 分支。

请求侧，OpenAI Chat：

- `response_format.type=json_schema` 降为 `json_object`，移除 `json_schema`。
- 按 `reasoning_effort` 写 `thinking.type=enabled|disabled`。
- disabled 时移除 `reasoning_effort` 并清理 assistant reasoning 字段。
- enabled 时 effort 映射：`max/xhigh -> max`，其它 -> `high`。
- 有 tool_calls 的 assistant 历史保留/回填 `reasoning_content`；无 tool_calls 的 assistant 历史移除 `reasoning_content` 和 `reasoning`。

请求侧，Anthropic target：

- 规范化 assistant tool_use 历史 thinking：删除 signature，空 thinking 补 `"tool call"`，`redacted_thinking` 转普通占位 thinking，无 thinking 时插入 `thinking:"tool call"`。
- `thinking.type=disabled` 时删除 `output_config.effort`，必要时删除空 `output_config`，并删除 `reasoning_effort`。

请求侧，legacy Completion：

- Codex/OpenAI `/v1/completions` 或 `/completions` + DeepSeek provider 改写 URL 到 `/beta/completions`。
- 该路径跳过 Chat body adapter，不进入 transformer 聊天矩阵。

响应侧：

- 没有 DeepSeek 专用 response wrapper；走通用 response conversion、failure classification、usage parser。

源码：

- `runtime/upstream.rs::apply_openai_chat_provider_body_compat_before_generic()`
- `runtime/upstream.rs::apply_anthropic_provider_body_compat()`
- `runtime/upstream.rs::is_deepseek_legacy_completion_forward()`

测试：

- `provider_body_compat_deepseek_chat_rewrites_json_schema_thinking_and_custom_tools`
- `provider_body_compat_deepseek_chat_preserves_reasoning_with_tool_calls_and_strips_without`
- `provider_body_compat_deepseek_anthropic_disabled_thinking_strips_effort_fields`
- `deepseek_legacy_completion_route_uses_beta_path`
- `deepseek_legacy_completion_body_skips_chat_adapter`

### 5.4 Moonshot / Kimi

触发条件：

- `providerType=moonshot|kimi`。

请求侧：

- OpenAI Chat target：`response_format.type=json_schema` 降为 `json_object`；assistant 有 tool_calls 且无非空 `reasoning_content` 时补 `"tool call"`。
- Anthropic target：规范化 assistant tool_use 历史 thinking。
- Codex -> Chat reasoning 矩阵可用 `thinking` + `reasoning_content`。
- OpenAI Chat `prompt_cache_key` 对 `kimi` / `moonshot` allowlist provider 可 reinject。

响应侧：

- Moonshot/Kimi Anthropic-compatible usage 解析按 provider-aware 规则处理 `cached_tokens` 和可能的负 input token 折扣；该逻辑属于 usage/cost 兼容，不在 transformer。

测试：

- `provider_body_compat_anthropic_reasoning_vendor_normalizes_tool_thinking_history`
- `chat_prompt_cache_key_reinjects_explicit_for_allowlisted_provider`
- 当前未见单独锁定 Moonshot Chat JSON schema 降级和 Chat tool-call reasoning backfill 的专项测试；新增或调整该分支时应补精确测试。

### 5.5 Z.ai / GLM / 智谱

触发条件：

- `providerType=zai|zhipu|glm|chatglm|bigmodel|big-model`。

请求侧，OpenAI Chat：

- JSON Schema response_format 降为 `json_object`。
- `metadata.user_id/request_id` 提升为顶层字段。
- 无 request_id 时生成 `req_<timestamp>`。
- 有 `tool_choice` 时强制为 `auto`。
- 按 `reasoning_effort` 写 `thinking.type`。
- Codex -> Chat reasoning 矩阵可写 `thinking`。

响应侧：

- 没有专用 response wrapper；走通用 response conversion。

测试：

- `provider_body_compat_zai_chat_moves_metadata_and_forces_auto_tool_choice`

### 5.6 Doubao / Volces

触发条件：

- `providerType=doubao|doubaoseed|doubao-seed|volces`。

请求侧：

- OpenAI Chat target：`metadata.user_id/request_id` 提升，缺 request_id 时生成；按 `reasoning_effort` 写 `thinking.type`。通用 Chat 清理之后顶层 `reasoning_effort` 不直传。
- OpenAI Responses target：删除 `metadata`。

响应侧：

- 没有 Doubao 专用 response wrapper。

注意：

- 当前 profile 默认多为 Anthropic 或 Responses endpoint；Chat branch 只有实际 target 为 OpenAI Chat 时触发。

测试：

- `provider_body_compat_doubao_chat_extracts_metadata_and_generates_request_id`

### 5.7 Bailian / DashScope / Qwen / Aliyun

触发条件：

- `providerType=bailian|dashscope|aliyun`。

请求侧，OpenAI Chat：

- 合并连续 assistant tool-call-only message。
- 有 side-effect 字段的 message 不合并，避免丢失 provider 附加语义。
- Codex -> Chat reasoning 矩阵可写 `enable_thinking`。

响应侧，OpenAI Chat SSE：

- 只在 target OpenAI Chat 且 provider kind Bailian 时启用 raw stream filter。
- 见到 `tool_calls` 后，后续文本 delta 先缓冲，在 finish 前作为独立 text delta 重发，避免 tool call 后文本顺序问题。
- 如果某个 tool call 已累计非空 arguments，上游再发 `{}` 参数片段时改为空字符串，避免重复空 args 污染已累计参数。

注意：

- 当前 profile 默认多为 Anthropic 或 Responses endpoint；Chat branch 只有实际 target 为 OpenAI Chat 时触发。
- SSE filter 必须保持在 runtime raw upstream SSE adapter，不能下沉到 transformer。

测试：

- `provider_body_compat_bailian_chat_merges_consecutive_tool_call_messages`
- `provider_body_compat_bailian_keeps_tool_call_messages_with_side_effect_fields`
- `bailian_stream_filter_buffers_text_after_tool_calls_until_finish`
- `bailian_stream_filter_drops_duplicate_empty_tool_arguments`

### 5.8 OpenRouter

触发条件：

- `providerType=openrouter|open-router`，通常 target OpenAI Chat。

请求侧：

- 顶层 `reasoning_effort` 移到 `reasoning.effort`。
- effort 映射：`max/xhigh -> xhigh`；`high/medium/low/minimal` 保留；`none/off/disabled -> none`。
- 默认 reasoning field policy 为 `reasoning`，除非 meta 显式覆盖。
- Codex -> Chat reasoning 矩阵使用 `reasoning.effort`。

响应侧：

- OpenAI Chat assistant 历史 reasoning 字段按 `reasoning` 策略输出/保留。

测试：

- `provider_body_compat_openrouter_moves_reasoning_effort_to_reasoning_object`
- `provider_body_compat_openai_chat_applies_reasoning_field_policy`
- `codex_chat_reasoning_config_maps_openrouter_effort_object`

### 5.9 SiliconFlow

触发条件：

- 当前没有 `ProviderBodyCompat::SiliconFlow`。
- 兼容由 `codexChatReasoning` explicit meta 或 `infer_codex_chat_reasoning_config()` 在明确 effective `providerType=siliconflow` 且 target OpenAI Chat 时触发。

请求侧：

- Codex -> Chat reasoning 写 `enable_thinking`。
- 不传 effort。
- output reasoning 期望为 `reasoning_content`。

响应侧：

- 由通用 OpenAI Chat transformer/parser 提取 `reasoning_content`。

注意：

- Gemini/Grok/Claude endpoint 不会从 profile 解析 Codex-only `codexChatReasoning` 字段；但如果最终 target 是 OpenAI Chat，且 effective `providerType` 明确为 `siliconflow`，runtime fallback inference 仍会触发 `enable_thinking` 兼容。区别是：显式 profile 配置只来自 Codex endpoint，fallback inference 来自明确 providerType/apiFormat，不来自模型名猜测。

测试：

- `codex_chat_reasoning_config_strips_effort_for_thinking_only_provider`
- `codex_chat_reasoning_custom_qwen_model_does_not_infer_provider_compat`
- 当前测试覆盖的是同类 `enable_thinking` 显式配置和 custom provider 负例，未见以 `providerType=siliconflow` 命名的专项测试。

### 5.10 StepFun

触发条件：

- 当前没有 `ProviderBodyCompat::StepFun`。
- 兼容由 `codexChatReasoning` 或明确 effective `providerType=stepfun` 的 inference 触发。

请求侧：

- `thinkingParam=none`。
- `effortParam=reasoning_effort`。
- `effortValueMode=low_high`，`minimal/low -> low`，其它 -> `high`。
- 模型名只在已识别 providerType 为 StepFun 后用于能力细分，例如 `2603`；自定义 provider 不会因为模型名包含 stepfun/2603 自动套规则。

响应侧：

- 走通用 Chat。

测试：

- `codex_chat_reasoning_custom_provider_model_names_do_not_infer_provider_compat`
- 当前未见以 `providerType=stepfun` 和 `2603` 能力细分命名的专项测试；修改该 inference 分支时应补精确测试。
- `codex_chat_reasoning_explicit_meta_overrides_inference`

### 5.11 MiniMax

触发条件：

- 当前没有 `ProviderBodyCompat::MiniMax`。
- 兼容由 `codexChatReasoning` 或明确 effective `providerType=minimax` 的 inference 触发。

请求侧：

- Codex -> Chat reasoning 写 `reasoning_split`。
- output format 声明为 `reasoning_details`。

响应侧：

- OpenAI Chat transformer/parser 已能提取 `reasoning_details`。

注意：

- text-only 图片预测启发式名单包含 MiniMax 相关模型前缀，但只有 `allowTextOnlyModelHeuristic=true` 时启用。
- 如 MiniMax endpoint 未来需要额外 body 字段清理，应新增 runtime adapter 和测试，不能只改 profile compat 描述。

测试：

- `codex_chat_reasoning_custom_provider_model_names_do_not_infer_provider_compat`
- 当前未见以 `providerType=minimax` 命名的 `reasoning_split` 专项测试；修改该 inference 分支时应补精确测试。

### 5.12 MiMo

触发条件：

- `providerType=mimo|xiaomimimo|xiaomi-mimo`。

请求侧：

- OpenAI Chat target：assistant tool_call 缺 `reasoning_content` 时补 `"tool call"`。
- Anthropic target：规范化 assistant tool_use 历史 thinking。
- Codex -> Chat reasoning 矩阵可写 `thinking` + `reasoning_content`。

响应侧：

- 没有专用 response wrapper。

测试：

- `provider_body_compat_anthropic_reasoning_vendor_normalizes_tool_thinking_history` 覆盖同类 Anthropic tool thinking 历史规范化，但当前样例使用 Moonshot provider。
- 当前未见 MiMo Chat `reasoning_content` backfill 或 MiMo providerType 专项测试；修改该分支时应补精确测试。

### 5.13 LongCat

触发条件：

- `providerType=longcat`。

请求侧：

- OpenAI Chat target after generic：所有 message `content` 归一为 array。
- string -> text part。
- null/none -> empty text。
- object -> array[object]。
- 其它类型 -> text。
- Anthropic target 作为 `AnthropicPlatform::LongCat`，使用 Bearer auth，并按非 Direct/非 Bedrock 平台过滤 native web_search。

响应侧：

- 没有专用 response wrapper。

测试：

- `provider_body_compat_longcat_chat_forces_message_content_arrays`

### 5.14 ModelScope

触发条件：

- `providerType=modelscope|model-scope`。

请求侧：

- OpenAI Chat target：删除 `metadata`。
- OpenAI Responses target：删除 `metadata`。
- profile 可声明 Codex -> Chat reasoning 配置。

响应侧：

- 没有专用 response wrapper。

测试：

- 当前未见单独锁定 ModelScope remove metadata 的专项测试；`provider_body_compat_detects_canonical_provider_type_aliases` 只覆盖 provider kind 识别。新增或调整 ModelScope 兼容时必须补精确测试。

### 5.15 Anthropic Direct / Bedrock / Vertex

触发条件：

- target protocol 必须是 `AnthropicMessages`。
- `providerType=bedrock|anthropic-bedrock|aws-bedrock` -> Bedrock。
- `providerType=vertex|anthropic-vertex|claude-vertex` -> Anthropic Vertex。
- `providerType=anthropic|claude|direct|claude-code` -> Direct Anthropic。
- `providerType=longcat|long-cat` -> LongCat platform。

请求侧，header/path/auth：

- Bedrock URL 使用 `/model/{model}/invoke` 或 `/model/{model}/invoke-with-response-stream`。
- Vertex URL 使用 base URL 中 project/location 前缀拼 `publishers/anthropic/models/{model}:rawPredict` 或 `:streamRawPredict`。
- Bedrock header/body version 为 `bedrock-2023-05-31`。
- Vertex header/body version 为 `vertex-2023-10-16`。
- Direct Anthropic 默认 `anthropic-version: 2023-06-01`。
- Bedrock/LongCat 使用 Bearer auth；Direct Anthropic 默认 `x-api-key`。

请求侧，body：

- Bedrock body 写 `anthropic_version=bedrock-2023-05-31`，移除 `model` 和 `stream`。
- Vertex body 写 `anthropic_version=vertex-2023-10-16`。
- Direct Anthropic body 含 native `web_search` tool 时注入 `anthropic-beta: web-search-2025-03-05` header。
- Bedrock native `web_search` 通过 body `anthropic_beta=["web-search-2025-03-05"]`。
- Vertex/LongCat/普通非 Direct、非 Bedrock 平台过滤 native web_search tool。

响应侧：

- 没有平台专用 response wrapper；走 Anthropic response/SSE conversion 和通用 failure classification。

测试：

- `anthropic_bedrock_provider_uses_model_invoke_path_and_version_header`
- `anthropic_bedrock_body_clears_model_and_stream`
- `anthropic_direct_web_search_adds_beta_header`
- `anthropic_vertex_filters_native_web_search_tool`
- `anthropic_vertex_provider_uses_raw_predict_path_and_version_header`

### 5.16 Gemini Direct / Vertex

触发条件：

- Gemini Native target 由 provider target protocol 决定。
- Gemini Vertex body compat 来自 `providerType=vertex|googlevertex|google-vertex|geminivertex|gemini-vertex`。

请求侧：

- target Gemini + streaming 自动补 `alt=sse`。
- Gemini API version 从 provider base URL 推断，支持 `v1` / `v1beta` / `v1alpha`。
- Gemini source 转非 Gemini target 时过滤 `alt=sse` 和 `key=` query。
- Gemini Vertex target 删除 `contents[].parts[].functionCall.id` 和 `functionResponse.id`。

响应侧：

- target Gemini 的原始 SSE 可被 `GeminiShadowStore` 旁路记录，用于后续 reliable session 的 thoughtSignature shadow 回放。
- Gemini response/SSE 的协议结构转换由 transformer 处理。

测试：

- `outbound_adapter_strips_gemini_function_ids_for_vertex`
- `conversion_route_rewrites_claude_to_gemini_native_generate_content_path`
- `conversion_route_rewrites_claude_to_gemini_native_streaming_path_and_query`

### 5.17 GitHub Copilot

触发条件：

- `providerType=copilot|github-copilot|githubcopilot`。

请求侧，target protocol：

- 请求级模型 `gpt-<major>` 且 major >= 5、并且不是 `gpt-5-mini` -> OpenAI Responses。
- 其它模型 -> OpenAI Chat。
- warmup 降级到 `gpt-5-mini` 只在 provider 是 Copilot、请求头包含 `anthropic-beta`、body 不是 compact、initiator 为 user、无 tools 时执行。

请求侧，token/header：

- API key field 是 GitHub-token-like，或 token 前缀为 `ghp_`、`github_pat_`、`gho_`、`ghu_`、`ghs_`、`ghr_` 时，先 exchange 到 Copilot bearer token。
- raw Copilot token 直接 Bearer。
- exchange endpoint 是 `https://api.github.com/copilot_internal/v2/token`。
- 结果按 token hash 缓存到过期前 5 分钟。
- 注入/覆盖 fingerprint headers：
  - `Editor-Version`
  - `Editor-Plugin-Version`
  - `User-Agent`
  - `Copilot-Integration-Id`
  - `Openai-Intent`
  - `X-Github-Api-Version`
  - `X-Vscode-User-Agent-Library-Version`
- body 有图片时写 `Copilot-Vision-Request: true`。
- 按 body/header 计算 `X-Initiator`。
- subagent 写 `X-Interaction-Type: conversation-subagent`。
- 基于 session/body 生成确定性 interaction/request/task ids。

请求侧，body：

- Claude 4.x Copilot model id 归一化，覆盖 date/dash/dot 和 `[1M]` 形态。
- Chat target 删除 assistant content 中 `thinking` / `redacted_thinking`。
- Chat orphan tool message 降级成 user 文本 `[Tool result for ...]`。
- Responses orphan `function_call_output` 降级成 user message。
- Responses `function_call` item `id=call_id`；缺 name 时 name=`function`。

响应侧：

- 没有专用 response wrapper；动态 route 后按 target protocol 的通用 response conversion。

注意：

- 不包含 GitHub device-code 登录 UI、账号存储或 live model list fallback。
- Copilot profile 必须保存 origin base URL，不能 fixed full URL 到 `/chat/completions`，否则会绕过动态 `/responses` path。

测试：

- `copilot_model_uses_responses_api_matches_axonhub_rule`
- `copilot_effective_provider_switches_chat_and_responses_by_model`
- `copilot_warmup_downgrades_model_before_route_selection`
- `copilot_warmup_does_not_downgrade_tool_or_agent_requests`
- `copilot_token_exchange_detection_is_explicit_or_github_token_shaped`
- `copilot_token_exchange_sends_github_token_and_caches_response`
- `copilot_openai_chat_body_normalizes_model_and_sanitizes_orphan_tool_message`
- `copilot_responses_body_normalizes_function_item_ids_and_orphan_outputs`
- `copilot_headers_override_forwarded_fingerprint_and_infer_agent_turn`
- `copilot_headers_detect_compact_subagent_and_vision`

### 5.18 Ollama

触发条件：

- `providerType=ollama|ollama-chat|ollamachat` 且 target OpenAI Chat。
- 或 `data.meta.apiFormat=ollama/chat` 且 target OpenAI Chat。

请求侧：

- Gateway target protocol 仍视作 OpenAI Chat，不新增第五种 transformer 协议。
- 上游 URL 使用 `/api/chat`，并剥离 base URL 尾部 `/v1`。
- 最后一步把 OpenAI Chat body 投影为 Ollama wire format：
  - `model`
  - `messages[].role/content`
  - `image_url` data URL 去前缀放 `images[]`
  - 普通 URL 原样放 `images[]`
  - reasoning field -> `thinking`
  - `temperature/top_p/top_k/max_tokens/max_completion_tokens` -> `options`
  - `max_*` -> `options.num_predict`
  - `stop` -> `options.stop` array
  - `response_format=json_object` -> `"json"`
  - `response_format=json_schema` -> schema object
  - `stream` 缺省 false

响应侧：

- 非流 Ollama JSON 先转 OpenAI Chat response。
- Ollama NDJSON / x-json-stream 先转 OpenAI Chat SSE，再进入 protocol SSE conversion。
- usage、done_reason、tool_calls、thinking 都在 Ollama adapter 中归一到 Chat 形态。

测试：

- `ollama_chat_url_uses_api_chat_and_strips_v1_base_suffix`
- `ollama_body_compat_converts_openai_chat_request_shape`
- `ollama_json_response_converts_to_openai_chat_response`
- `ollama_ndjson_stream_converts_to_openai_chat_sse`

### 5.19 Custom / legacy provider

触发条件：

- 没有 `gatewayProfile`，或 profile 解析失败/不匹配，runtime 使用 legacy meta。
- 如果 legacy meta 也没有 `providerType`，fallback 到 provider record `category`。

请求侧：

- 仍执行通用 source/target protocol 转换、header/auth、URL/path、Chat 通用清理、tool controls 清理、prompt cache 默认策略、lossy 策略、predictive media policy 等。
- 不会仅凭模型名或 Base URL 猜 DeepSeek/Qwen/GLM/MiniMax/MiMo/StepFun 等供应商方言。

响应侧：

- 走通用 response conversion、streaming 判断、failure/empty response classification。

注意：

- 如果用户希望 custom provider 获得内置供应商方言，应通过内置 profile endpoint 或显式 legacy meta 选择该兼容行为，而不是依赖模型名。

测试：

- `codex_chat_reasoning_custom_deepseek_model_does_not_infer_provider_compat`
- `codex_chat_reasoning_custom_qwen_model_does_not_infer_provider_compat`
- `codex_chat_reasoning_custom_provider_model_names_do_not_infer_provider_compat`

## 6. Codex Chat reasoning 矩阵

该矩阵只对 target OpenAI Chat 生效，入口是 `runtime/upstream.rs::apply_codex_chat_reasoning_config()` 和 `infer_codex_chat_reasoning_config()`。

优先级：

1. explicit `meta.codexChatReasoning/codex_chat_reasoning`。
2. 缺 explicit 时，根据明确 effective `providerType/apiFormat` fallback。
3. 自定义 provider 不能因为 body `model` 字符串像某供应商而触发 fallback。
4. 模型名只允许在已识别 provider 内做能力细分，例如 StepFun 的 `2603`。

当前支持：

| 模式 | 出站字段 |
|---|---|
| `thinkingParam=thinking` | `thinking:{type:enabled|disabled}` |
| `enable_thinking` | `enable_thinking: true/false` |
| `reasoning_split` | `reasoning_split: true/false` |
| `none` | 不写 thinking 参数 |
| `effortParam=reasoning_effort` | 写顶层 `reasoning_effort` |
| `effortParam=reasoning.effort` | 写 `reasoning.effort` |

disabled 行为：

- disabled 时删除顶层 `reasoning_effort`。
- 如果 effortParam 是 `reasoning.effort`，写 `reasoning:{effort:none}`。
- `supportsEffort=false` 时删除 `reasoning_effort`。

effort 映射：

- `deepseek`：`max/xhigh -> max`，其它 -> `high`。
- `low_high`：`minimal/low -> low`，其它 -> `high`。
- `openrouter`：`max/xhigh -> xhigh`，`high/medium/low/minimal` 保留，其它 none。
- `passthrough`：支持 `minimal/low/medium/high/xhigh`。

inferred provider：

- OpenRouter -> `reasoning.effort`，openrouter mode。
- SiliconFlow -> `enable_thinking`，不支持 effort，output `reasoning_content`。
- DeepSeek -> `thinking`，支持 effort，deepseek mode。
- StepFun -> 只有 provider 已识别后，模型含 `2603` 才 supports effort，low_high mode。
- Kimi/Moonshot -> `thinking`，不支持 effort，output `reasoning_content`。
- GLM/Z.ai/Zhipu -> `thinking`，不支持 effort。
- Qwen/DashScope/Bailian -> `enable_thinking`。
- MiniMax -> `reasoning_split`，output `reasoning_details`。
- MiMo -> `thinking`。

测试：

- `codex_chat_reasoning_config_maps_deepseek_effort_and_thinking`
- `codex_chat_reasoning_config_maps_openrouter_effort_object`
- `codex_chat_reasoning_config_strips_effort_for_thinking_only_provider`
- `codex_chat_reasoning_infers_deepseek_without_explicit_meta`
- `codex_chat_reasoning_custom_deepseek_model_does_not_infer_provider_compat`
- `codex_chat_reasoning_infers_openrouter_platform_before_model`
- `codex_chat_reasoning_custom_qwen_model_does_not_infer_provider_compat`
- `codex_chat_reasoning_custom_provider_model_names_do_not_infer_provider_compat`
- `codex_chat_reasoning_explicit_meta_overrides_inference`

## 7. Header、path、auth 兼容

通用规则：

- 入站 `authorization`、`x-api-key`、`x-goog-api-key`、`x-goog-api-client` 等不会直接透传为上游 auth；runtime 重新按 provider auth strategy 注入。
- `ProviderAuthStrategy`：
  - Anthropic API key -> `x-api-key`
  - Bearer -> `Authorization: Bearer ...`
  - Google API key -> `x-goog-api-key`
  - Google OAuth -> Bearer + `x-goog-api-client: GeminiCLI/1.0`
- converted route query 过滤 `beta=...`。
- Gemini source 转非 Gemini target 时过滤 `alt=sse` 和 `key=...`。
- target Gemini + streaming 自动补 `alt=sse`。
- provider full URL：`is_full_url=true` 或 base URL suffix `##` 时只 merge query，不追加默认 path。

特殊 path：

- DeepSeek legacy completion -> `/beta/completions`。
- Ollama -> `/api/chat`，剥离 base URL 尾部 `/v1`。
- Anthropic Bedrock -> `/model/{model}/invoke` 或 `/invoke-with-response-stream`。
- Anthropic Vertex -> `/publishers/anthropic/models/{model}:rawPredict` 或 `:streamRawPredict`。
- Copilot -> 按本次 dynamic target 选择 `/chat/completions`、`/responses` 或 `/responses/compact`。

## 8. 维护流程

新增或调整 provider/channel 兼容时，按以下步骤：

1. 先读 [`docs/gateway-protocol-conversion.md`](gateway-protocol-conversion.md)，确认架构边界、参考项目 baseline、查询入口和同步流程。
2. 再读本文，确认当前 provider 的触发条件、入参/出参兼容、默认行为、开关和测试。
3. 读取最近的模块级 `AGENTS.md`：
   - `tauri/src/coding/proxy_gateway/AGENTS.md`
   - `tauri/src/coding/proxy_gateway/transformer/AGENTS.md`
4. 如果改的是 profile/channel 身份，更新 `tauri/resources/gateway_provider_profiles.json`、`provider_profiles.rs` 白名单和 profile resolver 测试。
5. 如果改的是 body/header/path/auth/stream filter/rectifier，放在 runtime provider compat、middleware、side store 或 upstream 编排；不要把 provider type/base URL/API key/model catalog 传进 transformer。
6. 如果改的是 provider-agnostic 公共协议结构，才改 transformer。
7. 补最贴近用户路径的回归测试，尤其包含 custom provider 不误触发的负例。
8. 代码或测试改完后：
   - provider/channel 细节变了，必须更新本文。
   - 架构、IR、SSE 生命周期、pipeline/side store、参考项目 baseline 或同步结论变了，必须更新架构主文档。
   - 跨边界改动同时更新两份文档。
9. 只改本文时，至少运行陈旧表述搜索和 `git diff --check`，并人工核对源码路径和测试名称仍存在。

参考项目同步时，本文不单独维护 baseline commit；baseline、远端、目标 ref、查询入口和吸收日志归架构主文档。吸收后的 provider/channel wire 事实、开关和测试索引必须落到本文。
