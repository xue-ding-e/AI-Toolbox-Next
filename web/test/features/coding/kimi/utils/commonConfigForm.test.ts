import test from 'node:test';
import assert from 'node:assert/strict';

import { buildKimiCommonConfigSubmitValues } from '../../../../../features/coding/kimi/utils/commonConfigForm.ts';

/**
 * The general-config modal owns only the TOML payload. Root directory editing
 * lives in RootDirectoryModal; the backend keeps the stored `rootDir` when a
 * save omits `rootDir`/`clearRootDir`. Regression guard for the old form that
 * always sent `clearRootDir: false` while letting the user clear the field,
 * which silently resurrected the previous root directory.
 */
test('common config submit values carry only the config payload', () => {
  const values = buildKimiCommonConfigSubmitValues('[server]\nfoo = 1\n');
  assert.deepEqual(Object.keys(values).sort(), ['config']);
  assert.equal(values.config, '[server]\nfoo = 1\n');
  assert.equal('rootDir' in values, false);
  assert.equal('clearRootDir' in values, false);
});
