import assert from 'node:assert/strict';
import test from 'node:test';

import { parseMcpServersFromJsonValue } from '../../../../../features/coding/mcp/utils/mcpJsonImport.ts';

const phpStormCommand = 'C:\\Program Files\\JetBrains\\PhpStorm 2026.1.1\\jbr\\bin\\java';
const phpStormClasspath = [
  'C:\\Program Files\\JetBrains\\PhpStorm 2026.1.1\\plugins\\mcpserver\\lib\\mcpserver-frontend.jar',
  'C:\\Program Files\\JetBrains\\PhpStorm 2026.1.1\\lib\\util-8.jar',
].join(';');

test('parseMcpServersFromJsonValue preserves spaces in stdio command and args', () => {
  const servers = parseMcpServersFromJsonValue({
    mcpServers: {
      phpstorm: {
        type: 'stdio',
        env: {
          IJ_MCP_SERVER_PORT: '64342',
        },
        command: phpStormCommand,
        args: [
          '-classpath',
          phpStormClasspath,
          'com.intellij.mcpserver.stdio.McpStdioRunnerKt',
        ],
      },
    },
  });

  assert.equal(servers.length, 1);
  assert.equal(servers[0].name, 'phpstorm');
  assert.equal(servers[0].server_type, 'stdio');

  const serverConfig = servers[0].server_config as { command: string; args: string[]; env?: Record<string, string> };
  assert.equal(serverConfig.command, phpStormCommand);
  assert.deepEqual(serverConfig.args, [
    '-classpath',
    phpStormClasspath,
    'com.intellij.mcpserver.stdio.McpStdioRunnerKt',
  ]);
  assert.deepEqual(serverConfig.env, {
    IJ_MCP_SERVER_PORT: '64342',
  });
});

test('parseMcpServersFromJsonValue accepts a bare single server config object', () => {
  const servers = parseMcpServersFromJsonValue({
    type: 'stdio',
    env: {
      IJ_MCP_SERVER_PORT: '64342',
    },
    command: phpStormCommand,
    args: [
      '-classpath',
      phpStormClasspath,
      'com.intellij.mcpserver.stdio.McpStdioRunnerKt',
    ],
  });

  assert.equal(servers.length, 1);
  assert.equal(servers[0].name, 'imported-mcp-server');

  const serverConfig = servers[0].server_config as { command: string; args: string[] };
  assert.equal(serverConfig.command, phpStormCommand);
  assert.equal(serverConfig.args[1], phpStormClasspath);
});

test('parseMcpServersFromJsonValue ignores non-server nested objects in server maps', () => {
  const servers = parseMcpServersFromJsonValue({
    env: {
      IJ_MCP_SERVER_PORT: '64342',
    },
  });

  assert.deepEqual(servers, []);
});
