import assert from 'node:assert/strict';
import test from 'node:test';

import { getMcpCommandPackageVersion } from '../../../../../features/coding/mcp/utils/mcpCommandPackageVersion.ts';

test('detects npx package specs and treats unpinned packages as latest', () => {
  assert.deepEqual(
    getMcpCommandPackageVersion({
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-time'],
    }),
    {
      manager: 'npx',
      packageName: '@modelcontextprotocol/server-time',
      versionLabel: 'latest',
      displayText: 'latest',
    },
  );
});

test('parses scoped npm package versions from the last version separator', () => {
  assert.deepEqual(
    getMcpCommandPackageVersion({
      command: 'npx',
      args: ['@modelcontextprotocol/server-memory@1.2.3'],
    }),
    {
      manager: 'npx',
      packageName: '@modelcontextprotocol/server-memory',
      versionLabel: '1.2.3',
      displayText: 'v1.2.3',
    },
  );
});

test('treats pnpx and tpnx as npx-family runners', () => {
  assert.deepEqual(
    getMcpCommandPackageVersion({
      command: 'C:\\Users\\user\\.local\\bin\\pnpx.cmd',
      args: ['@playwright/mcp@1.56.0'],
    })?.manager,
    'npx',
  );
  assert.deepEqual(
    getMcpCommandPackageVersion({
      command: 'tpnx',
      args: ['-y', 'chrome-devtools-mcp@latest'],
    }),
    {
      manager: 'npx',
      packageName: 'chrome-devtools-mcp',
      versionLabel: 'latest',
      displayText: 'latest',
    },
  );
});

test('uses npx package option when command and package differ', () => {
  assert.deepEqual(
    getMcpCommandPackageVersion({
      command: 'npx',
      args: ['--yes', '--package=@upstash/context7-mcp@0.5.1', 'context7-mcp'],
    }),
    {
      manager: 'npx',
      packageName: '@upstash/context7-mcp',
      versionLabel: '0.5.1',
      displayText: 'v0.5.1',
    },
  );
});

test('detects uvx and uv tool run package specs', () => {
  assert.deepEqual(
    getMcpCommandPackageVersion({
      command: 'uvx',
      args: ['mcp-server-fetch'],
    }),
    {
      manager: 'uv',
      packageName: 'mcp-server-fetch',
      versionLabel: 'latest',
      displayText: 'latest',
    },
  );
  assert.deepEqual(
    getMcpCommandPackageVersion({
      command: 'uv',
      args: ['tool', 'run', '--from', 'mcp-server-fetch==2026.1.0', 'mcp-server-fetch'],
    }),
    {
      manager: 'uv',
      packageName: 'mcp-server-fetch',
      versionLabel: '2026.1.0',
      displayText: 'v2026.1.0',
    },
  );
});

test('supports cmd wrappers and ignores unrelated commands', () => {
  assert.deepEqual(
    getMcpCommandPackageVersion({
      command: 'cmd.exe',
      args: ['/c', 'npx.cmd', '-y', 'chrome-devtools-mcp@0.13.0'],
    }),
    {
      manager: 'npx',
      packageName: 'chrome-devtools-mcp',
      versionLabel: '0.13.0',
      displayText: 'v0.13.0',
    },
  );
  assert.equal(
    getMcpCommandPackageVersion({
      command: 'node',
      args: ['server.js'],
    }),
    null,
  );
  assert.equal(
    getMcpCommandPackageVersion({
      url: 'https://example.com/mcp',
    }),
    null,
  );
});
