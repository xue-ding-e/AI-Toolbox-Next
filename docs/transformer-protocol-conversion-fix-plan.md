# Transformer 协议转换修复计划（执行完成版）

> **状态**：已执行并验证
>
> - 初始审计基线：`e0c26ba`
> - 修复前仓库基线：`3c138e6`
> - 当前修复：未提交工作树修改；没有本轮 commit hash
> - 日期：2026-07-25
>
> 配套审计：`docs/transformer-protocol-conversion-audit.md`
>
> 关联参考：`docs/cc-switch-protocol-conversion-merge-review.md`、`docs/gateway-protocol-conversion.md`

## 1. 执行范围

本计划原本用于跟踪 transformer/runtime 审计问题。用户已明确要求把发现的问题全部修复，因此本轮按完整功能范围执行，并在实现后补跑全量验证。

本轮实际修改边界：

- Gateway runtime 的协议终态分类和 response stream 判定；
- reverse JSON/SSE middleware 的 request-scoped 接线及 SSE 透明性；
- Chat source stream 的 tool-call identity 门控；
- Responses reasoning item 的同轮归属和连续合并；
- Responses raw tool fragment 的 structured signature 校验；
- `PipelineContext` 中迁移死字段清理；
- transformer/runtime 模块文档及本计划、审计报告；
- 对 LF/CRLF 混合 SSE delimiter 的相邻边界修复。
- Responses forced-stream 聚合终态和无 terminal event 的 fail-closed 处理。
- reverse SSE 空白 block、EOF tail、混合行尾改写，以及 error 后禁止 EOF 正常 finish。
- 非流 Responses cancellation 与 LLM finish reason 的双向映射。
- 转换后 body 与原始 `upstream_response_body` 的双重 runtime failure/empty 分类。

没有修改前端、数据库 schema、provider profile schema 或提交/推送分支。

## 2. 执行结果总表

| ID | 任务 | 实现位置 | 状态 |
|----|------|----------|------|
| F-1 / MR-F follow-up | 合法 Responses `incomplete` / `cancelled` 不得触发 provider failure | `runtime/upstream.rs` 错误分类、empty detection、stream probe | 已完成 |
| F-2 | 实际 Content-Type 驱动 response streaming，避免 JSON 误入 SSE | `runtime/upstream.rs::should_stream_response` | 已完成 |
| F-2 | reverse SSE no-op 字节透明、保留元数据和 delimiter | `runtime/upstream.rs::rewrite_sse_block_with_outbound_stream` | 已完成 |
| F-3 / MR-T | tool identity 首次 emit 等待真实 ID，finish 才允许 synthetic fallback | `transformer/stream.rs` | 已完成 |
| F-4 / MR-B | pending reasoning 限制在当前 user turn，连续 reasoning 不丢失 | `transformer/openai/responses/shared.rs` | 已完成 |
| F-5 / T-3 | raw merge 前校验 structured signature 的数量/顺序/内容，空集合和不可签名项也 fail-closed | `transformer/openai/responses/shared.rs` | 已完成 |
| F-6 | 刷新审计报告和本计划的基线、状态和验证记录 | `docs/*.md` | 已完成 |
| F-7 / E-3 | 移除 `PipelineContext.lossy_warnings` | `runtime/middleware.rs`、`runtime/upstream.rs` | 已完成 |
| F-8 | 混合 LF/CRLF 时消费最早 SSE delimiter | `transformer/sse.rs`、`runtime/upstream.rs` | 已完成 |
| F-9 | Responses forced-stream 聚合识别取消终态；无 terminal event 返回连接错误；稀疏 response snapshot 按字段合并 | `runtime/upstream.rs` | 已完成 |
| F-10 | reverse SSE 空白字节透传，改写时保留逐行 LF/CRLF | `runtime/upstream.rs` | 已完成 |
| F-11 | source error 后阻止后续 block 和 EOF synthetic finish | `transformer/stream.rs` | 已完成 |
| F-12 | 非流 Responses cancellation 双向映射，禁止退化为 `completed` | `transformer/openai/responses/shared.rs` | 已完成 |
| F-13 | 转换后与原始 upstream body 同时参与成功响应分类和 empty detection | `runtime/upstream.rs` | 已完成 |
| 文档约束 | 回写合法终态、SSE 透明性、tool ID、reasoning 归属、signature 校验规则 | 两个模块 `AGENTS.md` | 已完成 |

