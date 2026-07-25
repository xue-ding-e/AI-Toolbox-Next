# Transformer 协议转换实现审计报告（修复复核版）

> 本文是 `ai-toolbox` Gateway transformer/runtime 协议转换审计的当前状态报告。
>
> - 初始审计基线：`e0c26ba`
> - 修复前复核基线：`3c138e6`（当前 `HEAD`）
> - 当前修复状态：工作树中的未提交修改；本轮没有可引用的修复 commit
> - 审计与验证日期：2026-07-25

## 0. 结论摘要

本轮从初次审查、当前实现复核和参考项目对照中共确认 F-1 至 F-13，现已全部修复并补齐回归测试。后续收口又强化了 F-5 的 raw-only 空 signature sidecar / 不可签名 structured tool 门控，以及 F-9 的稀疏 response snapshot 合并与 terminal `response:null` / 空对象回退，避免文档声称的 fail-closed 语义与实际实现存在最后一层偏差；最后补齐了非流 cancellation 双向映射和转换前后双 body 的 runtime 分类。

| 编号 | 问题 | 结果 |
|------|------|------|
| F-1 | OpenAI Responses 合法 `incomplete` / `cancelled` 终态被当成 provider failure | 已修复，已补非流和流式回归 |
| F-2 | reverse SSE middleware 在 no-op 时重序列化，丢失元数据和原始字节；JSON 可能误进 SSE wrapper | 已修复，已补字节透明、Content-Type 和 Ollama NDJSON 回归 |
| F-3 | MR-T 首次 tool-call identity 只等待 name，过早发 synthetic ID | 已修复，已补 Chat -> Responses/Anthropic 回归 |
| F-4 | MR-B pending reasoning 可能丢失或跨 user turn 归属错误 | 已修复，已补连续 reasoning、孤立 reasoning 和跨轮回归 |
| F-5 | T-3 只做 raw tool signature 碰撞过滤，没有校验 structured tool 完整集合 | 已修复，空集合也保留 sidecar，重排/增删/不可签名均 fail-closed |
| F-6 | 审计报告与修复计划仍描述旧代码状态 | 已刷新，当前文档明确基线和未提交状态 |
| F-7 | `PipelineContext.lossy_warnings` 是迁移后的死字段 | 已删除，warning 只保留在 `PreparedUpstreamBody` |
| F-8 | 混合 LF/CRLF SSE 缓冲区未选择最早分隔符 | 已在复核中补修，transformer/runtime 各有回归 |
| F-9 | forced-stream Responses 聚合遗漏取消终态，且无 terminal event 时伪造 `completed` | 已修复，合法终态保留；缺 terminal fail-closed；稀疏 snapshot 按字段合并 |
| F-10 | reverse SSE 空白 block/EOF 尾部被丢弃，改写 payload 时统一行尾 | 已修复，空白字节透传并逐行保留 LF/CRLF |
| F-11 | source stream error 后 EOF 又补正常 finish | 已修复，错误终态阻止后续事件和 EOF finish |
| F-12 | 非流 Responses cancellation 与 LLM `finish_reason` 映射不对称，可能被写成 `completed` | 已修复，`cancelled` / `canceled` 双向映射并补非流 roundtrip 回归 |
| F-13 | 协议转换后原始 `response.failed` 被隐藏，runtime 只检查最终 body | 已修复，成功响应分类和空响应检测同时检查转换后 body 与 `upstream_response_body` |

当前验证通过：

- `cd tauri && cargo test transformer --no-default-features`：206 passed
- `cd tauri && cargo test --lib coding::proxy_gateway::runtime::upstream`：183 passed
- `cd tauri && cargo test`：主库 1179 passed、1 ignored；`coding` 78、SQLite JSONB 21、migration 12、Surreal import 4、doc-tests 10 全部通过
- `pnpm test`：267 passed
- `pnpm exec tsc --noEmit`：通过

## 1. 审计范围与基线

### 1.1 代码范围

