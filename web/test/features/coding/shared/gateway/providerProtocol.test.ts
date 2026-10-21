import assert from 'node:assert/strict';
import test from 'node:test';

import {
  codexWireApiFormatFromConfig,
  grokProviderNeedsGatewayProxy,
  hasClaudeDesktopRoutingModels,
  isGatewayConfigFlagEnabled,
  providerNeedsGatewayProxy,
} from '../../../../../features/coding/shared/gateway/providerProtocol.ts';

test('codex wire api follows the selected model provider table', () => {
  assert.equal(codexWireApiFormatFromConfig(`
model_provider = "chat"
wire_api = "responses"

[model_providers.responses]
wire_api = "responses"

[model_providers.chat]
wire_api = "chat"
`), 'chat');
});

test('codex wire api keeps root-level legacy compatibility', () => {
  assert.equal(codexWireApiFormatFromConfig('wire_api = "chat"'), 'chat');
});

test('gateway config flag parser matches backend truthy compatibility values', () => {
  for (const value of [true, 1, 'true', '1', 'yes', 'on', ' YES ']) {
    assert.equal(isGatewayConfigFlagEnabled(value), true);
  }

  for (const value of [false, 0, 'false', '0', 'no', 'off', '', null, undefined]) {
    assert.equal(isGatewayConfigFlagEnabled(value), false);
  }
});

test('grok only needs gateway for gemini native, not chat/responses/anthropic', () => {
  for (const format of ['openai_chat', 'openai_responses', 'anthropic_messages', 'chat', 'responses', 'messages']) {
    assert.equal(grokProviderNeedsGatewayProxy(format), false);
  }
  assert.equal(grokProviderNeedsGatewayProxy('gemini_native'), true);
  assert.equal(grokProviderNeedsGatewayProxy(null), false);
  // Contrast: single-native compare would wrongly force proxy for chat.
  assert.equal(providerNeedsGatewayProxy('openai_chat', 'openai_responses'), true);
});


test('claude desktop safe-ID remapping needs the gateway independently of protocol', () => {
  assert.equal(hasClaudeDesktopRoutingModels({ 'claude-sonnet-5': { model: 'claude-opus-5' } }), true);
  assert.equal(hasClaudeDesktopRoutingModels({ 'claude-sonnet-5': { model: 'claude-sonnet-5' } }), false);
  assert.equal(hasClaudeDesktopRoutingModels({ 'claude-sonnet-5': { model: 'deepseek-v4-pro' } }), true);
});

test('claude desktop route helper handles absent and whitespace values conservatively', () => {
  assert.equal(hasClaudeDesktopRoutingModels(undefined), false);
  assert.equal(hasClaudeDesktopRoutingModels(null), false);
  assert.equal(hasClaudeDesktopRoutingModels({}), false);
  assert.equal(hasClaudeDesktopRoutingModels({ 'claude-sonnet-5': {} }), false);
  assert.equal(hasClaudeDesktopRoutingModels({ 'claude-sonnet-5': { model: '  ' } }), false);
  assert.equal(hasClaudeDesktopRoutingModels({ ' claude-sonnet-5 ': { model: ' claude-sonnet-5 ' } }), false);
  // This frontend helper does not claim parity with malformed/blank Rust routes.
});
