/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildSlimCouncilConfig,
  mergeCouncilAgentIntoAgents,
  parseSlimCouncilFormValues,
} from '../../../../../features/coding/opencode/components/ohMyOpenCodeSlimCouncilUtils.ts';

const t = (key: string, options?: Record<string, unknown>) => {
  if (!options) {
    return key;
  }
  return `${key}:${JSON.stringify(options)}`;
};

test('parseSlimCouncilFormValues prefers agents.council over legacy council.master', () => {
  const parsed = parseSlimCouncilFormValues({
    council: {
      master: {
        model: 'openai/legacy-master',
        prompt: 'legacy',
      },
      default_preset: 'default',
      timeout: 120000,
      master_timeout: 300000,
      master_fallback: ['openai/fallback'],
      councillor_execution_mode: 'serial',
      councillor_retries: 2,
      presets: {
        default: {
          master: {
            model: 'openai/preset-master',
          },
          alpha: {
            model: 'openai/gpt-5.6-luna',
            prompt: 'focus on bugs',
          },
        },
      },
    },
    agents: {
      council: {
        model: 'openai/gpt-5.6',
        variant: 'high',
        prompt: 'synthesize carefully',
        temperature: 0.2,
      },
    },
  });

  assert.equal(parsed.councilEnabled, true);
  assert.deepEqual(parsed.councilAgent, {
    model: 'openai/gpt-5.6',
    variant: 'high',
    prompt: 'synthesize carefully',
  });
  assert.equal(parsed.councilDefaultPreset, 'default');
  assert.equal(parsed.councilCouncillorsTimeout, 120000);
  assert.equal(parsed.councilExecutionMode, 'serial');
  assert.equal(parsed.councilRetries, 2);
  assert.deepEqual(parsed.councilPresets, [
    {
      name: 'default',
      councillors: [
        {
          name: 'alpha',
          model: 'openai/gpt-5.6-luna',
          variant: undefined,
          prompt: 'focus on bugs',
        },
      ],
    },
  ]);
});

test('parseSlimCouncilFormValues migrates legacy council.master when agents.council is missing', () => {
  const parsed = parseSlimCouncilFormValues({
    council: {
      master: {
        model: 'openai/legacy-master',
        variant: 'medium',
        prompt: 'legacy synthesizer',
      },
      presets: {
        default: {
          alpha: { model: 'openai/gpt-5.6-luna' },
        },
      },
    },
  });

  assert.deepEqual(parsed.councilAgent, {
    model: 'openai/legacy-master',
    variant: 'medium',
    prompt: 'legacy synthesizer',
  });
});

test('parseSlimCouncilFormValues supports legacy nested councillors objects', () => {
  const parsed = parseSlimCouncilFormValues({
    council: {
      presets: {
        review: {
          councillors: {
            reviewer: {
              model: 'openai/gpt-5.6',
            },
          },
        },
      },
    },
  });

  assert.deepEqual(parsed.councilPresets, [
    {
      name: 'review',
      councillors: [
        {
          name: 'reviewer',
          model: 'openai/gpt-5.6',
          variant: undefined,
          prompt: undefined,
        },
      ],
    },
  ]);
});

test('buildSlimCouncilConfig writes agents.council payload and strips master fields', () => {
  const result = buildSlimCouncilConfig(
    {
      councilEnabled: true,
      councilAgent: {
        model: 'openai/gpt-5.6',
        variant: 'high',
        prompt: 'synthesize carefully',
      },
      councilDefaultPreset: 'default',
      councilCouncillorsTimeout: 180000,
      councilExecutionMode: 'parallel',
      councilRetries: 3,
      councilPresets: [
        {
          name: 'default',
          councillors: [
            {
              name: 'alpha',
              model: 'openai/gpt-5.6-luna',
              prompt: 'focus on bugs',
            },
          ],
        },
      ],
    },
    t,
  );

  assert.equal(result.errorMessage, undefined);
  assert.deepEqual(result.councilAgent, {
    model: 'openai/gpt-5.6',
    variant: 'high',
    prompt: 'synthesize carefully',
  });
  assert.deepEqual(result.council, {
    default_preset: 'default',
    timeout: 180000,
    councillor_execution_mode: 'parallel',
    councillor_retries: 3,
    presets: {
      default: {
        alpha: {
          model: 'openai/gpt-5.6-luna',
          prompt: 'focus on bugs',
        },
      },
    },
  });
  assert.equal(Object.prototype.hasOwnProperty.call(result.council, 'master'), false);
  assert.equal(Object.prototype.hasOwnProperty.call(result.council, 'master_timeout'), false);
  assert.equal(Object.prototype.hasOwnProperty.call(result.council, 'master_fallback'), false);
});

test('buildSlimCouncilConfig requires synthesizer model when council is enabled', () => {
  const result = buildSlimCouncilConfig(
    {
      councilEnabled: true,
      councilAgent: {
        prompt: 'missing model',
      },
      councilPresets: [
        {
          name: 'default',
          councillors: [{ name: 'alpha', model: 'openai/gpt-5.6' }],
        },
      ],
    },
    t,
  );

  assert.equal(result.council, null);
  assert.equal(result.errorMessage, 'opencode.ohMyOpenCodeSlim.councilAgentModelRequired');
});

test('mergeCouncilAgentIntoAgents preserves unmanaged agents.council fields', () => {
  const merged = mergeCouncilAgentIntoAgents(
    {
      orchestrator: { model: 'openai/gpt-5.6' },
    },
    {
      model: 'openai/gpt-5.6',
      variant: 'high',
      prompt: 'synthesize carefully',
    },
    {
      model: 'old-model',
      temperature: 0.2,
      skills: ['review'],
    },
  );

  assert.deepEqual(merged, {
    orchestrator: { model: 'openai/gpt-5.6' },
    council: {
      temperature: 0.2,
      skills: ['review'],
      model: 'openai/gpt-5.6',
      variant: 'high',
      prompt: 'synthesize carefully',
    },
  });
});
