/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { buildSlimAgentsFromFormValues } from '../../../../../features/coding/opencode/components/ohMyOpenCodeSlimFormUtils.ts';

test('buildSlimAgentsFromFormValues preserves unmanaged agent fields while updating managed ones', () => {
  const result = buildSlimAgentsFromFormValues({
    builtInAgentKeys: ['orchestrator'],
    customAgents: ['reviewer'],
    formValues: {
      agent_orchestrator_model: 'gpt-5.4',
      agent_orchestrator_variant: 'fast',
      agent_reviewer_model: 'gpt-5.4-mini',
    },
    initialAgents: {
      orchestrator: {
        model: 'old-model',
        variant: 'old-variant',
        skills: ['plan', 'delegate'],
        temperature: 0.2,
      },
      reviewer: {
        skills: ['lint'],
      },
    },
  });

  assert.deepEqual(result, {
    orchestrator: {
      skills: ['plan', 'delegate'],
      temperature: 0.2,
      model: 'gpt-5.4',
      variant: 'fast',
    },
    reviewer: {
      skills: ['lint'],
      model: 'gpt-5.4-mini',
    },
  });
});

test('buildSlimAgentsFromFormValues saves advanced settings for built-in and custom agents', () => {
  const result = buildSlimAgentsFromFormValues({
    builtInAgentKeys: ['orchestrator'],
    customAgents: ['codex-delegator'],
    formValues: {
      agent_orchestrator_model: 'openai/GPT-5.5',
      'agent_codex-delegator_model': 'openai/GPT-5.4',
    },
    advancedSettings: {
      orchestrator: {
        prompt: 'Plan first',
        orchestratorPrompt: 'Coordinate specialists',
        displayName: 'Lead',
        skills: ['planning'],
        mcps: ['github'],
        options: {
          temperature: 0.2,
        },
      },
      'codex-delegator': {
        prompt: 'Delegate coding tasks',
        skills: ['delegate'],
      },
    },
  });

  assert.deepEqual(result, {
    orchestrator: {
      prompt: 'Plan first',
      orchestratorPrompt: 'Coordinate specialists',
      displayName: 'Lead',
      skills: ['planning'],
      mcps: ['github'],
      options: {
        temperature: 0.2,
      },
      model: 'openai/GPT-5.5',
    },
    'codex-delegator': {
      prompt: 'Delegate coding tasks',
      skills: ['delegate'],
      model: 'openai/GPT-5.4',
    },
  });
});

test('buildSlimAgentsFromFormValues keeps managed fields controlled by form values', () => {
  const result = buildSlimAgentsFromFormValues({
    builtInAgentKeys: ['orchestrator'],
    customAgents: [],
    formValues: {
      agent_orchestrator_model: 'openai/GPT-5.5',
      agent_orchestrator_variant: 'high',
      agent_orchestrator_fallback_models: ['openai/GPT-5.4'],
    },
    advancedSettings: {
      orchestrator: {
        model: 'advanced-model',
        variant: 'advanced-variant',
        fallback_models: ['advanced-fallback'],
        prompt: 'Keep this',
      },
    },
  });

  assert.deepEqual(result, {
    orchestrator: {
      prompt: 'Keep this',
      model: 'openai/GPT-5.5',
      variant: 'high',
    },
  });
});

test('buildSlimAgentsFromFormValues lets edited advanced settings replace initial unmanaged fields', () => {
  const result = buildSlimAgentsFromFormValues({
    builtInAgentKeys: ['orchestrator'],
    customAgents: [],
    formValues: {
      agent_orchestrator_model: 'openai/GPT-5.5',
    },
    initialAgents: {
      orchestrator: {
        model: 'old-model',
        prompt: 'old prompt',
        options: {
          temperature: 0.8,
        },
      },
    },
    advancedSettings: {
      orchestrator: {
        prompt: 'new prompt',
      },
    },
  });

  assert.deepEqual(result, {
    orchestrator: {
      prompt: 'new prompt',
      model: 'openai/GPT-5.5',
    },
  });
});