- `tauri/src/coding/proxy_gateway/runtime/upstream.rs`
- `tauri/src/coding/proxy_gateway/runtime/middleware.rs`
- `tauri/src/coding/proxy_gateway/transformer/stream.rs`
- `tauri/src/coding/proxy_gateway/transformer/sse.rs`
- `tauri/src/coding/proxy_gateway/transformer/openai/responses/shared.rs`
- 对应的 transformer/runtime 回归测试
- `tauri/src/coding/proxy_gateway/AGENTS.md`
- `tauri/src/coding/proxy_gateway/transformer/AGENTS.md`

### 1.2 代码基线语义

`e0c26ba` 是原始审计报告引用的功能基线，`3c138e6` 是本轮开始前仓库的当前 `HEAD`。修复代码没有提交，因此任何文档都不能把本轮修改伪造为新的 commit。

Transformer 的结构仍然是：

1. 入站协议通过 `AiProtocol` / `ConversionRoute` 解析。
2. 不同协议先进入统一 LLM request/response 模型。
3. JSON 和 SSE 分别通过 transformer/kernel 转换。
4. Gateway runtime 负责 provider URL、header、鉴权、provider-specific adapter、failover 和日志。
5. request-scoped `ConversionContext` 随同一次请求的响应转换存活，不进入数据库或跨请求 store。

本轮没有把 provider 专属兼容逻辑下沉到 transformer，也没有把 runtime side store 引入 transformer。

## 2. 修复明细

### 2.1 F-1：合法 Responses 终态不应触发 provider failure

#### 原问题

OpenAI Responses 的 `status: "incomplete"`、`status: "cancelled"` / `"canceled"` 是请求已经得到协议级终态的合法结果。它们可能带部分 output，也可能 output 为空。若把它们统一归类为错误，会产生错误的同 provider retry、跨 provider failover 和模型健康扣分。

#### 当前实现

`runtime/upstream.rs` 的协议错误判断现在采用以下边界：

- 顶层非空 `error`：错误。
- `type` / `event` 为 `error` 或 `response.failed`：错误。
- `status` 或嵌套 `response.status` 为 `failed`：错误。
- `response.incomplete`、`response.cancelled`、`response.canceled` 及对应 status：不是 provider failure。

非流式响应的 meaningful-content 判断显式接受这些合法终态，即使 `output` 为空；流式首包 probe 也会把这些终态视为已经收到协议意义上的响应，不会把它们判成空响应失败。非流 Responses 转换还采用明确的双向映射：`status: "incomplete"` -> LLM `finish_reason: "length"`，`status: "cancelled"` / `"canceled"` -> `finish_reason: "cancelled"`；反向转换把 `finish_reason: "cancelled"` / `"canceled"` 规范为 Responses `status: "canceled"`，不会写成 `completed`。对于非流客户端需要聚合的 Responses SSE，只有实际收到 `response.completed`、`response.failed`、`response.incomplete`、`response.cancelled` 或 `response.canceled` 才能生成 JSON；如果流在这些事件之前结束，会返回连接类错误进入既有 retry/failover，而不会把“截断”伪装成合法 `incomplete`。

这里仍然保留一个必要的优先级：如果响应同时存在非空 `error`，顶层错误仍然优先按失败处理；合法的 `error: null` 不会触发失败。

#### 回归测试

- `incomplete_and_cancelled_terminal_responses_are_not_provider_failures`
- `streaming_first_chunk_accepts_partial_output_with_incomplete_terminal_event`
- transformer 侧已有 `responses_cancelled_stream_passthrough_does_not_synthesize_completed`

### 2.2 F-2：response stream 判定与 reverse SSE 透明性

#### 原问题

旧路径只要请求体带 `stream: true` 或 route 声明 streaming，就可能把明确的 JSON 响应送进 SSE wrapper。反向 middleware 即使没有改变事件，也会重新序列化 JSON，并重建 SSE block，导致以下信息丢失或变化：

- `event`、`id`、`retry`
- comment 行
- 多行 `data`
- 原始 JSON 空白
- LF/CRLF 和原始 block 分隔符

#### 当前实现

`should_stream_response()` 现在按优先级判断：

1. 实际响应 `Content-Type` 包含 `text/event-stream`：进入 SSE 路径。
2. 有明确但非 SSE 的 `Content-Type`（例如 `application/json`）：不进入 SSE wrapper。
3. 没有 `Content-Type`：才使用请求 `stream:true` 或 Gemini route 声明作为兼容 fallback。

