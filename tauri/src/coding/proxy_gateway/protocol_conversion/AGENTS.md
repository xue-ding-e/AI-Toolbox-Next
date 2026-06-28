# Protocol Conversion Module Notes

## 一句话职责

- 在 Proxy Gateway 请求路径中提供独立、可复用的 AI 协议载荷转换：Anthropic Messages、OpenAI Chat Completions、OpenAI Responses、Gemini Native 的 JSON 与 SSE 聊天协议互转。

## Source of Truth

- 转换模块的 Source of Truth 是统一中间模型新内核：`llm::Request` / `llm::Response`、Inbound/Outbound transformer、`StreamKernel`，以及 `AiProtocol`、`ConversionRoute`、`convert_request_body`、`convert_response_body`、`convert_error_response_body` 和 `convert_sse_stream` 的行为与测试。
- Runtime 只负责判断入站 route、读取 provider 的 target protocol、拼上游 path/header/auth、保存 `request_body` 与 `upstream_request_body` 快照；协议结构转换必须留在本目录。
- `ProviderGatewayMeta.apiFormat` 表示上游真实目标协议，不表示入站 CLI 协议。入站协议由 Gateway route 推导，二者组成 `ConversionRoute`。
- `apiFormat` 字符串别名的唯一解析入口是 `AiProtocol::from_api_format`。Runtime provider 读取、前端/后端是否需要 Gateway 接管的判断，以及后续新增协议都必须复用它；不要在 `provider_protocol` 或 runtime 文件里复制第二套 parser。别名要同时覆盖 snake_case、slash 和 dash 形式，例如 `anthropic/messages`、`openai/responses`、`openai-chat`。
- `source == target` 时 Gateway 必须直通，不调用结构转换；直通路径仍可做已有模型名改写、`[1M]` 标记剥离等 runtime 级处理，但不能重写协议结构。
- 本模块不能依赖数据库、Tauri app handle、provider 表、Gateway runtime context、请求日志或模型健康状态。
- SSE 转换必须边读边写，不允许为了格式转换、日志或统计先 full-buffer 整个上游流。
- `json.rs` / `streaming.rs` 是旧实现遗留文件；新开发和测试以 `kernel.rs`、`stream.rs`、各协议 transformer、`fixtures/reference/` 和 `fixtures/live_provider/` 为准，不要把旧实现重新作为 fallback。

## 支持矩阵

当前 JSON request/response/error 和 SSE stream 支持：

| source | target | 状态 |
|---|---|---|
| `AnthropicMessages` | `OpenAiChat` | 支持 |
| `OpenAiChat` | `AnthropicMessages` | 支持 |
| `AnthropicMessages` | `OpenAiResponses` | 支持 |
| `OpenAiResponses` | `AnthropicMessages` | 支持 |
| `OpenAiChat` | `OpenAiResponses` | 支持 |
| `OpenAiResponses` | `OpenAiChat` | 支持 |
| `AnthropicMessages` | `GeminiNative` | 支持 |
| `GeminiNative` | `AnthropicMessages` | 支持 |
| `GeminiNative` | `OpenAiChat` / `OpenAiResponses` | 支持 |
| `OpenAiChat` / `OpenAiResponses` | `GeminiNative` | 支持 |

任何新增协议或新增聊天载荷能力都必须补全 request/response/SSE/error 的矩阵测试；不能只支持单向或只支持 JSON。

## 参考实现对照结论

