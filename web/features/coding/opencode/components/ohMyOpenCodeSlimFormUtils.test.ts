import test from 'node:test';
import assert from 'node:assert/strict';

import { buildSlimAgentsFromFormValues } from './ohMyOpenCodeSlimFormUtils.ts';

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