Ollama 是明确的 provider-specific 例外：

1. `application/x-ndjson` 只有在 provider 被识别为 Ollama 时才进入特殊流路径。
2. 先由 Ollama adapter 转为 OpenAI Chat SSE。
3. 后续再执行通用 SSE 转换和 reverse middleware。
4. 普通 provider 的 NDJSON 不会未经转换进入通用 SSE wrapper。

reverse SSE block 的当前行为：

- 没有 `data`、`[DONE]`、非 JSON data，原样透传。
- middleware 执行后 JSON value 没有变化，直接返回原始 block 字节。
- JSON 确实变化时，只替换 `data` payload，保留所有非 data 行。
- 原始 block 有 delimiter 时保留 `\n\n` 或 `\r\n\r\n`；没有 delimiter 的尾块只在需要输出时补 delimiter。
- 完整的空白 block 和 EOF 尾部空白也原样透传，不再用 `trim()` 静默丢弃。
- 确有 JSON 改写时，data 行和非 data 行各自保留原有行尾，不把混合 LF/CRLF 统一成单一格式。
- block parser 同时支持 LF/CRLF；混合时选择物理位置最早的 delimiter，避免跨事件合并。

这使 `BillingHeaderCchMiddleware` 可以在确实需要回填 `cch` 时改写事件，同时不破坏 SSE 元数据；无业务改写的普通事件则保持字节透明。

#### 回归测试

- `response_streaming_uses_actual_content_type_for_json_fallbacks`
- `rewrite_sse_block_zero_diff_without_billing_context`
- `rewrite_sse_block_restores_billing_cch_and_passthrough_done`
- `reverse_sse_parser_uses_the_earliest_mixed_line_ending_delimiter`
- `transformer::sse::tests::parser_uses_the_earliest_mixed_line_ending_delimiter`
- `ollama_ndjson_stream_converts_to_openai_chat_sse`

### 2.3 F-3：MR-T 等待真实 tool-call ID

#### 原问题

部分 OpenAI Chat-compatible provider 会先发送 tool name 和参数，稍后才发送真实 tool-call ID。旧状态机只要 name 到达就打开目标 tool item，随后只能使用 `call_<index>` synthetic ID；真实 ID 到达后无法修正已经发给客户端的 item。

#### 当前实现

未打开的 tool call 必须同时满足：

- `name` 非空；
- provider 真实 `id` 非空。

只有在两者都满足后才进入 ready 集合并首次 emit。已打开的 tool call 仍可继续接收 arguments delta。finish 阶段会对仍有 name 但始终没有真实 ID 的 legacy tool call 做最终化，并使用 `call_<index>` fallback。首次打开顺序仍按 tool index 升序。

#### 回归测试

- `chat_stream_tool_call_waits_for_real_id_before_first_emit`
- `chat_stream_anthropic_tool_start_waits_for_real_id_before_first_emit`
- `chat_stream_tool_calls_flush_in_index_order_despite_late_identity`

### 2.4 F-4：MR-B reasoning 只在当前 user turn 内归属

#### 原问题

Responses input 中的 reasoning item 可能连续出现、出现在 input 末尾，或位于 user boundary 前。旧逻辑可能覆盖连续 reasoning、把 pending reasoning 挂到更早的 assistant，或在没有 assistant 时静默丢弃。

#### 当前实现

`append_responses_input_to_messages()` 维护当前 user 段内的 `last_assistant_index`：

- 连续 reasoning item 先合并，再和后续 assistant/function/custom tool 做 forward merge。
- 遇到 user boundary 时清空 assistant 归属，防止跨轮回挂。
- 当前 user 段存在 assistant 时，trailing reasoning 追加到该 assistant；追加不是再次执行 forward merge。
- 当前段没有可归属 assistant 时，保留 standalone assistant reasoning，不静默丢弃。
- reasoning signature 和 transformer metadata 按已有合并语义保留。

#### 回归测试