## 3. 实现说明

### 3.1 Runtime 终态和流判定

`gateway_body_reports_error()` 现在只把真正的 error envelope 和 `failed` 终态报告为失败。`incomplete`、`cancelled`、`canceled` 作为合法终态提交响应，空 output 也不触发 `EmptyResponse`。非流 Responses 状态与 LLM finish reason 采用显式双向映射：`cancelled` / `canceled` -> `cancelled`，反向 canonicalize 为 Responses `canceled`，不会输出 `completed`。成功响应分类同时检查转换后的 body 和原始 `upstream_response_body`，避免转换隐藏失败 envelope 或把合法取消结果误判为空。Responses forced-stream 聚合只有在收到实际 terminal event 后才返回 JSON；无 terminal 的截断流返回 `GatewayFailureKind::Connection`，不会伪造 `completed` 或合法 `incomplete`。

response stream 判定顺序：

1. `text/event-stream` -> SSE；
2. 明确的其他 Content-Type（尤其 `application/json`）-> 非 SSE；
3. 缺少 Content-Type -> 兼容使用请求/route 的 streaming 声明；
4. Ollama `application/x-ndjson` -> 先转 OpenAI Chat SSE，再进入后续链路。

### 3.2 Reverse SSE

reverse stream middleware 仍然使用 request-scoped `Pipeline` 和 `PipelineContext`，并以 reverse 顺序执行。每个完整 SSE block 的 JSON event 会在 middleware 前后比较：

- value 未变化：直接返回原始 block bytes；
- value 变化：只替换 `data` payload；
- 空白 block/EOF tail 也原样透传；payload 改写逐行保留原始 LF/CRLF；
- metadata、comment、id、retry、非 data 行和原始换行格式继续保留；
- `[DONE]`、非 JSON 和无 data block 不重建。

这保持了 CCH 等确需 response reverse 的窄 compat，同时避免 no-op middleware 造成协议格式漂移。

### 3.3 MR-T

`SourceStreamState` 对未打开 tool call 同时检查 name 和真实 ID。ready tool 按 index 升序打开；finish 时对 legacy 无 ID tool 生成 `call_<index>`，因此 synthetic ID 不会在后续仍可能收到真实 ID 时提前泄漏。

### 3.4 MR-B

Responses request conversion 用当前 user 段的 `last_assistant_index` 管理 pending reasoning：

- 连续 reasoning 先合并；
- forward merge 与 trailing append 分开；
- user boundary 重置归属；
- 没有同轮 assistant 时保留 standalone reasoning。

### 3.5 T-3

只要原请求存在 raw tool fragment，就保存 structured signature sidecar；原 structured 集合为空时也保存 `[]`，不能把“原集合为空”误当成“没有完整性证据”。同时保存 `openai_responses_tool_signatures_complete=true`；sidecar 缺失、格式异常、marker 不是 `true`，或任一 `function` / `custom` tool 的 name 缺失或为空白时，raw fragment merge 一律 fail-closed。只有 sidecar 与当前 structured tools 的签名数量、顺序和内容完全一致时才执行 merge，完整匹配后才进行 raw collision 过滤。

### 3.6 字段归属

lossy warning 是 runtime request preparation 的结果，不是协议转换上下文。它现在只由 `PreparedUpstreamBody.lossy_warnings` 持有，并用于最终响应 header；`PipelineContext` 和 `ConversionContext` 不再承载它。

### 3.7 F-9：Responses forced-stream 终态

Responses 聚合器现在识别 `response.completed`、`response.failed`、`response.incomplete`、`response.cancelled` 和 `response.canceled`。只有实际 terminal event 到达才生成非流 JSON；流在 terminal event 前结束时返回 `GatewayFailureKind::Connection`，交给已有 retry/failover。

