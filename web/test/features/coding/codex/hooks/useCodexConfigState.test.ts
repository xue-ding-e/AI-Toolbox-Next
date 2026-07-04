/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { normalizeCodexCatalogModels } from '../../../../../features/coding/codex/utils/codexCatalogModels.ts';

test('normalizeCodexCatalogModels preserves image capability metadata', () => {
  const models = normalizeCodexCatalogModels([
    {
      model: ' text-only-model ',
      displayName: ' Text Only ',
      contextWindow: '128,000',
      supportsImage: false,
      vision: false,
      attachment: false,
      modalities: {
        input: [' text ', 'image', ''],
        output: [' text '],
      },
    },
    {
      model: 'vision-model',
      supportsImage: true,
      modalities: {
        input: ['text', 'image'],
      },
    },
  ]);

  assert.deepEqual(models, [
    {
      model: 'text-only-model',
      displayName: 'Text Only',
      contextWindow: 128000,
      supportsImage: false,
      vision: false,
      attachment: false,
      modalities: {
        input: ['text', 'image'],
        output: ['text'],
      },
    },
    {
      model: 'vision-model',
      supportsImage: true,
      modalities: {
        input: ['text', 'image'],
      },
    },
  ]);
});