- `consecutive_reasoning_items_are_preserved_when_followed_by_assistant`
- `reasoning_without_same_turn_assistant_is_kept_before_user_boundary`
- `trailing_reasoning_does_not_cross_user_boundary`
- `trailing_reasoning_at_input_end_attaches_to_previous_assistant`
- `trailing_reasoning_appends_after_embedded_forward_merge`
- `trailing_reasoning_appends_to_assistant_that_already_has_tool_calls`

### 2.5 F-5：T-3 完整校验 structured tool signature

#### 原问题

Responses raw tool fragment 使用原 index 合并回 structured tools。旧逻辑虽然已经采集了 `openai_responses_tool_signatures` sidecar，也会过滤同 signature 的 raw collision，但没有确认中间转换后的 structured tools 仍然是同一组工具。工具增删或重排时，raw fragment 可能插入错误位置。

#### 当前实现

只要原请求存在 raw tool fragment，`attach_responses_raw_request_metadata()` 就会保存 `openai_responses_tool_signatures`；原 structured tool 集合为空时保存 `[]`，从而明确区分“原集合确实为空”和“没有完整性证据”。

`merge_raw_responses_fragments_with_signatures()` 在合并前比较：

- structured tool 数量；
- structured tool 顺序；
- 每个 tool 的完整 signature（例如 `function:name`、`custom:name`）。

签名采集使用完整 `Option<Vec<_>>`，不会通过 `filter_map` 隐藏无法生成 signature 的 structured tool。请求级 `openai_responses_tool_signatures_complete` 必须为 `true`，且 signature sidecar 必须存在、是合法字符串数组并与当前 structured tools 完整匹配；sidecar 缺失、类型异常、marker 不是 `true`，或任一 `function` / `custom` tool 的 name 缺失或只有空白时，都完全放弃 raw fragment merge，只返回当前 structured tools。只有完整匹配时，才继续执行原有的 raw collision 过滤。

#### 回归测试

- `raw_tools_merge_skips_fragments_when_structured_signatures_are_reordered`
- `raw_tools_merge_drops_raw_tool_colliding_with_structured_signature`
- `raw_only_tools_preserve_an_empty_structured_signature_sidecar`
- `raw_tools_merge_skips_fragments_when_structured_tool_is_added_to_empty_signature_set`
- `raw_tools_merge_fails_closed_when_any_structured_tool_has_no_signature`

### 2.6 F-6：文档状态刷新

旧文档把 `e0c26ba` 之后已经落地的实现继续描述成未修复，并且把 reverse hook 描述成尚未存在。当前事实是：

- `Middleware::on_outbound_response` / `on_outbound_stream` 已存在；
- JSON response reverse 已接线；
- SSE reverse 已接线并边读边写；
- 本轮修复重点是 reverse SSE 的透明性和 stream 判定，不是新增一套 reverse hook；
- 当前修复代码仍在未提交工作树。

本报告和 `transformer-protocol-conversion-fix-plan.md` 已按此事实重写。

### 2.7 F-7：移除 `PipelineContext.lossy_warnings`

`lossy_warnings` 的真实所有权属于 runtime 的 `PreparedUpstreamBody`，用于最终响应的 `X-Transformer-Lossy` header。它不属于 transformer `ConversionContext`，也不属于 middleware 的 `PipelineContext`。本轮已从 `PipelineContext` 删除，避免形成没有生产消费者的迁移残留字段。

### 2.8 F-8：混合 SSE 换行边界

本轮复核发现，原有 delimiter 查找逻辑会先全局搜索 CRLF，再搜索 LF；如果一个缓冲区中较早出现 LF block、较晚出现 CRLF block，可能一次消费多个事件。runtime reverse parser 和 transformer 通用 parser 都已改为比较两种 delimiter 的位置并消费最早者，并各自补了测试。

### 2.9 F-9：forced-stream Responses 聚合必须区分合法终态和截断

参考 AxonHub `origin/unstable` 的 `responsesOutboundStream` 与 `AggregateStreamChunks` 语义后，runtime 聚合器补齐了 `response.cancelled` / `response.canceled`，并保留上游 terminal response 的 status、error、usage 和 output。

