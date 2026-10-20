import type { GatewayRequestLogDetail, GatewayRequestLogItem } from '../../../../../services/proxyGatewayApi';

// These are synthetic DTOs, never captured requests, credentials, or user logs.
// Keep the list DTO separate: observations belong only to the detail endpoint.
const baseDetail: GatewayRequestLogDetail = {
  trace_id: 'fixture-observations',
  data_source: 'proxy',
  started_at: '2026-01-01T00:00:00Z',
  ended_at: '2026-01-01T00:00:01Z',
  cli_key: 'codex',
  route_name: 'responses',
  method: 'POST',
  path: '/v1/responses',
  provider_id: 'fixture-provider',
  provider_name: 'Synthetic provider',
  requested_model: 'fixture-model',
  upstream_model_id: 'fixture-model',
  upstream_url: null,
  status_code: 200,
  upstream_status_code: 200,
  success: true,
  error_category: null,
  error_message: null,
  duration_ms: 1000,
  attempt_count: 1,
  total_attempt_count: 1,
  failover: false,
  input_tokens: 10,
  output_tokens: 5,
  cache_read_tokens: 0,
  cache_creation_tokens: 0,
  total_tokens: 15,
  request_body_bytes: 0,
  response_body_bytes: 0,
  is_streaming: false,
  first_token_ms: null,
  request_headers: null,
  request_body: null,
  upstream_request_body: null,
  response_headers: null,
  upstream_response_body: null,
  response_body: null,
  provider_attempts: [],
};

export const HTML_LIKE_SUMMARY = '<img data-observation-injection src="data:," onerror="window.__observationExecuted=true"><script data-observation-injection>window.__observationExecuted=true</script><b data-observation-injection>synthetic & summary</b>';

const scenarios = {
  'state-matrix': {
    native_tools: [
      { kind: 'web_search', declared: true, forwarded: false, used: false },
      { kind: 'file_search', declared: false, forwarded: true, used: false },
      { kind: 'image_generation', declared: false, forwarded: false, used: true },
      { kind: 'mcp', declared: true, forwarded: true, used: false },
      { kind: 'code_interpreter', declared: true, forwarded: true, used: true },
    ],
  },
  filtered: {
    native_tools: [{ kind: 'web_search', declared: true, forwarded: false, used: false }],
    rewrites: [{ kind: 'native_tool_filtered', detail: 'web_search' }],
  },
  rewrites: {
    rewrites: [
      { kind: 'lossy_conversion', detail: 'native tools normalized' },
      { kind: 'prompt_cache_key' },
      { kind: 'service_tier', detail: null },
      { kind: 'billing_cch_stripped', detail: '' },
    ],
  },
  legacy: {
    // A legacy record has no observation fields, even if a body looks like a
    // native declaration/result. The view must not invent markers by parsing it.
    request_body: '{"tools":[{"type":"web_search"}],"fixture":"legacy-body"}',
    response_body: '{"output":[{"type":"web_search_call"}],"fixture":"legacy-response"}',
  },
  empty: { native_tools: [], rewrites: [] },
  'safe-summary': { rewrites: [{ kind: 'lossy_conversion', detail: HTML_LIKE_SUMMARY }] },
} satisfies Record<string, Partial<GatewayRequestLogDetail>>;

export const observationDetails: GatewayRequestLogDetail[] = Object.entries(scenarios).map(([scenario, fields]) => ({
  ...baseDetail,
  ...fields,
  trace_id: `fixture-observations-${scenario}`,
}));

export const observationRows: GatewayRequestLogItem[] = observationDetails.map(detail => ({
  trace_id: detail.trace_id,
  data_source: 'proxy',
  cli_key: 'codex',
  route_name: 'responses',
  method: 'POST',
  path: '/v1/responses',
  provider_id: 'fixture-provider',
  provider_name: 'Synthetic provider',
  requested_model: 'fixture-model',
  upstream_model_id: 'fixture-model',
  reasoning_effort: null,
  service_tier: null,
  compaction_mode: null,
  status_code: 200,
  success: true,
  error_message: null,
  created_at: '2026-01-01T00:00:01Z',
  duration_ms: 1000,
  input_tokens: 10,
  output_tokens: 5,
  cache_read_tokens: 0,
  cache_creation_tokens: 0,
  total_tokens: 15,
  total_cost_usd: '0.0001',
  is_streaming: false,
  first_token_ms: null,
}));