- 参考实现使用统一 `llm.Request` / `llm.Response` 中间模型，覆盖 chat、responses、compact、embedding、image、video、rerank 等更大范围；本模块只处理 Gateway CLI 代理需要的聊天协议转换。
- 参考实现有 provider signature marker/footprint 机制，用于跨渠道同会话切换时保留 Anthropic thinking signature、Gemini thoughtSignature、OpenAI Responses encrypted_content。AI Toolbox 当前协议转换模块没有会话级统一模型和 footprint，不能伪造 marker，也不能把某个 provider 的私有签名错误转发给另一个 provider。
- 当前实现只映射可公开互通的 reasoning 文本：OpenAI `reasoning_content` / Responses reasoning summary / Anthropic `thinking`。`signature_delta`、Gemini thought signature、OpenAI `encrypted_content` 暂不做跨 provider marker 生命周期；未来若要实现，必须先引入明确的作用域/footprint 设计和测试。
- 参考实现的 stream transformer 对 tool call、reasoning、finish reason、usage、error event 都有状态机；本模块已对当前支持协议补齐对应的轻量状态机，但保持无 DB、无会话存储、无全局影子状态。
- 参考 fixture 已复制到 `fixtures/reference/{anthropic,openai_chat,openai_responses,gemini}/`。自动化测试必须对复制进来的 118 个 fixture 全部分类；当前聊天内核 supported 子集为 35 个 request、34 个 response、43 个 stream fixture，并全部转到所有非 identity target。`*.aggregator.json`、`*.stream.response.json`、`gemini/gemini-thought.jsonl` 属辅助语料；Responses compact、image generation、embedding、video、encrypted-only signature 等非聊天/签名生命周期能力作为明确 out-of-scope 或后续长期项保留。
- 全量参考 fixture 矩阵主要防止 panic、解析漂移和 shape 回退；关键协议语义还必须补精确断言。目前已锁定 system/instructions/systemInstruction、base64 image、stop sequences、tool_choice、tool schema strict、多 Anthropic `tool_result` 与同消息后续文本、tool_result cache/is_error、tool_result -> Gemini functionResponse name/id、OpenAI reasoning/reasoning_content、Responses custom tool JSON/SSE、Gemini request-level `thinkingConfig`、Gemini native Google tools、Gemini schema type 归一化、Gemini thoughts usage、Gemini thought text、Responses function_call arguments.done 完整参数、finish 幂等、Chat finish `delta:{}` 和 Gemini stream 不输出 `[DONE]`。
- 参考实现支持 Responses compact 与 custom tool；本模块不扩展 compact 协议。Responses custom tool 在聊天 request/response/stream 内通过 Chat 兼容扩展 `responses_custom_tool` 与 `response_custom_tool_call` 保留 call_id/name/input/output；转换到没有 custom tool 原生形态的目标协议时只能 best-effort 表达为普通 tool call。

## JSON 请求转换细节

- Anthropic `system` 转 OpenAI Chat `system` message，转 Responses `instructions`，转 Gemini `systemInstruction.parts[].text`。
- OpenAI Chat `system` 和 `developer` 都汇总到 Anthropic `system` 或 Responses `instructions`，顺序保留，用空行连接。
- Anthropic `messages[].content` 支持 string 和 block array；OpenAI/Gemini 转入时统一输出 Anthropic block array。
- 文本映射：
  - Anthropic `text` <-> Chat text / Responses `input_text`、`output_text` / Gemini `parts[].text`。
- 图片/文档映射：
  - Anthropic base64 `image` 转 Chat `image_url` data URL、Responses `input_image`、Gemini `inlineData`。
  - Chat/Responses data URL image 转 Anthropic `image.source`。
  - Gemini `inlineData` 转统一 data URL 媒体内容；当前目标协议出站只保证图片类 data URL 的互通，不实现完整 document/audio/video 生命周期。
- 工具定义映射：
  - Anthropic `tools[].input_schema` <-> Chat `tools[].function.parameters` <-> Responses `tools[].parameters` <-> Gemini `functionDeclarations[].parameters`。
  - Anthropic `BatchTool` 在转 OpenAI/Gemini 时过滤。
  - Responses `custom` tool <-> Chat 兼容扩展 `responses_custom_tool`；不要把它因为带有空 `function` 占位就退化成普通 function tool。
  - Gemini native `googleSearch` / `codeExecution` / `urlContext` 保留为中间模型 Google tools；转回 Gemini 时继续输出对应 native tool object。
  - Gemini SDK 可能给 schema `type` 返回 `OBJECT` / `STRING` 等大写值；入站必须递归归一为小写，避免转 OpenAI/Anthropic 后 schema 不合法。
