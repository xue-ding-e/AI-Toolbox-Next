import assert from 'node:assert/strict';
import test from 'node:test';

import {
  deriveGatewayRequestDisplay,
  formatModelRoute,
  gatewayRequestDisplayKind,
  isGatewayRequestUsageApplicable,
  normalizeAttemptCounts,
  requestExportPrefix,
  requestLineText,
  shouldShowBodyComparison,
} from '../../../../../features/coding/gateway/utils/gatewayFormatters.ts';

test('normalizeAttemptCounts falls back total attempts for legacy request logs', () => {
  assert.deepEqual(normalizeAttemptCounts({ attempt_count: 2, total_attempt_count: 0 }), {
    current: 2,
    total: 2,
  });
});

test('normalizeAttemptCounts keeps total attempts when present', () => {
  assert.deepEqual(normalizeAttemptCounts({ attempt_count: 1, total_attempt_count: 3 }), {
    current: 1,
    total: 3,
  });
});

test('shouldShowBodyComparison only shows distinct stored bodies', () => {
  assert.equal(shouldShowBodyComparison(null, '{"ok":true}'), false);
  assert.equal(shouldShowBodyComparison('{"ok":true}', '{"ok":true}'), false);
  assert.equal(shouldShowBodyComparison('{"upstream":true}', '{"client":true}'), true);
});

test('formatModelRoute ignores placeholder model values', () => {
  assert.equal(formatModelRoute('unknown', 'anthropic/claude-sonnet-4-5', '-'), 'anthropic/claude-sonnet-4-5');
  assert.equal(formatModelRoute('claude-sonnet-4-5', 'anthropic/claude-sonnet-4-5', '-'), 'claude-sonnet-4-5 -> anthropic/claude-sonnet-4-5');
  assert.equal(formatModelRoute('unknown', 'unknown', '-'), '-');
});

test('gateway request display detects model list endpoints across CLIs', () => {
  assert.equal(gatewayRequestDisplayKind({
    method: 'GET',
    path: '/anthropic/v1/models',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'modelList');
  assert.equal(gatewayRequestDisplayKind({
    method: 'GET',
    path: '/openai/v1/models',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'modelList');
  assert.equal(gatewayRequestDisplayKind({
    method: 'GET',
    path: '/gemini/v1beta/models?key=xxx',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'modelList');
  assert.equal(gatewayRequestDisplayKind({
    method: 'GET',
    path: '/gemini/v1beta/models:listModels',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'modelList');
});

test('gateway request display detects compact and connection probes', () => {
  assert.equal(gatewayRequestDisplayKind({
    method: 'POST',
    path: '/openai/v1/responses/compact',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'contextCompact');
  assert.equal(gatewayRequestDisplayKind({
    method: 'HEAD',
    path: '/anthropic',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'connectionProbe');
  assert.equal(gatewayRequestDisplayKind({
    method: 'GET',
    path: '/openai/v1',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'connectionProbe');
  assert.equal(gatewayRequestDisplayKind({
    method: 'HEAD',
    path: '/gemini/v1beta',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'connectionProbe');
});

test('deriveGatewayRequestDisplay exposes title keys and request line metadata', () => {
  const display = deriveGatewayRequestDisplay({
    method: 'GET',
    path: '/openai/v1/models',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  });

  assert.equal(display.kind, 'modelList');
  assert.equal(display.titleKey, 'gateway.page.requests.requestTypes.modelList');
  assert.equal(display.requestLine, 'GET /openai/v1/models');
  assert.equal(display.modelApplicable, false);
});

test('usage display is only applicable to model and compact requests', () => {
  assert.equal(isGatewayRequestUsageApplicable({
    method: 'POST',
    path: '/openai/v1/responses',
    requested_model: 'gpt-5',
    upstream_model_id: 'openai/gpt-5',
  }), true);
  assert.equal(isGatewayRequestUsageApplicable({
    method: 'POST',
    path: '/openai/v1/responses/compact',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), true);
  assert.equal(isGatewayRequestUsageApplicable({
    method: 'GET',
    path: '/openai/v1/models',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), false);
  assert.equal(isGatewayRequestUsageApplicable({
    method: 'HEAD',
    path: '/gemini/v1beta',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), false);
  assert.equal(isGatewayRequestUsageApplicable({
    method: 'POST',
    path: '/openai/v1/embeddings',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), false);
});

test('requestLineText falls back when method and path are absent', () => {
  assert.equal(requestLineText({}, 'Path not recorded'), 'Path not recorded');
  assert.equal(requestLineText({ method: 'get' }, 'Path not recorded'), 'GET');
  assert.equal(requestLineText({ path: '/openai/v1/models' }, 'Path not recorded'), '/openai/v1/models');
});

test('requestExportPrefix avoids unknown filenames for non-model requests', () => {
  assert.equal(requestExportPrefix({
    method: 'GET',
    path: '/openai/v1/models',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'models-list');
  assert.equal(requestExportPrefix({
    method: 'POST',
    path: '/responses/compact',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'compact');
  assert.equal(requestExportPrefix({
    method: 'HEAD',
    path: '/gemini/v1beta',
    requested_model: 'unknown',
    upstream_model_id: 'unknown',
  }), 'probe');
  assert.equal(requestExportPrefix({
    method: 'POST',
    path: '/openai/v1/responses',
    requested_model: 'gpt-5',
    upstream_model_id: 'openai/gpt-5',
  }), 'gpt-5-openai-gpt-5');
});
