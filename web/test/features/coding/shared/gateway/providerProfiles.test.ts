import assert from 'node:assert/strict';
import test from 'node:test';

import {
  CUSTOM_PROVIDER_PROFILE_ID,
  findGatewayProviderEndpoint,
  findGatewayProviderEndpointByReference,
  getGatewayProviderApiFormatFromMeta,
  getGatewayProviderProfilesForTool,
  inferGatewayProviderEndpointSelection,
  inferUniqueGatewayProviderEndpointSelection,
  mergeGatewayProfileReferenceIntoMeta,
  toGatewayProviderProfileReference,
  updateGatewayProviderProfiles,
} from '../../../../../features/coding/shared/gateway/providerProfiles.ts';

test('gateway profile reference resolves Gemini endpoint independent of legacy fields', () => {
  updateGatewayProviderProfiles({
    schemaVersion: 1,
    profiles: [
      {
        id: 'deepseek',
        providerType: 'deepseek',
        label: 'DeepSeek',
        tools: {
          gemini: {
            defaultEndpointId: 'openai_chat',
            endpoints: [
              {
                id: 'openai_chat',
                label: 'OpenAI Chat',
                apiFormat: 'openai_chat',
                baseUrl: 'https://api.deepseek.com',
              },
              {
                id: 'anthropic_messages',
                label: 'Anthropic',
                apiFormat: 'anthropic_messages',
                baseUrl: 'https://api.deepseek.com/anthropic',
              },
            ],
          },
        },
      },
    ],
  });

  assert.equal(getGatewayProviderProfilesForTool('gemini')[0]?.id, 'deepseek');

  const selection = inferGatewayProviderEndpointSelection({
    tool: 'gemini',
    meta: {
      gatewayProfile: toGatewayProviderProfileReference('gemini', 'deepseek', 'anthropic_messages'),
      providerType: 'deepseek',
      apiFormat: 'openai_chat',
    },
    providerType: 'deepseek',
    apiFormat: 'openai_chat',
  });

  assert.deepEqual(selection, {
    providerProfileId: 'deepseek',
    providerEndpointId: 'anthropic_messages',
  });
  assert.equal(
    findGatewayProviderEndpoint(selection.providerProfileId, 'gemini', selection.providerEndpointId)?.baseUrl,
    'https://api.deepseek.com/anthropic',
  );
  assert.equal(
    findGatewayProviderEndpointByReference(
      toGatewayProviderProfileReference('gemini', 'deepseek', 'anthropic_messages'),
      'gemini',
    )?.endpoint.id,
    'anthropic_messages',
  );
  assert.equal(
    getGatewayProviderApiFormatFromMeta(
      { gatewayProfile: { tool: 'gemini', profileId: 'deepseek', endpointId: 'anthropic_messages' } },
      'gemini',
    ),
    'anthropic_messages',
  );
});

test('legacy endpoint inference only upgrades unique provider type and api format matches', () => {
  updateGatewayProviderProfiles({
    schemaVersion: 1,
    profiles: [
      {
        id: 'deepseek',
        providerType: 'deepseek',
        label: 'DeepSeek',
        tools: {
          gemini: {
            defaultEndpointId: 'openai_chat',
            endpoints: [
              {
                id: 'openai_chat',
                label: 'OpenAI Chat',
                apiFormat: 'openai_chat',
                baseUrl: 'https://api.deepseek.com',
              },
              {
                id: 'anthropic_messages',
                label: 'Anthropic',
                apiFormat: 'anthropic_messages',
                baseUrl: 'https://api.deepseek.com/anthropic',
              },
            ],
          },
        },
      },
    ],
  });

  const selection = inferUniqueGatewayProviderEndpointSelection({
    tool: 'gemini',
    providerType: 'deepseek',
    apiFormat: 'anthropic',
  });

  assert.deepEqual(selection, {
    providerProfileId: 'deepseek',
    providerEndpointId: 'anthropic_messages',
  });
});

