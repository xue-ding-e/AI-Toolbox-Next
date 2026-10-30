import { test } from 'node:test';
import assert from 'node:assert/strict';
import { formatBuildVersion } from '../../../../features/settings/utils/buildVersion.ts';
test('build version distinguishes equal versions without trusting arbitrary environment text', () => {
  assert.equal(formatBuildVersion('1.2.1-next.1', 'a'.repeat(40)), '1.2.1-next.1+aaaaaaaaaaaa');
  for (const value of [undefined, '', 'short', '../bad', 42]) assert.equal(formatBuildVersion('1.2.1', value), '1.2.1');
  assert.equal(formatBuildVersion('', undefined), '-');
});