聚合器现在只在真正收到 Responses terminal event 后返回 JSON。若上游只发送 `response.created`、delta、`output_item.done` 或 `[DONE]` 就结束，返回 `GatewayFailureKind::Connection`，让已有的 retry/failover 处理截断；不能将这个情况写成 `status: "incomplete"`，因为 `incomplete` 是上游明确发送的合法协议终态，按规则不应触发 provider health mutation。

created、in-progress 和 terminal event 都可能只携带稀疏 response snapshot，聚合器现在按字段浅层合并，避免后续空对象或只含 usage/status 的对象覆盖已有 id、model、created_at。terminal event 中只有 object 型 `response` 才作为 snapshot；字段省略、显式 `null`、空对象或非对象时保留最近的 base snapshot，terminal `status` 只有非空字符串才可信，否则由 terminal event 类型覆盖。若流中已有 `response.output_item.done` 且 terminal snapshot 没有非空 output array（包括缺失、`null` 或空数组），则用已收集 item 重建 created snapshot 中的陈旧空 `output`。这样既不会返回 JSON `null`，也不会丢失元数据或明确完成的部分输出。

#### 回归测试

- `codex_official_sse_aggregate_preserves_cancelled_terminal_response`
- `codex_official_sse_aggregate_rejects_missing_terminal_event`
- `codex_official_sse_aggregate_merges_sparse_response_snapshots`

### 2.10 F-10：reverse SSE 空白透传与改写行尾保真

补充复核发现，wrapper 层在消费完整 block 和 EOF tail 前使用 `trim()`，会丢掉只有分隔符或只有空白的原始字节。现已对所有非空原始字节调用同一 rewrite helper；无 data/非 JSON/no-op 仍原样返回。

当 middleware 确实改变 JSON data 时，改写器现在逐行读取并保留每一行自己的 LF/CRLF，仍只替换 data payload，不改变 event、id、retry、comment 和其他 metadata。

#### 回归测试

- `reverse_sse_stream_preserves_blank_blocks_and_whitespace_tail`
- `rewrite_sse_block_preserves_mixed_line_endings_when_payload_changes`

### 2.11 F-11：SSE error 是终态，不能在 EOF 再补正常 finish

`StreamKernel` 新增 source error termination gate：识别到 JSON/SSE error、空 error event、transport `fail()` 或 source parser 产生 `StreamError` 后，后续 source block 被忽略，`finish_source()` 不再生成 Chat stop、Responses completed 或 Gemini finish。目标协议已经输出的 error envelope 保持唯一终态。

#### 回归测试

- `chat_stream_to_gemini_openai_error_chunk_emits_gemini_error_event`
- `chat_stream_to_gemini_transport_error_emits_gemini_error_event`
- `chat_stream_to_anthropic_openai_error_chunk_emits_error_event`

### 2.12 F-12：非流 Responses cancellation 双向映射

#### 原问题

Responses 非流响应的 `status: "cancelled"` / `"canceled"` 经过统一 LLM response 时，如果沿用只识别 `failed`、`incomplete` 和 `completed` 的映射，会丢失“已取消”这一协议终态；反向写回 Responses 时还可能被错误地输出为 `status: "completed"`。这会让客户端看到错误的 finish reason，并破坏流式与非流式语义的一致性。

#### 当前实现

`openai/responses/shared.rs` 现在采用明确的双向映射：

- Responses `cancelled` / `canceled` -> LLM `finish_reason: "cancelled"`；
- LLM `finish_reason: "cancelled"` / `"canceled"` -> Responses `status: "canceled"`；
- `incomplete` 仍映射为 `length`，`failed` 仍映射为 `error`。

这只规范化 Responses 输出中的 status 拼写，不把合法取消结果升级为 `completed`，也不改变 F-1 对 runtime retry/failover/health 的合法终态判断。

#### 回归测试

- `responses_non_stream_cancellation_status_roundtrips_without_completed`

### 2.13 F-13：转换前后都必须参与 runtime 响应分类

#### 原问题

跨协议响应转换可能把原始 Responses `status: "failed"` 或错误 envelope 转成目标协议形状。若 runtime 只检查转换后的 `response.body`，上游已经明确失败的响应可能被当作普通成功内容返回；相反，若转换后的 body 为空，也不能因为丢失了原始 status 就把合法 `cancelled` / `incomplete` 误判为 `EmptyResponse`。

