/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import type { OpenClawConfig } from '@/types/openclaw';

import {
  extractOpenClawOtherConfigFields,
  mergeOpenClawOtherConfigFields,
} from '../../../../../features/coding/openclaw/utils/openClawOtherConfig.ts';

test('extractOpenClawOtherConfigFields hides the sections owned by other surfaces', () => {
  const result = extractOpenClawOtherConfigFields({
    models: { mode: 'merge', providers: {} },
    agents: { defaults: { model: { primary: 'a/b', fallbacks: [] } } },
    mcp: { servers: { demo: { command: 'demo' } } },
    env: { A: '1' },
    tools: { profile: 'coding' },
  });

  assert.deepEqual(result, {
    env: { A: '1' },
    tools: { profile: 'coding' },
  });
});

test('extractOpenClawOtherConfigFields returns undefined when only hidden sections exist', () => {
  assert.equal(
    extractOpenClawOtherConfigFields({
      models: { mode: 'merge' },
      agents: {},
      mcp: { servers: {} },
    }),
    undefined,
  );
  assert.equal(extractOpenClawOtherConfigFields(null), undefined);
});

test('mergeOpenClawOtherConfigFields keeps mcp written by the MCP page (issue #406)', () => {
  // The backend removes every root section the payload omits, so a payload built
  // from the editor text alone (which never contains `mcp`) would delete the
  // whole `mcp.servers` map.
  const freshFile: OpenClawConfig = {
    models: { mode: 'merge' },
    agents: { defaults: { model: { primary: 'a/b', fallbacks: [] } } },
    mcp: { servers: { demo: { type: 'stdio', command: 'demo' } } },
    env: { OLD: '1' },
  };

  const merged = mergeOpenClawOtherConfigFields(freshFile, { env: { NEW: '2' } });

  assert.deepEqual(merged.mcp, { servers: { demo: { type: 'stdio', command: 'demo' } } });
  assert.deepEqual(merged.models, freshFile.models);
  assert.deepEqual(merged.agents, freshFile.agents);
  assert.deepEqual(merged.env, { NEW: '2' });
});

test('mergeOpenClawOtherConfigFields lets hand-typed keys win and tolerates non-objects', () => {
  const base: OpenClawConfig = { env: { A: '1' }, mcp: { servers: { keep: {} } } };

  assert.deepEqual(
    mergeOpenClawOtherConfigFields(base, { mcp: { servers: { typed: {} } } }).mcp,
    { servers: { typed: {} } },
  );
  assert.deepEqual(mergeOpenClawOtherConfigFields(base, null), base);
  assert.deepEqual(mergeOpenClawOtherConfigFields(base, ['not', 'an', 'object']), base);
});
