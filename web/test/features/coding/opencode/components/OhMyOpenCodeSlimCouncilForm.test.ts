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
    modelSourceValue: ['openai/gpt-5.6', 'openai/fallback'],
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
    modelSourceValue: 'openai/legacy-master',
  });
});

test('agents.council alone stays enabled and preserves its full model chain', () => {
  const modelChain = [
    'openai/primary',
    'openai/fallback-a',
    { id: 'openai/fallback-b', variant: 'high', timeout: 120000 },
  ];
  const parsed = parseSlimCouncilFormValues({
    agents: {
      council: {
        model: modelChain,
        prompt: 'synthesize',
      },
    },
  });

  assert.equal(parsed.councilEnabled, true);
  assert.equal(parsed.councilAgent?.model, 'openai/primary');
  const result = buildSlimCouncilConfig(parsed, t);

  assert.equal(result.errorMessage, undefined);
  assert.equal(result.council, null);
  assert.deepEqual(result.councilAgent, {
    model: modelChain,
    prompt: 'synthesize',
  });
});

test('editing the council primary model preserves fallback entries and object fields', () => {
  const parsed = parseSlimCouncilFormValues({
    agents: {
      council: {
        model: [
          { id: 'openai/old-primary', variant: 'medium', custom: true },
          'openai/fallback-a',
          { id: 'openai/fallback-b', variant: 'high', custom: 'fallback' },
        ],
      },
    },
  });
  if (!parsed.councilAgent) {
    throw new Error('expected parsed council agent');
  }
  parsed.councilAgent.model = 'openai/new-primary';

  const result = buildSlimCouncilConfig(parsed, t);
  assert.deepEqual(result.councilAgent?.model, [
    { id: 'openai/new-primary', variant: 'medium', custom: true },
    'openai/fallback-a',
    { id: 'openai/fallback-b', variant: 'high', custom: 'fallback' },
  ]);
});

test('legacy council.master_fallback is merged into the council agent model chain', () => {
  const parsed = parseSlimCouncilFormValues({
    council: {
      master: { model: 'openai/primary' },
      master_fallback: [
        'openai/fallback-a',
        { id: 'openai/fallback-b', variant: 'high' },
        'openai/primary',
      ],
    },
  });
  const result = buildSlimCouncilConfig(parsed, t);

  assert.deepEqual(result.councilAgent?.model, [
    'openai/primary',
    'openai/fallback-a',
    { id: 'openai/fallback-b', variant: 'high' },
  ]);
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

test('mergeCouncilAgentIntoAgents keeps an existing fallback chain when only the primary changes', () => {
  const merged = mergeCouncilAgentIntoAgents(
    undefined,
    { model: 'openai/new-primary' },
    {
      model: [
        'openai/old-primary',
        'openai/fallback-a',
        { id: 'openai/fallback-b', variant: 'high' },
      ],
    },
  );

  assert.deepEqual((merged?.council as Record<string, unknown>).model, [
    'openai/new-primary',
    'openai/fallback-a',
    { id: 'openai/fallback-b', variant: 'high' },
  ]);
});