这和 AxonHub `origin/unstable` 的 `ErrStreamIncomplete` 语义一致，并且避免把“上游截断”错误地纳入合法 `incomplete` 的免重试规则。

created、in-progress 和 terminal event 的 response 都可能是稀疏 snapshot，聚合器按字段浅层合并，避免后续空对象或只含 status/usage 的对象覆盖已有 id、model、created_at。terminal `response` 省略、为 `null`、空对象或非对象时保留最近的 base snapshot；terminal `status` 只有非空字符串才可信，否则按事件类型回填。存在 `response.output_item.done` 且 terminal snapshot 没有非空 output array（包括缺失、`null` 或空数组）时，以聚合 item 重建 created snapshot 中的陈旧空 `output`，避免最终返回 JSON `null` 或丢失元数据、部分输出。

### 3.8 F-10：Reverse SSE 原始字节和行尾

wrapper 不再用 `trim()` 丢弃空白 block 或 EOF tail；no-op、无 data、非 JSON 和 `[DONE]` 都保留原始字节。发生实际 middleware 改写时，仅替换 data payload，并逐行保留原始 LF/CRLF。

### 3.9 F-11：错误终态门控

`StreamKernel` 记录 source error termination。JSON/SSE error、空 error event、transport `fail()` 或 source parser 的 `StreamError` 一旦发生，后续 source block 被忽略，EOF 不再补正常 finish；目标协议只保留 error envelope。

### 3.10 F-12：非流 Responses cancellation 双向映射

`responses_status_to_finish()` 将 Responses `status: "cancelled"` / `"canceled"` 映射为 LLM `finish_reason: "cancelled"`；`finish_to_responses_status()` 将 LLM `finish_reason: "cancelled"` / `"canceled"` 映射为 Responses `status: "canceled"`。`incomplete` / `length` 和 `failed` / `error` 的既有映射不变。该映射只规范化协议字段，不参与 runtime 的合法终态与 failover 分类。

### 3.11 F-13：转换前后双 body 的 runtime 分类

`classify_success_protocol_error()` 与 `classify_empty_success_response()` 通过共享 helper 同时检查：

- 转换后的 `DebugHttpResponse.body`；
- 原始 provider body `DebugHttpResponse.upstream_response_body`。

任一 body 含 `failed` 或非空 error envelope，都必须分类为 `UpstreamBadRequest`。任一 body 明确含合法 `incomplete` / `cancelled` / `canceled`，即使转换后的 body 为空，也不能返回 `EmptyResponse`；只有原始和最终 body 都没有实际内容且不是合法终态时，才维持控制流空响应失败。

### 3.12 参考项目复核

#### cc-switch

已只读复核 `cc-switch` HEAD `a377d793` 的 `transform_codex_chat.rs`、`streaming_codex_chat.rs`、`transform_responses.rs`、`streaming_responses.rs` 和 `forwarder.rs`。tool identity、连续 index 门控、finish synthetic ID、trailing reasoning forward merge 与当前实现方向一致。cc-switch 当前在非流转换、流转换和 forwarder error detector 中都把 `status=cancelled` 当作 transform/failover 错误；本项目有意采用 OpenAI/AxonHub 的合法 terminal 语义，不因 cancellation 扣健康或重试，这是明确的产品策略差异。

#### AxonHub

已通过 `git show origin/unstable:<path>` 复核 AxonHub `origin/unstable` `01707aa6` 的 `outbound_stream.go`、`aggregator.go`、`inbound.go` 和 orchestrator pass-through。AxonHub 将 completed/failed/cancelled/incomplete 都视为 terminal，无 terminal 则返回 `ErrStreamIncomplete`；本项目采用连接类错误复用现有 failover，并让非流 cancellation 映射保持同一终态语义。AxonHub 的 raw pass-through 和 WebSocket executor 不属于本轮边界；本项目的 structured signature 完整匹配门控是更窄的 fail-closed 加强。

## 4. 回归测试

本轮新增或强化的关键测试：