- 工具选择映射：
  - Anthropic `any` <-> OpenAI/Responses `required`。
  - Anthropic `{type:"tool", name}` <-> Chat `{type:"function", function:{name}}` <-> Responses `{type:"function", name}` <-> Gemini `allowedFunctionNames`。
- 工具调用与工具结果：
  - Anthropic `tool_use` <-> Chat `tool_calls` / legacy `function_call` <-> Responses `function_call` <-> Gemini `functionCall`。
  - Anthropic `tool_result` <-> Chat `role:"tool"` <-> Responses `function_call_output` <-> Gemini `functionResponse`。
  - Anthropic 单条 user message 内允许多个 `tool_result` 和后续普通 text/image；入站不能在第一个 tool_result 处提前返回，出站应把连续 tool results 合并回同一个 Anthropic user content，保留 `cache_control` / `is_error`。
  - Responses `custom_tool_call` / `custom_tool_call_output` 必须和 Chat 兼容扩展 `responses_custom_tool` 双向保真；同一 request 内用前序 custom call id 判断后续 tool output 类型，不做跨请求影子状态。
  - Gemini 缺少 functionCall id 时生成 `gemini_synth_<index>`；转回 Gemini 时不会把这个 synthetic id 作为真实 id 发上游。
  - Gemini `functionResponse.name` 和缺失的 id 通过同一请求里的历史 functionCall 做 best-effort 补全；没有历史时用 id/name fallback。不做跨请求影子状态。
- Reasoning 映射：
  - Chat `reasoning` / `reasoning_content`、Responses `reasoning.summary[].text`、Anthropic `thinking`、Gemini `thought: true` 文本互转。
  - Anthropic 顶层 `thinking` / `output_config.effort` 转 OpenAI Chat `reasoning_effort` 或 Responses `reasoning.effort`；反向转 Anthropic 时用 `reasoning_effort` 生成 `thinking` 配置。
  - Gemini `generationConfig.thinkingConfig` 转 `reasoning_effort`；反向转 Gemini 时用 `reasoning_effort` 生成 `thinkingConfig`，并保持 `includeThoughts` 与 `thinkingLevel`/budget 语义。
  - Anthropic `redacted_thinking`、thinking `signature`、Responses `encrypted_content`、Gemini `thoughtSignature` 暂不做 provider marker 生命周期。
- 参数映射：
  - Anthropic `max_tokens` -> Chat `max_tokens` 或 o/GPT-5 系列 `max_completion_tokens`，-> Responses `max_output_tokens`，-> Gemini `generationConfig.maxOutputTokens`。
  - Chat `max_completion_tokens` / `max_tokens` -> Anthropic `max_tokens` / Responses `max_output_tokens`。
  - Responses `max_output_tokens` -> Anthropic `max_tokens` / Chat `max_tokens`。
  - `temperature`、`top_p`、`stream` 按目标协议保留；stop 在 Anthropic 使用 `stop_sequences`，OpenAI/Responses 使用 `stop`，Gemini 使用 `stopSequences`。
  - OpenAI Chat `response_format` 与 Responses `text.format`、Gemini `generationConfig.responseMimeType/responseSchema` 互转；Anthropic 目标没有等价 JSON schema 字段，不伪造约束。
  - OpenAI Chat / Responses 支持的请求级 pass-through 字段必须显式接线，至少包括 `parallel_tool_calls`、`prompt_cache_key`、`metadata`、`service_tier`、`frequency_penalty`、`presence_penalty`、`top_logprobs`、`user`、`verbosity`、`logprobs`、`logit_bias`、`seed`。
  - `extra_body` 只作为显式字段读写；不要把未知顶层字段自动合并到目标协议 body，避免把 source-only 参数误发给不支持的 provider。
- OpenAI stream request 转 Chat target 时必须补 `stream_options.include_usage=true`，避免流式 usage 丢失。

## JSON 响应转换细节

- Anthropic response 转 Chat：
  - `text` 合并为 assistant `message.content`。
  - `thinking` 写入 `reasoning_content`。
  - `tool_use` 写入 `tool_calls`。
  - `stop_reason` 映射为 `finish_reason`：`end_turn/stop_sequence -> stop`，`max_tokens -> length`，`tool_use -> tool_calls`。
