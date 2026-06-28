# Protocol Conversion Module Notes

## 一句话职责

- 在 Proxy Gateway 请求路径中提供独立、可复用的 AI 协议载荷转换：Anthropic Messages、OpenAI Chat Completions、OpenAI Responses、Gemini Native 的 JSON 与 SSE 聊天协议互转。

## Source of Truth

- 转换模块的 Source of Truth 是 `AiProtocol`、`ConversionRoute`、`convert_request_body`、`convert_response_body`、`convert_error_response_body` 和 `convert_sse_stream` 的行为与测试。
- Runtime 只负责判断入站 route、读取 provider 的 target protocol、拼上游 path/header/auth、保存 `request_body` 与 `upstream_request_body` 快照；协议结构转换必须留在本目录。
- `ProviderGatewayMeta.apiFormat` 表示上游真实目标协议，不表示入站 CLI 协议。入站协议由 Gateway route 推导，二者组成 `ConversionRoute`。
- `source == target` 时 Gateway 必须直通，不调用结构转换；直通路径仍可做已有模型名改写、`[1M]` 标记剥离等 runtime 级处理，但不能重写协议结构。
- 本模块不能依赖数据库、Tauri app handle、provider 表、Gateway runtime context、请求日志或模型健康状态。
- SSE 转换必须边读边写，不允许为了格式转换、日志或统计先 full-buffer 整个上游流。

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
| `GeminiNative` | `OpenAiChat` / `OpenAiResponses` | 明确 unsupported |
| `OpenAiChat` / `OpenAiResponses` | `GeminiNative` | 明确 unsupported |

不支持的 Gemini/OpenAI 直转路线必须返回 `ProtocolConversionError::UnsupportedRoute`，不能退化成错误协议的直通。

## AxonHub 对照结论

- AxonHub 使用统一 `llm.Request` / `llm.Response` 中间模型，覆盖 chat、responses、compact、embedding、image、video、rerank 等更大范围；本模块只处理 Gateway CLI 代理需要的聊天协议转换。
- AxonHub 有 provider signature marker/footprint 机制，用于跨渠道同会话切换时保留 Anthropic thinking signature、Gemini thoughtSignature、OpenAI Responses encrypted_content。AI Toolbox 当前协议转换模块没有会话级统一模型和 footprint，不能伪造 marker，也不能把某个 provider 的私有签名错误转发给另一个 provider。
- 当前实现只映射可公开互通的 reasoning 文本：OpenAI `reasoning_content` / Responses reasoning summary / Anthropic `thinking`。`signature_delta`、Gemini thought signature、OpenAI `encrypted_content` 暂不做跨 provider marker 生命周期；未来若要实现，必须先引入明确的作用域/footprint 设计和测试。
- AxonHub 的 stream transformer 对 tool call、reasoning、finish reason、usage、error event 都有状态机；本模块已对当前支持协议补齐对应的轻量状态机，但保持无 DB、无会话存储、无全局影子状态。
- AxonHub 支持 Responses compact 与 custom tool；本模块当前只保留普通 function tool 的互通语义，custom tool 在 Responses 流中按 function-call 兼容路径处理，不扩展 compact 协议。

## JSON 请求转换细节

- Anthropic `system` 转 OpenAI Chat `system` message，转 Responses `instructions`，转 Gemini `systemInstruction.parts[].text`。
- OpenAI Chat `system` 和 `developer` 都汇总到 Anthropic `system` 或 Responses `instructions`，顺序保留，用空行连接。
- Anthropic `messages[].content` 支持 string 和 block array；OpenAI/Gemini 转入时统一输出 Anthropic block array。
- 文本映射：
  - Anthropic `text` <-> Chat text / Responses `input_text`、`output_text` / Gemini `parts[].text`。
- 图片/文档映射：
  - Anthropic base64 `image` 转 Chat `image_url` data URL、Responses `input_image`、Gemini `inlineData`。
  - Chat/Responses data URL image 转 Anthropic `image.source`。
  - Gemini `inlineData` 按 MIME type 转 Anthropic `image` 或 `document`。
- 工具定义映射：
  - Anthropic `tools[].input_schema` <-> Chat `tools[].function.parameters` <-> Responses `tools[].parameters` <-> Gemini `functionDeclarations[].parameters`。
  - Anthropic `BatchTool` 在转 OpenAI/Gemini 时过滤。
- 工具选择映射：
  - Anthropic `any` <-> OpenAI/Responses `required`。
  - Anthropic `{type:"tool", name}` <-> Chat `{type:"function", function:{name}}` <-> Responses `{type:"function", name}` <-> Gemini `allowedFunctionNames`。