test('buildSlimAgentsFromFormValues only replaces initial unmanaged fields for edited agents', () => {
  const result = buildSlimAgentsFromFormValues({
    builtInAgentKeys: ['orchestrator', 'oracle'],
    customAgents: [],
    formValues: {
      agent_orchestrator_model: 'openai/GPT-5.5',
      agent_oracle_model: 'openai/GPT-5.4',
    },
    initialAgents: {
      orchestrator: {
        model: 'old-orchestrator',
        prompt: 'old orchestrator prompt',
      },
      oracle: {
        model: 'old-oracle',
        prompt: 'old oracle prompt',
        options: {
          temperature: 0.3,
        },
      },
    },
    advancedSettings: {
      orchestrator: {
        prompt: 'new orchestrator prompt',
      },
    },
  });

  assert.deepEqual(result, {
    orchestrator: {
      prompt: 'new orchestrator prompt',
      model: 'openai/GPT-5.5',
    },
    oracle: {
      prompt: 'old oracle prompt',
      options: {
        temperature: 0.3,
      },
      model: 'openai/GPT-5.4',
    },
  });
});

test('buildSlimAgentsFromFormValues removes initial unmanaged fields when advanced settings are cleared', () => {
  const result = buildSlimAgentsFromFormValues({
    builtInAgentKeys: ['orchestrator'],
    customAgents: [],
    formValues: {
      agent_orchestrator_model: 'openai/GPT-5.5',
    },
    initialAgents: {
      orchestrator: {
        model: 'old-model',
        prompt: 'old prompt',
      },
    },
    advancedSettings: {
      orchestrator: {},
    },
  });

  assert.deepEqual(result, {
    orchestrator: {
      model: 'openai/GPT-5.5',
    },
  });
});

test('buildSlimAgentsFromFormValues omits agent when managed and unmanaged fields are both empty', () => {
  const result = buildSlimAgentsFromFormValues({
    builtInAgentKeys: ['orchestrator'],
    customAgents: [],
    formValues: {},
    initialAgents: {
      orchestrator: {
        model: 'old-model',
        variant: 'old-variant',
      },
    },
  });

  assert.deepEqual(result, {});
});

test('buildSlimAgentsFromFormValues does not write legacy fallback_models for managed agent fields', () => {
  const result = buildSlimAgentsFromFormValues({
    builtInAgentKeys: ['oracle'],
    customAgents: [],
    formValues: {
      agent_oracle_model: 'gpt-5.4',
      agent_oracle_fallback_models: [' gpt-5.4-mini ', '', 'gpt-4.1'],
    },
    initialAgents: {
      oracle: {
        model: 'old-oracle',
        fallback_models: ['legacy-model'],
        temperature: 0.3,
      },
    },
  });

  assert.deepEqual(result, {
    oracle: {
      temperature: 0.3,
      model: 'gpt-5.4',
    },
  });
});

test('buildSlimAgentsFromFormValues removes legacy fallback_models when user clears managed fallback field', () => {
  const result = buildSlimAgentsFromFormValues({
    builtInAgentKeys: ['oracle'],
    customAgents: [],
    formValues: {
      agent_oracle_model: 'gpt-5.4',
      agent_oracle_fallback_models: [],
    },
    initialAgents: {
      oracle: {
        model: 'old-oracle',
        fallback_models: ['legacy-model'],
        skills: ['plan'],
      },
    },
  });

  assert.deepEqual(result, {
    oracle: {
      skills: ['plan'],
      model: 'gpt-5.4',
    },
  });
});

test('buildSlimAgentsFromFormValues does not preserve legacy fallback_models for custom agents', () => {
  const result = buildSlimAgentsFromFormValues({
    builtInAgentKeys: [],
    customAgents: ['reviewer'],
    formValues: {
      agent_reviewer_model: 'gpt-5.4-mini',
      agent_reviewer_fallback_models: ' gpt-4.1-mini ',
    },
    initialAgents: {
      reviewer: {
        tools: ['lint'],
      },
    },
  });

  assert.deepEqual(result, {
    reviewer: {
      tools: ['lint'],
      model: 'gpt-5.4-mini',
    },
  });
});
