import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const readSource = (relativePath: string) =>
  readFileSync(new URL(relativePath, import.meta.url), 'utf8');

const chipModuleSource = readSource(
  '../../../../../features/coding/shared/gateway/gatewayStatusChip.module.less',
);
const failoverModuleSource = readSource(
  '../../../../../features/coding/shared/gateway/GatewayFailoverButton.module.less',
);
const failoverButtonSource = readSource(
  '../../../../../features/coding/shared/gateway/GatewayFailoverButton.tsx',
);
const aggregateButtonSource = readSource(
  '../../../../../features/coding/shared/gateway/GatewayAggregateButton.tsx',
);
const codexPageSource = readSource(
  '../../../../../features/coding/codex/pages/CodexPage.tsx',
);

/**
 * The provider list header renders the gateway takeover chip and the aggregate
 * entry chip side by side.  They were built from two different visual systems
 * (self-drawn pill vs. Ant Design default button), which is exactly the
 * mismatch users reported, so both now share one pill stylesheet.
 */
test('gateway header chips share one pill language', () => {
  assert.match(chipModuleSource, /\.chip \{/);
  assert.match(chipModuleSource, /height: 22px;/);
  assert.match(chipModuleSource, /border-radius: 11px;/);
  assert.match(chipModuleSource, /\.chipActive \{/);
  assert.match(chipModuleSource, /\.chipCurrent \{/);

  assert.match(
    failoverButtonSource,
    /import chipStyles from '\.\/gatewayStatusChip\.module\.less';/,
  );
  assert.match(
    aggregateButtonSource,
    /import chipStyles from '\.\/gatewayStatusChip\.module\.less';/,
  );
  assert.match(failoverButtonSource, /chipStyles\.chip/);
  assert.match(aggregateButtonSource, /chipStyles\.chip/);

  // The pill must not be re-declared next to the shared module again.
  assert.doesNotMatch(failoverModuleSource, /\.button \{/);
  assert.doesNotMatch(failoverModuleSource, /\.dot_green \{/);

  // Aggregate state shown by the chip follows the shared mode helper instead
  // of a local `mode === 'aggregate'` comparison.
  assert.match(
    codexPageSource,
    /current=\{isGatewayAggregateMode\(gatewayCliStatus\?\.mode\)\}/,
  );
});