- 工具调用与工具结果：
  - Anthropic `tool_use` <-> Chat `tool_calls` / legacy `function_call` <-> Responses `function_call` <-> Gemini `functionCall`。
  - Anthropic `tool_result` <-> Chat `role:"tool"` <-> Responses `function_call_output` <-> Gemini `functionResponse`。
  - Gemini 缺少 functionCall id 时生成 `gemini_synth_<index>`；转回 Gemini 时不会把这个 synthetic id 作为真实 id 发上游。
  - Gemini `functionResponse.name` 通过同一请求里的历史 `tool_use_id -> name` 做 best-effort 补全；没有历史时用 id/name fallback。不做跨请求影子状态。
- Reasoning 映射：
  - Chat `reasoning_content`、Responses `reasoning.summary[].text`、Anthropic `thinking` 互转。
  - Anthropic `redacted_thinking`、thinking `signature`、Responses `encrypted_content`、Gemini `thoughtSignature` 暂不做 provider marker 生命周期。
- 参数映射：
  - Anthropic `max_tokens` -> Chat `max_tokens` 或 o/GPT-5 系列 `max_completion_tokens`，-> Responses `max_output_tokens`，-> Gemini `generationConfig.maxOutputTokens`。
  - Chat `max_completion_tokens` / `max_tokens` -> Anthropic `max_tokens` / Responses `max_output_tokens`。
  - Responses `max_output_tokens` -> Anthropic `max_tokens` / Chat `max_tokens`。
  - `temperature`、`top_p`、`stream` 按目标协议保留；stop 在 Anthropic 使用 `stop_sequences`，OpenAI/Responses 使用 `stop`，Gemini 使用 `stopSequences`。
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
  - Gemini `promptTokenCount` 扣除 `cachedContentTokenCount` 后写 Anthropic `input_tokens`；`candidatesTokenCount` 缺失时可用 `totalTokenCount - promptTokenCount` best-effort 推导 output。

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
  - finish -> `response.completed`。
- Responses -> Chat：
  - `response.created` -> Chat role delta。
  - `response.output_text.delta` -> Chat content delta。
  - `response.reasoning_summary_text.delta` -> Chat `reasoning_content` delta。
  - `response.output_item.added(function_call)` + `response.function_call_arguments.delta` -> Chat `tool_calls` delta。
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
- 不实现 AxonHub 的 signature marker/footprint 生命周期。未来实现前必须补设计文档和测试，明确 marker 生成、识别、转发、丢弃和跨 provider mismatch 行为。
- 不在本模块处理上游 URL、query、header、auth、model mapping、`[1M]` URL 段剥离、request logging、usage cost、provider failover。

## 测试覆盖矩阵

- `cargo test protocol_conversion::json`
  - Anthropic -> Chat request：system/tools/stream usage。
  - Chat -> Anthropic request：developer/system/media/tool_calls/tool_result/max/stop/tool_choice。
  - Anthropic -> Responses request：image/thinking/tool_use/tool_result/tool_choice。
  - Responses -> Anthropic request：function_call_output/max。
  - Chat -> Responses response：tool_calls/usage。
  - Anthropic -> Chat response：thinking/tool_use/usage/cache/finish。
  - Responses -> Anthropic response：reasoning/refusal/tool/usage/cache/finish。
  - Responses -> Chat response：reasoning/tool/usage/cache/incomplete finish。
  - Gemini <-> Anthropic request/response：system/media/tools/tool_result/generation/usage/blocked prompt。
  - Error conversion：OpenAI/Anthropic/Gemini JSON error 和非 JSON 原样返回。
  - Unsupported route：Gemini -> OpenAI 明确失败。
- `cargo test protocol_conversion::streaming`
  - Finish 幂等：Chat -> Anthropic、Responses -> Chat。
  - Chat -> Anthropic：text/reasoning/tool_call/finish。
  - Chat -> Responses：text/reasoning/tool_call/finish。
  - Anthropic -> Chat：thinking_delta/tool_use/input_json_delta/finish/DONE。
  - Anthropic -> Responses：thinking_delta/tool_use/input_json_delta/done/completed。
  - Responses -> Anthropic：reasoning/function_call delta/tool stop。
  - Responses -> Chat：reasoning/function_call delta/tool finish。
  - Gemini -> Anthropic：累计文本差值、functionCall、blocked/invalid JSON finish。
  - Anthropic -> Gemini：text/tool_use/usage/finish。

## 最小验证

- 修改 JSON 转换后至少跑 `cd tauri && cargo test protocol_conversion::json`。
- 修改 SSE parser 或 stream state 后至少跑 `cd tauri && cargo test protocol_conversion::streaming`。
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
