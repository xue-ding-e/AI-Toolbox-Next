/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildCodexConnectivityModelIds,
  codexCatalogRowKey,
  findCodexCatalogRowIndex,
  fromCodexCatalogModelFormValues,
  removeCodexCatalogModels,
  reorderCodexCatalogModels,
  resolveCodexDefaultReasoningEffort,
  toCodexCatalogModelFormValues,
  upsertCodexCatalogModel,
} from '../../../../../features/coding/codex/utils/codexCatalogModels.ts';
import type { CodexCatalogModel } from '../../../../../types/codex.ts';

const rows: CodexCatalogModel[] = [
  { model: 'gpt-5.5', displayName: 'GPT-5.5', contextWindow: 400000 },
  { model: 'gpt-5.5', displayName: 'GPT-5.5 Fast' },
  { model: 'deepseek-v4-flash' },
];

test('row keys stay unique for one upstream model exposed under several menu names', () => {
  const keys = rows.map((row) => codexCatalogRowKey(row));
  assert.equal(new Set(keys).size, 3);
  assert.equal(findCodexCatalogRowIndex(rows, codexCatalogRowKey(rows[1])), 1);
  assert.equal(findCodexCatalogRowIndex(rows, 'missing'), -1);
});

test('upsert replaces the edited row and appends when the previous key is gone', () => {
  const replaced = upsertCodexCatalogModel(
    rows,
    { model: 'gpt-5.5', displayName: 'GPT-5.5 Turbo' },
    codexCatalogRowKey(rows[1]),
  );
  assert.deepEqual(replaced.map((row) => row.displayName), ['GPT-5.5', 'GPT-5.5 Turbo', undefined]);

  const appended = upsertCodexCatalogModel(rows, { model: 'glm-5.3' }, 'missing');
  assert.equal(appended.length, 4);
  assert.equal(appended[3].model, 'glm-5.3');
});

test('remove and reorder address rows by key, keeping duplicate upstream ids apart', () => {
  const withoutSecond = removeCodexCatalogModels(rows, [codexCatalogRowKey(rows[1])]);
  assert.deepEqual(withoutSecond.map((row) => row.displayName), ['GPT-5.5', undefined]);

  const reordered = reorderCodexCatalogModels(rows, [
    codexCatalogRowKey(rows[2]),
    codexCatalogRowKey(rows[0]),
    'missing',
  ]);
  assert.deepEqual(reordered.map((row) => row.model), [
    'deepseek-v4-flash',
    'gpt-5.5',
    'gpt-5.5',
  ]);
  assert.equal(reordered[1].displayName, 'GPT-5.5');
});

test('form values round-trip every catalog field the dialog edits', () => {
  const row: CodexCatalogModel = {
    model: 'glm-5.3',
    displayName: 'GLM 5.3',
    contextWindow: 1000000,
    supportsImage: false,
    vision: false,
    attachment: true,
    modalities: { input: ['text'], output: ['text'] },
    reasoningLevels: ['low', 'high'],
    defaultReasoningLevel: 'high',
    serviceTiers: ['priority'],
  };

  const values = toCodexCatalogModelFormValues(row);
  assert.equal(values.imageSupport, 'rejects');
  const restored = fromCodexCatalogModelFormValues(values, row);
  assert.deepEqual(restored, row);
});

test('explicit image choice wins and Auto clears the explicit flag', () => {
  const base: CodexCatalogModel = { model: 'm', supportsImage: false, vision: false };

  const auto = fromCodexCatalogModelFormValues({ model: 'm', imageSupport: 'auto' }, base);
  assert.equal(auto.supportsImage, undefined);
  assert.equal(auto.vision, false);

  const supports = fromCodexCatalogModelFormValues({ model: 'm', imageSupport: 'supports' }, base);
  assert.equal(supports.supportsImage, true);

  const rejects = fromCodexCatalogModelFormValues({ model: 'm', imageSupport: 'rejects' }, base);
  assert.equal(rejects.supportsImage, false);
});

test('clearing optional fields drops them and keeps the default level inside the level list', () => {
  const cleared = fromCodexCatalogModelFormValues({
    model: 'gpt-5.5',
    displayName: '   ',
    contextWindow: 0,
    reasoningLevels: [],
    defaultReasoningLevel: 'high',
    serviceTiers: [],
    inputModalities: [],
    outputModalities: [],
  });
  assert.deepEqual(cleared, { model: 'gpt-5.5' });

  const mismatched = fromCodexCatalogModelFormValues({
    model: 'gpt-5.5',
    reasoningLevels: ['low', 'high'],
    defaultReasoningLevel: 'max',
  });
  assert.deepEqual(mismatched.reasoningLevels, ['low', 'high']);
  assert.equal(mismatched.defaultReasoningLevel, undefined);
});

test('resolves the config.toml reasoning effort from the primary catalog row', () => {
  const catalog: CodexCatalogModel[] = [
    { model: 'gpt-5.5', displayName: 'GPT-5.5', defaultReasoningLevel: 'high' },
    { model: 'deepseek-v4-flash' },
    { model: 'glm-5.3', defaultReasoningLevel: '  max  ' },
  ];

  assert.equal(resolveCodexDefaultReasoningEffort(catalog, 'gpt-5.5'), 'high');
  assert.equal(resolveCodexDefaultReasoningEffort(catalog, ' glm-5.3 '), 'max');
  // A main model without a declared default level falls back to xhigh.
  assert.equal(resolveCodexDefaultReasoningEffort(catalog, 'deepseek-v4-flash'), 'xhigh');
  // A main model missing from the catalog must leave config.toml untouched.
  assert.equal(resolveCodexDefaultReasoningEffort(catalog, 'missing'), undefined);
  assert.equal(resolveCodexDefaultReasoningEffort(catalog, ''), undefined);
});

test('connectivity model ids union the config default with every catalog row', () => {
  assert.deepEqual(
    buildCodexConnectivityModelIds('gpt-5.5', [
      { model: 'gpt-5.5', displayName: 'GPT-5.5' },
      { model: 'gpt-5.5', displayName: 'GPT-5.5 Fast' },
      { model: 'deepseek-v4-flash' },
    ]),
    ['gpt-5.5', 'deepseek-v4-flash'],
  );
  assert.deepEqual(buildCodexConnectivityModelIds('', undefined), []);
  assert.deepEqual(buildCodexConnectivityModelIds(undefined, [{ model: ' glm-5.3 ' }]), ['glm-5.3']);
});

test('an unset image choice preserves the stored explicit flag', () => {
  // The dialog no longer edits image input, so a missing value must fall back to
  // the stored row instead of silently dropping an explicit supportsImage.
  const base: CodexCatalogModel = { model: 'm', supportsImage: false, serviceTiers: ['priority'] };
  const preserved = fromCodexCatalogModelFormValues({ model: 'm', serviceTiers: ['priority'] }, base);
  assert.equal(preserved.supportsImage, false);
  assert.deepEqual(preserved.serviceTiers, ['priority']);

  const cleared = fromCodexCatalogModelFormValues({ model: 'm', imageSupport: 'auto' }, base);
  assert.equal(cleared.supportsImage, undefined);
});