test('legacy endpoint inference returns custom for duplicate provider type and api format matches', () => {
  updateGatewayProviderProfiles({
    schemaVersion: 1,
    profiles: [
      {
        id: 'deepseek',
        providerType: 'deepseek',
        label: 'DeepSeek',
        tools: {
          gemini: {
            defaultEndpointId: 'openai_chat',
            endpoints: [
              {
                id: 'openai_chat',
                label: 'OpenAI Chat',
                apiFormat: 'openai_chat',
                baseUrl: 'https://api.deepseek.com',
              },
              {
                id: 'anthropic_messages',
                label: 'Anthropic',
                apiFormat: 'anthropic_messages',
                baseUrl: 'https://api.deepseek.com/anthropic',
              },
            ],
          },
        },
      },
      {
        id: 'deepseek_global',
        providerType: 'deepseek',
        label: 'DeepSeek Global',
        tools: {
          gemini: {
            defaultEndpointId: 'openai_chat',
            endpoints: [
              {
                id: 'openai_chat',
                label: 'OpenAI Chat',
                apiFormat: 'openai_chat',
                baseUrl: 'https://api.deepseek.global',
              },
            ],
          },
        },
      },
    ],
  });

  const selection = inferUniqueGatewayProviderEndpointSelection({
    tool: 'gemini',
    providerType: 'deepseek',
    apiFormat: 'openai_chat',
  });

  assert.deepEqual(selection, {
    providerProfileId: CUSTOM_PROVIDER_PROFILE_ID,
    providerEndpointId: undefined,
  });
});

test('gateway profile reference meta merge stores reference and drops derived snapshots', () => {
  const merged = mergeGatewayProfileReferenceIntoMeta(
    {
      providerType: 'deepseek',
      apiFormat: 'openai_chat',
      apiKeyField: 'x-api-key',
      reasoningField: 'reasoning',
      defaultMaxTokens: 4096,
      codexChatReasoning: { supportsEffort: true },
      imageInputPolicy: 'strip',
      costMultiplier: '1.2',
      promptCacheKey: 'session',
    },
    toGatewayProviderProfileReference('codex', 'deepseek', 'openai_chat'),
  );

  assert.deepEqual(merged, {
    gatewayProfile: {
      tool: 'codex',
      profileId: 'deepseek',
      endpointId: 'openai_chat',
    },
    costMultiplier: '1.2',
    promptCacheKey: 'session',
  });
});

test('custom gateway profile meta merge preserves manually managed compatibility fields', () => {
  const merged = mergeGatewayProfileReferenceIntoMeta(
    {
      gatewayProfile: toGatewayProviderProfileReference('codex', 'deepseek', 'openai_chat'),
      gateway_profile: {
        tool: 'codex',
        profile_id: 'deepseek',
        endpoint_id: 'openai_chat',
      },
      providerType: 'deepseek',
      provider_type: 'deepseek_legacy',
      apiFormat: 'openai_chat',
      api_format: 'openai_chat_legacy',
      apiKeyField: 'x-api-key',
      api_key_field: 'legacy-key',
      reasoningField: 'reasoning',
      reasoning_field: 'legacy-reasoning',
      defaultMaxTokens: 4096,
      default_max_tokens: 8192,
      codexChatReasoning: { supportsEffort: true },
      codex_chat_reasoning: { supportsThinking: true },
      imageInputPolicy: 'strip',
      image_input_policy: 'text_only',
      textOnlyModels: ['deepseek-chat'],
      text_only_models: ['deepseek-legacy'],
      imageCapableModels: ['deepseek-vision'],
      image_capable_models: ['deepseek-legacy-vision'],
      allowTextOnlyModelHeuristic: true,
      allow_text_only_model_heuristic: true,
      costMultiplier: '1.2',
      promptCacheKey: 'session',
    },
    undefined,
    'anthropic_messages',
  );

  assert.deepEqual(merged, {
    providerType: 'deepseek',
    provider_type: 'deepseek_legacy',
    apiFormat: 'anthropic_messages',
    apiKeyField: 'x-api-key',
    api_key_field: 'legacy-key',
    reasoningField: 'reasoning',
    reasoning_field: 'legacy-reasoning',
    defaultMaxTokens: 4096,
    default_max_tokens: 8192,
    codexChatReasoning: { supportsEffort: true },
    codex_chat_reasoning: { supportsThinking: true },
    imageInputPolicy: 'strip',
    image_input_policy: 'text_only',
    textOnlyModels: ['deepseek-chat'],
    text_only_models: ['deepseek-legacy'],
    imageCapableModels: ['deepseek-vision'],
    image_capable_models: ['deepseek-legacy-vision'],
    allowTextOnlyModelHeuristic: true,
    allow_text_only_model_heuristic: true,
    costMultiplier: '1.2',
    promptCacheKey: 'session',
  });
});