- Runtime：
  - `incomplete_and_cancelled_terminal_responses_are_not_provider_failures`
  - `streaming_first_chunk_accepts_partial_output_with_incomplete_terminal_event`
  - `response_streaming_uses_actual_content_type_for_json_fallbacks`
  - `rewrite_sse_block_zero_diff_without_billing_context`
  - `rewrite_sse_block_restores_billing_cch_and_passthrough_done`
  - `reverse_sse_stream_preserves_blank_blocks_and_whitespace_tail`
  - `rewrite_sse_block_preserves_mixed_line_endings_when_payload_changes`
  - `reverse_sse_parser_uses_the_earliest_mixed_line_ending_delimiter`
  - `gateway_body_reports_error_detects_delimiterless_multiline_sse_error_payload`
  - `success_protocol_error_checks_original_body_after_response_conversion`
  - `empty_converted_body_accepts_legal_cancelled_original_response`
  - `empty_converted_body_rejects_control_only_original_sse`
  - `codex_official_sse_aggregate_preserves_cancelled_terminal_response`
  - `codex_official_sse_aggregate_rejects_missing_terminal_event`
  - `codex_official_sse_aggregate_merges_sparse_response_snapshots`
- Transformer stream：
  - `chat_stream_tool_call_waits_for_real_id_before_first_emit`
  - `chat_stream_anthropic_tool_start_waits_for_real_id_before_first_emit`
  - `chat_stream_tool_calls_flush_in_index_order_despite_late_identity`
  - `chat_stream_tool_call_uses_synthetic_id_only_at_eof_without_real_id`
  - `chat_stream_to_gemini_openai_error_chunk_emits_gemini_error_event`
- Responses reasoning/raw tools：
  - `consecutive_reasoning_items_are_preserved_when_followed_by_assistant`
  - `reasoning_without_same_turn_assistant_is_kept_before_user_boundary`
  - `trailing_reasoning_does_not_cross_user_boundary`
  - `raw_tools_merge_skips_fragments_when_structured_signatures_are_reordered`
  - `raw_only_tools_preserve_an_empty_structured_signature_sidecar`
  - `raw_tools_merge_skips_fragments_when_structured_tool_is_added_to_empty_signature_set`
  - `raw_tools_merge_fails_closed_when_any_structured_tool_has_no_signature`
  - `responses_non_stream_cancellation_status_roundtrips_without_completed`
- 通用 SSE：
  - `transformer::sse::tests::parser_uses_the_earliest_mixed_line_ending_delimiter`

## 5. 验证结果

### 5.1 已通过

```text
cd tauri && cargo test transformer --no-default-features
206 passed; 0 failed

cd tauri && cargo test --lib coding::proxy_gateway::runtime::upstream
183 passed; 0 failed

cd tauri && cargo test
lib: 1179 passed; 1 ignored
tests/coding.rs: 78 passed
tests/sqlite_jsonb.rs: 21 passed
tests/sqlite_migration_state.rs: 12 passed
tests/sqlite_surreal_import.rs: 4 passed
doc-tests: 10 passed

pnpm test
267 passed; 0 failed

pnpm exec tsc --noEmit
通过

git diff --check
通过
```

### 5.2 已知基线 warning

Rust 测试编译仍有既有 warning：

```text
tauri/src/coding/magic_context.rs:125
unused variable: xdg_config_home
```

已执行 `cargo fmt --all -- --check`，但受仓库既有全量格式差异影响而失败；本轮没有执行全仓格式化。没有发现由本轮代码引入的 `git diff --check` 问题。

## 6. 明确不做和状态解释

- T-2（Responses hosted output 覆盖）经复核是伪问题，当前直通/聚合路径完整保留 output，不再作为待办。
- R-3（Copilot Chat proactive thinking strip）在 Chat 路径已经存在，不重复实现。
- R-4 的 reverse hook 已存在并生产接线；本轮修复的是 stream 判定和 SSE 透明性，不把所有 rectifier/side store 强行迁到 middleware。
- I-2/I-3/I-4、R-5 等历史信息性差异不属于本轮发现的问题。
- 所有“已完成”均指当前工作树和测试结果，不代表存在新的 commit。当前没有执行 commit 或 push。