- Chat response 转 Anthropic：
  - `reasoning_content` 转 `thinking`。
  - `content` 转 `text`。
  - `tool_calls` / `function_call` 转 `tool_use`。
  - 有 tool call 时 `finish_reason` / missing finish 可推导为 `tool_use`。
- Responses response 转 Anthropic/Chat：
  - `output[].message.content[].output_text` 转文本。
  - `refusal` 作为文本块保留，stop reason 不强行改写，除非 Responses status/finish 信息明确。
  - `annotations` / URL citations 作为 OpenAI message metadata 保留到支持该字段的 Chat target。
  - `function_call` / `custom_tool_call` 转 Anthropic `tool_use` 或 Chat `tool_calls`。
  - `reasoning.summary[].text` 转 Anthropic `thinking` 或 Chat `reasoning_content`。
  - `status=completed` 且有 tool call 时映射为 Anthropic `tool_use` / Chat `tool_calls`；`status=incomplete` 映射为 Anthropic `max_tokens` / Chat `length`。
- Gemini response 转 Anthropic：
  - `promptFeedback.blockReason` 生成 refusal 文本并设置 `stop_reason=refusal`。
  - `candidates[0].content.parts[].text` 转 Anthropic text。
  - `functionCall` 转 Anthropic `tool_use`。
  - `finishReason`：`MAX_TOKENS -> max_tokens`，`SAFETY/RECITATION/SPII/BLOCKLIST/PROHIBITED_CONTENT -> refusal`，有 tool call 时 `tool_use`，其他默认 `end_turn`。
- Anthropic response 转 Gemini：
  - text/tool_use 映射到 Gemini `parts[].text` / `functionCall`。
  - usage 映射到 `usageMetadata`，finish 映射到 Gemini `STOP` / `MAX_TOKENS` / `SAFETY`。
- Usage 映射：
  - OpenAI prompt/input tokens 转 Anthropic `input_tokens`；cached tokens 转 `cache_read_input_tokens`。
  - Anthropic `input_tokens + cache_read_input_tokens + cache_creation_input_tokens` 转 Gemini/OpenAI prompt。
  - Responses `input_tokens_details.cached_tokens` 会从 Anthropic `input_tokens` 中扣出，避免缓存 token 被重复计入非缓存输入。
  - Gemini `promptTokenCount` 扣除 `cachedContentTokenCount` 后写 Anthropic `input_tokens`；`thoughtsTokenCount` 计入统一 `completion_tokens` 和 `reasoning_tokens`，转回 Gemini 时 `candidatesTokenCount = completion_tokens - reasoning_tokens`；`totalTokenCount` 优先按上游值保留。

## SSE 转换细节

- SSE parser 支持 `\n\n` 和 `\r\n\r\n`，支持 UTF-8 chunk 边界跨包，忽略空 data 和无效 JSON data。
- OpenAI Chat -> Anthropic：
  - `delta.content` -> `content_block_start(text)` + `text_delta`。
  - `delta.reasoning_content` / `delta.reasoning` -> `content_block_start(thinking)` + `thinking_delta`。
  - `delta.tool_calls[].function.arguments` -> `content_block_start(tool_use)` + `input_json_delta`。
  - `[DONE]`、finish chunk 重复出现时只输出一组 `message_delta` + `message_stop`。
- Anthropic -> Chat：
  - `message_start` -> Chat role delta。
  - `text_delta` -> `delta.content`。
  - `thinking_delta` -> `delta.reasoning_content`。
  - `tool_use` start/delta -> Chat `tool_calls` name/id/arguments 增量。
  - `message_delta.stop_reason` -> Chat `finish_reason`。
  - `message_stop` 走统一 finish，避免重复 `[DONE]`。
- OpenAI Chat -> Responses：
  - `delta.content` -> `response.output_text.delta`。
  - `delta.reasoning_content` -> `response.reasoning_summary_text.delta`。
  - `delta.tool_calls` -> `response.output_item.added(function_call)` + `response.function_call_arguments.delta`。
  - Chat 兼容扩展 `responses_custom_tool` -> `response.output_item.added(custom_tool_call)` + `response.custom_tool_call_input.delta/done`。
  - finish -> `response.completed`。
