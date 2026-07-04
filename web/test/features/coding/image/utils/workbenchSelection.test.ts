import assert from 'node:assert/strict';
import test from 'node:test';

import type { ImageChannel } from '../../../../../features/coding/image/services/imageApi.ts';
import {
  buildWorkbenchModelOptions,
  getAvailableChannelsForMode,
  resolveWorkbenchSelection,
} from '../../../../../features/coding/image/utils/workbenchSelection.ts';

const createChannel = (
  overrides: Partial<ImageChannel> & Pick<ImageChannel, 'id' | 'name' | 'sort_order' | 'models'>
): ImageChannel => ({
  provider_kind: 'openai_compatible',
  base_url: 'https://example.test/v1',
  api_key: 'test-key',
  generation_path: null,
  edit_path: null,
  timeout_seconds: 300,
  enabled: true,
  created_at: 1,
  updated_at: 1,
  ...overrides,
});

test('workbench channel options are filtered by the selected image mode', () => {
  const modelOptions = buildWorkbenchModelOptions([
    createChannel({
      id: 'channel-text',
      name: 'Text Channel',
      sort_order: 2,
      models: [{
        id: 'shared-image-model',
        name: 'Shared Image Model',
        supports_text_to_image: true,
        supports_image_to_image: false,
        enabled: true,
      }],
    }),
    createChannel({
      id: 'channel-edit',
      name: 'Edit Channel',
      sort_order: 1,
      models: [{
        id: 'shared-image-model',
        name: 'Shared Image Model',
        supports_text_to_image: false,
        supports_image_to_image: true,
        enabled: true,
      }],
    }),
  ]);

  const selectedModel = modelOptions.find((model) => model.id === 'shared-image-model') ?? null;

  assert.deepEqual(
    getAvailableChannelsForMode(selectedModel, 'text_to_image').map((channel) => channel.id),
    ['channel-text']
  );
  assert.deepEqual(
    getAvailableChannelsForMode(selectedModel, 'image_to_image').map((channel) => channel.id),
    ['channel-edit']
  );
});

test('resolveWorkbenchSelection picks the first sorted channel when model changes', () => {
  const modelOptions = buildWorkbenchModelOptions([
    createChannel({
      id: 'channel-late',
      name: 'Later Channel',
      sort_order: 20,
      models: [{
        id: 'image-model',
        name: 'Image Model',
        supports_text_to_image: true,
        supports_image_to_image: true,
        enabled: true,
      }],
    }),
    createChannel({
      id: 'channel-first',
      name: 'First Channel',
      sort_order: 10,
      models: [{
        id: 'image-model',
        name: 'Image Model',
        supports_text_to_image: true,
        supports_image_to_image: true,
        enabled: true,
      }],
    }),
  ]);

  assert.deepEqual(
    resolveWorkbenchSelection({
      mode: 'text_to_image',
      modelId: 'image-model',
      channelId: 'channel-late',
      modelOptions,
      preferFirstChannel: true,
    }),
    {
      modelId: 'image-model',
      channelId: 'channel-first',
    }
  );
});