#### 当前实现

`runtime/upstream.rs` 的成功响应分类现在同时检查：

- 转换后的客户端 body：`DebugHttpResponse.body`；
- provider 原始 body：`DebugHttpResponse.upstream_response_body`。

因此：

- 任一 body 表示顶层/嵌套 `failed` 或 error envelope，都分类为 `UpstreamBadRequest`；
- 任一 body 明确表示合法 `incomplete` / `cancelled` / `canceled`，即使 output 为空，也不会触发 `EmptyResponse`；
- 只有转换后和原始 body 都没有实际内容，且不是合法终态或错误 envelope 时，才保留控制流空响应的 `EmptyResponse` 行为。

#### 回归测试

- `success_protocol_error_checks_original_body_after_response_conversion`
- `empty_converted_body_accepts_legal_cancelled_original_response`
- `empty_converted_body_rejects_control_only_original_sse`

## 3. 参考项目复核

### 3.1 cc-switch

只读复核了 `cc-switch` HEAD `a377d793` 的 `src-tauri/src/proxy/providers/transform_codex_chat.rs`、`streaming_codex_chat.rs`、`streaming_responses.rs` 和 `forwarder.rs`：

- Responses -> Chat 的 request-scoped tool context、namespace 展平和反向恢复，与本项目保留 `ConversionContext` 的方向一致。
- streamed Chat tool call 只有在真实 `call_id` 和非空 name 可用时才首次打开；finish 阶段才为长期缺失 ID 的 legacy call 生成 `call_<index>`，与本项目状态机一致。
- 连续 reasoning、trailing reasoning 回挂和 forward merge 的分层处理与本项目一致。
- cc-switch 当前把 cancellation 作为 transform/failover 错误：`transform_responses.rs` 对 `status=cancelled` 返回 `TransformError`，`streaming_responses.rs` 把 terminal snapshot 的 `failed|cancelled` 发为错误，`forwarder.rs` 的 JSON/SSE error detector 也把 `cancelled` 纳入错误。ai-toolbox 没有照搬这一产品策略，而是有意采用 OpenAI/AxonHub 的合法 terminal 语义，不因 cancellation 重试、跨渠道切换或扣健康分。

### 3.2 AxonHub

复核使用 `git show origin/unstable:<path>`，因为本地 checkout `d327c759` 比 `origin/unstable` 落后 212 个提交；当前参考 tip 为 `01707aa6`。重点检查了：

- `llm/transformer/openai/responses/outbound_stream.go`
- `llm/transformer/openai/responses/aggregator.go`
- `llm/transformer/openai/responses/inbound.go`
- `internal/server/orchestrator/pass_through.go` 及相关 pipeline

得到的结论：

- `completed`、`failed`、`cancelled`、`incomplete` 都是明确 terminal；取消会提交完成状态并映射 `finish_reason=cancelled`，aggregator 保留 `canceled` status。
- AxonHub 的非流/聚合方向不会把 cancellation 当作普通 completed；本项目新增的非流 Responses 双向映射与这一 terminal 语义一致。
- AxonHub aggregator 按字段保存 response ID、model、created_at、usage、error 和 incomplete details，稀疏 terminal snapshot 不会整体覆盖前序状态；本项目使用 request-local JSON object 浅层合并实现等价的元数据保留，并额外保留上游完整 terminal output。
- Responses stream 没有 terminal event 时，AxonHub 返回 `ErrStreamIncomplete`。本项目 forced-stream 聚合现在返回连接类错误，而不是伪造 `completed` 或把截断伪装成合法 `incomplete`，语义等价且能复用现有 failover。
- AxonHub 的 raw item/pass-through 能力由独立 pipeline 和提交 `abb9d84e` 引入；本项目 raw Responses tool merge 额外要求 structured signature 数量、顺序和内容完全匹配，是更窄的 fail-closed 防错门控，不是照搬 AxonHub 的架构。
- AxonHub 还实现了 Responses WebSocket executor/session/pool；本项目仍只有 HTTP JSON/SSE，因此没有为了“对齐参考”引入半套 WebSocket。

### 3.3 复核后的方案边界