- Responses -> Chat：
  - `response.created` -> Chat role delta。
  - `response.output_text.delta` -> Chat content delta。
  - `response.reasoning_summary_text.delta` -> Chat `reasoning_content` delta。
  - `response.output_item.added(function_call)` + `response.function_call_arguments.delta` -> Chat `tool_calls` delta。
  - `response.output_item.added(custom_tool_call)` + `response.custom_tool_call_input.delta/done` -> Chat 兼容扩展 `responses_custom_tool` delta。
  - `response.completed` -> Chat finish + `[DONE]`，有 tool call 时 finish reason 为 `tool_calls`。
- Anthropic -> Responses：
  - `text_delta` -> `response.output_text.delta`。
  - `thinking_delta` -> `response.reasoning_summary_text.delta`。
  - `tool_use` start/delta/stop -> Responses function_call item added / arguments delta / arguments done / output item done。
  - `message_stop` -> `response.completed`。
- Responses -> Anthropic：
  - `response.output_text.delta` -> Anthropic text block。
  - `response.reasoning_summary_text.delta` -> Anthropic thinking block。
  - function_call item/delta -> Anthropic tool_use block + input_json_delta。
  - `response.completed` 有 tool call 时 stop reason 为 `tool_use`，否则 `end_turn`。
- Gemini -> Anthropic：
  - Gemini stream chunks 可能发送累计文本，本模块按前缀差值输出 Anthropic `text_delta`。
  - `functionCall` 在 finish 时输出 Anthropic tool_use block；缺 id 时使用 synthetic id。
  - blocked prompt 在 finish 时输出 refusal 文本。
- Anthropic -> Gemini：
  - `text_delta` 直接输出 Gemini SSE chunk。
  - `tool_use` start/delta/stop 累计 JSON 参数后输出 Gemini `functionCall`。
  - `message_delta.usage` 转 Gemini `usageMetadata`；`message_stop` 输出 finish chunk。

## Error 转换细节

- `convert_error_response_body` 只在 body 是 JSON 且能提取 message 时转换；非 JSON 或无法识别 error shape 时原样返回。
- OpenAI/Responses target 使用 `{error:{message,type,param,code}}`。
- Anthropic target 使用 `{type:"error", error:{type,message}}`。
- Gemini target 使用 `{error:{code,message,status}}`，并按常见 error type 映射 HTTP-like code/status。

## 非目标范围

- 不处理 embedding、image generation、video、rerank、OpenAI Responses compact。
- 不做跨请求工具名影子存储。Gemini functionResponse 的 name 只从当前请求已有 tool_use/tool_result 关系 best-effort 推导。
- 不实现参考实现的 signature marker/footprint 生命周期。未来实现前必须补设计文档和测试，明确 marker 生成、识别、转发、丢弃和跨 provider mismatch 行为。
- 不在本模块处理上游 URL、query、header、auth、model mapping、`[1M]` URL 段剥离、request logging、usage cost、provider failover。

## 测试覆盖矩阵

