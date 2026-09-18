import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import type { GatewayCliTakeoverStatus } from '../../../../../services/proxyGatewayApi.ts';
import { isGatewayProxyMode } from '../../../../../features/coding/shared/gateway/providerProtocol.ts';

const codexPageSource = readFileSync(
  new URL('../../../../../features/coding/codex/pages/CodexPage.tsx', import.meta.url),
  'utf8',
);

const status = (
  partial: Partial<GatewayCliTakeoverStatus> & Pick<GatewayCliTakeoverStatus, 'mode'>,
): GatewayCliTakeoverStatus => ({
  cli_key: 'codex',
  state: 'restore_unavailable',
  dot: 'red',
  can_takeover: false,
  can_restore_direct: false,
  gateway_origin: 'http://127.0.0.1:37124',
  runtime_root: 'C:\\fixture\\codex',
  managed_targets: [],
  primary_provider_id: 'provider-1',
  provider_priorities: [],
  message: 'Gateway takeover manifest exists, but one or more backups are missing',
  ...partial,
});

test('Codex keeps takeover locks when the active gateway manifest cannot be restored', () => {
  assert.match(
    codexPageSource,
    /const gatewayTakeoverActive = isGatewayProxyMode\(gatewayCliStatus\?\.mode\);/,
  );
  assert.doesNotMatch(
    codexPageSource,
    /const gatewayTakeoverActive = Boolean\(gatewayCliStatus\?\.can_restore_direct\);/,
  );

  const activeButUnrestorable = status({ mode: 'aggregate' });
  assert.equal(activeButUnrestorable.can_restore_direct, false);
  assert.equal(isGatewayProxyMode(activeButUnrestorable.mode), true);
});