参考项目支持当前修复的核心协议判断，但没有要求把 provider failover、完整 orchestrator pipeline、WebSocket transport 或跨请求状态全部搬入 transformer。当前方案保留以下有意差异：

- 合法 cancellation 不扣健康、不重试；这是本项目 Gateway 相对 cc-switch 的明确产品策略差异，并与 AxonHub terminal 解析一致。
- raw tool merge 使用完整 structured signature guard；这是针对本项目 sidecar 形态的额外 fail-closed 保护。
- forced-stream 缺 terminal 直接报连接类错误；合法 `response.incomplete` 仍按协议终态透传。
- runtime 响应分类同时参考转换后 body 和原始 upstream body；这是由本项目跨协议转换可能隐藏原始 failure envelope 的数据流决定的，不是把 runtime 分类逻辑下沉到 transformer。

## 4. 已确认不是当前问题的项目

以下项目在历史审计中曾被列为风险，但经代码和测试复核后不应继续作为“未修复问题”描述：

### 4.1 T-2：Responses completed hosted item 覆盖

Responses -> Responses 直通不会进入目标 Responses 状态机；流式请求原样透传，非流客户端的 runtime 聚合器从完整 `response.completed.response` 或所有 `output_item.done` 恢复 output。因此当前路径不会因为 `completed_responses_output()` 的转换范围而丢 hosted item。

### 4.2 R-3：Copilot Chat proactive thinking strip

OpenAI Chat Copilot 路径已经有主动剥离 `thinking` / `redacted_thinking` block 的 provider adapter。Responses 路径或 `reasoning_content` 形态的窄差异不是本轮报告的同一问题，不能把已实现的 Chat 行为继续标记为缺失。

### 4.3 R-4：reverse hook 的事实状态

reverse hook 已存在并在生产 JSON/SSE 路径接线。仍然保留在 `upstream.rs` 的 rectifier、side store、协议转换和 provider adapter 是职责边界设计，不应为了“迁移到 middleware”而扩大本轮改动。

## 5. 设计边界与未改变的行为

- 同协议 source/target 仍然直通，不经结构化 round-trip。
- Provider-specific URL、header、鉴权、Ollama wire format、Bedrock/Vertex、Copilot、xAI 等仍由 runtime 处理。
- Transformer 不依赖数据库、Tauri app handle、provider 表或跨请求 store。
- SSE 转换仍然边读边写，不为协议转换或 reverse middleware 全量缓冲上游流。
- `ConversionContext` 仍是单请求状态；lossy warning、健康状态和 side store 不进入 transformer。
- 本轮未修改前端代码、provider 配置 schema 或数据库结构。

## 6. 验证记录

### 6.1 通过

| 命令 | 结果 |
|------|------|
| `cd tauri && cargo test transformer --no-default-features` | 206 passed |
| `cd tauri && cargo test --lib coding::proxy_gateway::runtime::upstream` | 183 passed |
| `cd tauri && cargo test` | lib 1179 passed、1 ignored；`coding` 78、SQLite JSONB 21、migration 12、Surreal import 4、doc-tests 10 全部通过 |
| `pnpm test` | 267 passed |
| `pnpm exec tsc --noEmit` | 通过 |
| `git diff --check` | 通过 |

### 6.2 非本轮 warning / 工具基线

Rust 测试仍报告仓库既有 warning：

```text
tauri/src/coding/magic_context.rs:125
unused variable: xdg_config_home
```

已执行 `cargo fmt --all -- --check`，但仓库全量格式检查受大量与本轮无关的既有格式差异影响而失败；本轮没有执行全仓格式化，以避免制造无关 diff。代码行为验证以编译、测试和 `git diff --check` 为准。

## 7. 最终状态

审计中列出的 F-1 至 F-13 均已修复并有回归测试，最终收口还覆盖 raw-only 空 signature sidecar、不可签名 structured tool、稀疏 created/in-progress/terminal response snapshot、非流 cancellation 双向映射，以及转换前后双 body 的 runtime 分类。当前工作树没有提交或推送；修复完成的事实应以当前代码和上述测试结果为准，而不是以某个尚不存在的 commit hash 为准。
