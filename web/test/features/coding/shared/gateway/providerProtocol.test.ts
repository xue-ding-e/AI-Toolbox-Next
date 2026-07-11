import assert from 'node:assert/strict';
import test from 'node:test';

import {
  codexWireApiFormatFromConfig,
  isGatewayConfigFlagEnabled,
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
