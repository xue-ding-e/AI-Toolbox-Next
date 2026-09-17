import assert from 'node:assert/strict';
import test from 'node:test';

import {
  MCP_SERVER_NAME_MAX_LENGTH,
  MCP_SERVER_NAME_PATTERN,
  isMcpServerNameValidForDsh,
} from '../../../../../features/coding/mcp/utils/mcpServerName.ts';

test('isMcpServerNameValidForDsh accepts the names dsh can load', () => {
  assert.equal(isMcpServerNameValidForDsh('a'), true);
  assert.equal(isMcpServerNameValidForDsh('my-server_1'), true);
  assert.equal(isMcpServerNameValidForDsh('A'.repeat(MCP_SERVER_NAME_MAX_LENGTH)), true);
});

test('isMcpServerNameValidForDsh rejects what dsh would refuse to load', () => {
  assert.equal(isMcpServerNameValidForDsh(''), false);
  assert.equal(isMcpServerNameValidForDsh('my server'), false);
  assert.equal(isMcpServerNameValidForDsh('a.b'), false);
  assert.equal(isMcpServerNameValidForDsh('名字'), false);
  assert.equal(isMcpServerNameValidForDsh('a'.repeat(MCP_SERVER_NAME_MAX_LENGTH + 1)), false);
});

test('the pattern is reusable across calls', () => {
  // A stateful (global-flagged) pattern would alternate results here.
  assert.equal(MCP_SERVER_NAME_PATTERN.test('ok-name'), true);
  assert.equal(MCP_SERVER_NAME_PATTERN.test('ok-name'), true);
});