- `cargo test protocol_conversion::kernel`
  - `request_conversion_covers_all_non_identity_protocol_routes`：4 个协议两两非 identity 的 12 条 request route。
  - `response_conversion_covers_all_non_identity_protocol_routes`：4 个协议两两非 identity 的 12 条 response route。
  - `sse_conversion_covers_all_non_identity_protocol_routes`：4 个协议两两非 identity 的 12 条 SSE route，覆盖 text、reasoning、tool_call、finish、UTF-8/SSE 分块。
  - `reference_request_fixtures_convert_to_all_targets`：参考 simple/tool/thinking request fixture 转所有非 identity target。
  - `reference_response_fixtures_convert_to_all_targets`：参考 stop/tool/thinking response fixture 转所有非 identity target。
  - `reference_stream_fixtures_convert_to_all_targets`：参考 `{Type,Data}` JSONL stream fixture 转标准 SSE 后再转所有非 identity target。
  - `reference_all_copied_fixtures_are_classified`：复制进来的 118 个参考 fixture 必须全部进入 supported/auxiliary/out-of-scope 分类，防止新增语料被静默忽略。
  - `reference_all_supported_request_fixtures_convert_to_all_targets`：35 个 supported request fixture 全部转所有非 identity target。
  - `reference_all_supported_response_fixtures_convert_to_all_targets`：34 个 supported response fixture 全部转所有非 identity target。
  - `reference_all_supported_stream_fixtures_convert_to_all_targets`：43 个 supported stream fixture 全部转标准 SSE 后再转所有非 identity target。
  - `reference_*_semantics_*` 与精确回归测试：从参考实现测试翻译来的关键断言，覆盖 stop/tool_choice、图片、工具结果、reasoning、Anthropic 混合 tool_result、Responses custom tool、Gemini thinkingConfig/native tools/schema/usage、stream tool argument accumulation 与 finish 幂等。
  - `live_provider_response_fixtures_convert_to_all_targets`：真实 provider HTTP 200 响应样本转所有非 identity target，覆盖 OpenAI Chat、OpenAI Responses、Anthropic Messages、Gemini Native 的真实响应 shape、reasoning、finish/status 和 usage 边界。
- `cargo test protocol_conversion` 是本模块当前推荐的局部验证命令；它包含 `kernel` 测试和编译边界。

## 回归测试规则

- 以后任何协议转换问题，无论来自开发自测、review、真实 provider 验证还是用户反馈，都必须在同一任务内补一个最贴近失败模式的回归测试；没有测试不能宣称修复完成。
- 外部 provider 返回 shape 导致的问题优先沉淀为 `fixtures/live_provider/` 或更小的脱敏 fixture；转换器逻辑问题优先补精确单元断言；SSE 状态机问题必须补 stream fixture 或逐事件断言。
- 真实 provider fixture 不得包含 API key、Authorization header、query key 或用户敏感输入；动态 id/timestamp 可以稳定化，但必须保留能触发问题的协议结构、finish/status、usage 和 content/reasoning 字段。
- 修改后至少跑 `cd tauri && cargo test protocol_conversion --no-default-features`；大范围协议转换改动还必须按根规则补跑全量测试集合。

## 最小验证

- 修改 JSON 转换、SSE parser、stream state、统一模型或 transformer 后至少跑 `cd tauri && cargo test protocol_conversion --no-default-features`。
- 修改 route/path/header/auth 编排后额外跑 `cd tauri && cargo test proxy_gateway::runtime::upstream` 和 `cd tauri && cargo test proxy_gateway::runtime::providers`。
- 大范围协议转换改动交付前按根规则跑 `cd tauri && cargo test`；若同时改前端 provider 表单/i18n，再跑 `pnpm test`、`pnpm exec tsc --noEmit` 和 i18n check。

## Gotchas

- 新增协议时先扩展 `AiProtocol` 和 `ConversionRoute`，再同时补 JSON、SSE、error、runtime target path/header/auth、provider `apiFormat` 解析和测试。
- 不要在 `runtime/upstream.rs` 里临时写协议字段转换 helper；只允许 runtime 计算 route、调用本模块、保存转换后上游 body。
- 流式转换中完成事件必须幂等。OpenAI `[DONE]`、Responses `response.completed`、Anthropic `message_stop` 和 finish chunk 可能组合出现，只能输出一组目标协议完成事件。
- 对目标 Anthropic 的流式 tool_use 必须保证 `content_block_start`、若干 `input_json_delta`、`content_block_stop` 顺序完整。
- 对目标 Chat 的流式 finish chunk 必须包含 `delta:{}`，兼容 OpenAI 客户端 streaming parser。
- 对目标 Gemini 的 stream 不输出 OpenAI `[DONE]`；Gemini 结束由最后一个带 `finishReason` 的 chunk 表达。
- 无效 JSON SSE event 直接忽略，不得 panic；source 结束时仍按当前状态尝试 finish。
