import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  OMP_BUILTIN_AGENT_DEFAULTS,
  OMP_BUILTIN_AGENT_NAMES,
  OMP_RESERVED_AGENT_NAMES,
  buildOmpBuiltinAgentConfig,
  getOmpAgentFieldBool,
  getOmpAgentFieldString,
  getOmpAgentFieldStringArray,
  getOmpAgentModelDisplay,
  hasExplicitOmpAgentConfig,
  isValidOmpAgentFileName,
  ompAgentConfigToDraft,
  ompAgentDraftToConfig,
} from '../../../../../features/coding/oh_my_pi/utils/ompAgentsUtils.ts';
import type { OmpAgentConfig } from '../../../../../types/ohMyPi.ts';

test('builtin agent names cover bundled task agents', () => {
  for (const name of ['task', 'sonic', 'scout', 'reviewer', 'security-reviewer']) {
    assert.equal(OMP_BUILTIN_AGENT_NAMES.has(name), true);
    assert.ok(OMP_BUILTIN_AGENT_DEFAULTS[name]);
  }
});

test('builtin defaults: task/sonic carry model+thinkingLevel, others are bare', () => {
  assert.equal(OMP_BUILTIN_AGENT_DEFAULTS.task.model, '@task');
  assert.equal(OMP_BUILTIN_AGENT_DEFAULTS.sonic.model, '@smol');
  assert.equal(OMP_BUILTIN_AGENT_DEFAULTS.scout.model, undefined);
  assert.equal(OMP_BUILTIN_AGENT_DEFAULTS.reviewer.model, undefined);
});

test('builtin config builder fills name/model/thinkingLevel', () => {
  const task = buildOmpBuiltinAgentConfig('task');
  assert.equal(task.name, 'task');
  assert.equal(task.model, '@task');
  assert.equal(task.thinkingLevel, 'auto');
  const scout = buildOmpBuiltinAgentConfig('scout');
  assert.equal(scout.name, 'scout');
  assert.equal(scout.model, undefined);
  // 未知名返回空对象(调用方按普通配置处理,不会落到内置分支)。
  assert.deepEqual(buildOmpBuiltinAgentConfig('not-builtin'), {});
});

test('reserved names include main and sub', () => {
  assert.equal(OMP_RESERVED_AGENT_NAMES.has('main'), true);
  assert.equal(OMP_RESERVED_AGENT_NAMES.has('sub'), true);
});

test('valid agent file names pass and invalid ones fail', () => {
  for (const name of ['reviewer', 'security-reviewer', 'my_agent', 'a.b']) {
    assert.equal(isValidOmpAgentFileName(name), true);
  }
  for (const name of ['', '.', '..', '../evil', 'a/b', 'a b', '中文']) {
    assert.equal(isValidOmpAgentFileName(name), false);
  }
});

test('model display extracts primary and fallback count from array model', () => {
  const agent: OmpAgentConfig = { model: ['@smol', 'openai/gpt-5-mini'] };
  assert.deepEqual(getOmpAgentModelDisplay(agent), {
    primaryModel: '@smol',
    fallbackCount: 1,
  });
});

test('model display handles string model and missing model', () => {
  assert.deepEqual(getOmpAgentModelDisplay({ model: 'openai/gpt-5' }), {
    primaryModel: 'openai/gpt-5',
    fallbackCount: 0,
  });
  assert.deepEqual(getOmpAgentModelDisplay(undefined), {
    primaryModel: undefined,
    fallbackCount: 0,
  });
  assert.deepEqual(getOmpAgentModelDisplay({ model: 42 }), {
    primaryModel: undefined,
    fallbackCount: 0,
  });
});

test('string array field parses csv string and array', () => {
  assert.deepEqual(getOmpAgentFieldStringArray({ tools: 'read, grep' }, 'tools'), ['read', 'grep']);
  assert.deepEqual(getOmpAgentFieldStringArray({ tools: ['read', 'grep'] }, 'tools'), [
    'read',
    'grep',
  ]);
  assert.deepEqual(getOmpAgentFieldStringArray({}, 'tools'), []);
  assert.deepEqual(getOmpAgentFieldStringArray({ tools: [1, 2] }, 'tools'), []);
});

test('string and bool field getters handle missing values', () => {
  assert.equal(getOmpAgentFieldString({ description: 'x' }, 'description'), 'x');
  assert.equal(getOmpAgentFieldString({}, 'description'), undefined);
  assert.equal(getOmpAgentFieldBool({ blocking: true }, 'blocking'), true);
  assert.equal(getOmpAgentFieldBool({ blocking: 'yes' }, 'blocking'), undefined);
});

test('config to draft maps frontmatter fields', () => {
  const agent: OmpAgentConfig = {
    name: 'reviewer',
    description: 'Reviews',
    model: '@smol',
    thinkingLevel: 'high',
    tools: 'read, grep',
    blocking: true,
    readSummarize: false,
    prewalk: '@smol',
    advisor: true,
    spawns: '*',
    autoloadSkills: ['git'],
    prompt: 'Review carefully',
    unknownField: { keep: true },
  };
  const draft = ompAgentConfigToDraft(agent);
  assert.equal(draft.name, 'reviewer');
  assert.equal(draft.description, 'Reviews');
  assert.equal(draft.model, '@smol');
  assert.equal(draft.thinkingLevel, 'high');
  assert.deepEqual(draft.tools, ['read', 'grep']);
  assert.equal(draft.blocking, true);
  assert.equal(draft.readSummarize, false);
  assert.equal(draft.prewalk, '@smol');
  assert.equal(draft.advisor, 'true');
  assert.equal(draft.spawns, '*');
  assert.deepEqual(draft.autoloadSkills, ['git']);
  assert.equal(draft.prompt, 'Review carefully');
});

test('draft to config preserves unknown fields and removes cleared fields', () => {
  const existing: OmpAgentConfig = {
    name: 'reviewer',
    unknownField: { keep: true },
    tools: ['read'],
  };
  const next = ompAgentDraftToConfig(
    {
      name: 'reviewer',
      description: 'Updated',
      model: undefined,
      tools: [],
      autoloadSkills: [],
    },
    existing,
  );
  assert.equal(next.description, 'Updated');
  assert.equal(next.model, undefined);
  assert.equal(next.tools, undefined);
  assert.deepEqual(next.unknownField, { keep: true });
});

test('draft to config converts prewalk/advisor booleans', () => {
  const next = ompAgentDraftToConfig(
    {
      name: 'x',
      tools: [],
      autoloadSkills: [],
      prewalk: 'true',
      advisor: '@smol:high',
    },
    {},
  );
  assert.equal(next.name, 'x');
  assert.equal(next.prewalk, true);
  assert.equal(next.advisor, '@smol:high');
});

test('round trip config -> draft -> config keeps semantics', () => {
  const agent: OmpAgentConfig = {
    name: 'sonic',
    description: 'Mechanical updates',
    model: '@smol',
    tools: ['edit', 'yield'],
    custom: { flag: 1 },
  };
  const draft = ompAgentConfigToDraft(agent);
  const rebuilt = ompAgentDraftToConfig(draft, { custom: { flag: 1 } });
  assert.equal(rebuilt.name, 'sonic');
  assert.equal(rebuilt.description, 'Mechanical updates');
  assert.equal(rebuilt.model, '@smol');
  assert.deepEqual(rebuilt.tools, ['edit', 'yield']);
  assert.deepEqual(rebuilt.custom, { flag: 1 });
});

test('hasExplicitConfig: pure builtin defaults are NOT explicit', () => {
  // 未覆盖任何字段的内置 agent(buildOmpBuiltinAgentConfig 的产物)应视为未覆盖。
  for (const name of Object.keys(OMP_BUILTIN_AGENT_DEFAULTS)) {
    const config = buildOmpBuiltinAgentConfig(name) as OmpAgentConfig;
    assert.equal(
      hasExplicitOmpAgentConfig(config),
      false,
      `${name} default should not be explicit`,
    );
  }
});

test('hasExplicitConfig: CLI-level changes make it explicit', () => {
  const taskWithTools = {
    ...(buildOmpBuiltinAgentConfig('task') as OmpAgentConfig),
    tools: ['read'],
  };
  assert.equal(hasExplicitOmpAgentConfig(taskWithTools), true);

  const taskOtherModel = {
    ...(buildOmpBuiltinAgentConfig('task') as OmpAgentConfig),
    model: 'openai/gpt-5',
  };
  assert.equal(hasExplicitOmpAgentConfig(taskOtherModel), true);

  const scoutWithPrompt = {
    name: 'scout',
    prompt: 'Just read',
  };
  assert.equal(hasExplicitOmpAgentConfig(scoutWithPrompt), true);
});

test('hasExplicitConfig: custom (non-builtin) is always explicit', () => {
  assert.equal(hasExplicitOmpAgentConfig({ name: 'custom', description: 'x' }), true);
  assert.equal(hasExplicitOmpAgentConfig({ name: 'custom' }), true);
});

test('core model roles cover 9 native OMP roles', async () => {
  const { OMP_CORE_MODEL_ROLES, OMP_CORE_MODEL_ROLE_KEYS } = await import(
    '../../../../../features/coding/oh_my_pi/utils/ompAgentsUtils.ts'
  );
  assert.equal(OMP_CORE_MODEL_ROLES.length, 9);
  for (const key of [
    'default',
    'plan',
    'task',
    'advisor',
    'commit',
    'tiny',
    'smol',
    'slow',
    'vision',
  ]) {
    assert.equal(OMP_CORE_MODEL_ROLE_KEYS.has(key), true);
  }
});

test('parse and format OmpModelRoleEntry', async () => {
  const { parseOmpModelRoleEntry, formatOmpModelRoleEntry } = await import(
    '../../../../../features/coding/oh_my_pi/utils/ompAgentsUtils.ts'
  );
  // String with thinking suffix
  assert.deepEqual(parseOmpModelRoleEntry('anthropic/claude-sonnet-4-6:high'), {
    model: 'anthropic/claude-sonnet-4-6',
    thinkingLevel: 'high',
  });
  // Bare model string
  assert.deepEqual(parseOmpModelRoleEntry('openai/gpt-5'), {
    model: 'openai/gpt-5',
  });
  // Object input
  assert.deepEqual(
    parseOmpModelRoleEntry({ model: 'anthropic/claude-opus-4-6', thinkingLevel: 'auto' }),
    {
      model: 'anthropic/claude-opus-4-6',
      thinkingLevel: 'auto',
    },
  );
  // Empty or invalid input
  assert.deepEqual(parseOmpModelRoleEntry(undefined), {});

  // Format entry
  assert.deepEqual(
    formatOmpModelRoleEntry({ model: 'anthropic/claude-sonnet-4-6', thinkingLevel: 'high' }),
    {
      model: 'anthropic/claude-sonnet-4-6',
      thinkingLevel: 'high',
    },
  );
  assert.equal(formatOmpModelRoleEntry({ model: '' }), undefined);
  assert.equal(formatOmpModelRoleEntry(undefined), undefined);
});

test('getOmpThinkingOptionsForModel adapts strictly to provider model settings', async () => {
  const { getOmpThinkingOptionsForModel } = await import(
    '../../../../../features/coding/oh_my_pi/utils/ompAgentsUtils.ts'
  );

  const mockProviders = [
    {
      providerKey: 'anthropic',
      displayName: 'Anthropic',
      sources: ['models_yml'],
      categories: ['api_key'],
      credentialKind: 'api_key',
      runtimeFiles: ['models.yml'],
      isBuiltin: false,
      isOverride: false,
      isDefault: false,
      modelIds: ['claude-3-7-sonnet', 'claude-3-5-haiku'],
      modelsProvider: {
        models: [
          {
            id: 'claude-3-7-sonnet',
            reasoning: true,
            thinking: {
              efforts: ['low', 'high'],
              defaultLevel: 'high',
            },
          },
          {
            id: 'claude-3-5-haiku',
            reasoning: false,
          },
        ],
      },
    },
    {
      providerKey: 'openai',
      displayName: 'OpenAI',
      sources: ['models_yml'],
      categories: ['api_key'],
      credentialKind: 'api_key',
      runtimeFiles: ['models.yml'],
      isBuiltin: false,
      isOverride: false,
      isDefault: false,
      modelIds: ['gpt-5'],
      modelsProvider: {
        models: [
          {
            id: 'gpt-5',
            reasoning: true,
            thinking: {
              efforts: ['minimal', 'low', 'medium', 'high', 'xhigh'],
              defaultLevel: 'medium',
            },
          },
        ],
      },
    },
  ];

  // 1. 未选择模型
  const noModel = getOmpThinkingOptionsForModel('', mockProviders as never);
  assert.equal(noModel.supported, false);
  assert.equal(noModel.placeholderKey, 'ohMyPi.subagents.selectModelFirst');
  assert.deepEqual(noModel.options, []);

  // 2. 角色别名
  const aliasModel = getOmpThinkingOptionsForModel('@task', mockProviders as never);
  assert.equal(aliasModel.supported, true);
  assert.equal(aliasModel.isGeneric, true);
  assert.ok(aliasModel.options.some((o) => o.value === 'auto'));

  // 3. 严格贴合模型思考等级(claude-3-7-sonnet 仅声明了 low 与 high)
  const sonnetResult = getOmpThinkingOptionsForModel(
    'anthropic/claude-3-7-sonnet',
    mockProviders as never,
  );
  assert.equal(sonnetResult.supported, true);
  assert.equal(sonnetResult.defaultLevel, 'high');
  // options 包含 off, low, high, auto; 决不能包含 medium 或 minimal
  const sonnetLevels = sonnetResult.options.map((o) => o.value);
  assert.deepEqual(sonnetLevels, ['off', 'low', 'high', 'auto']);

  // 4. 不支持思考的模型(claude-3-5-haiku, reasoning: false)
  const haikuResult = getOmpThinkingOptionsForModel(
    'anthropic/claude-3-5-haiku',
    mockProviders as never,
  );
  assert.equal(haikuResult.supported, false);
  assert.equal(haikuResult.placeholderKey, 'ohMyPi.subagents.thinkingNotSupported');
  assert.deepEqual(haikuResult.options, []);

  // 5. 裸 modelId 匹配(未带 provider 前缀但匹配到 gpt-5)
  const gpt5Result = getOmpThinkingOptionsForModel('gpt-5', mockProviders as never);
  assert.equal(gpt5Result.supported, true);
  assert.equal(gpt5Result.defaultLevel, 'medium');
  const gpt5Levels = gpt5Result.options.map((o) => o.value);
  assert.ok(gpt5Levels.includes('xhigh'));

  // 6. 自定义外部未知模型
  const customModel = getOmpThinkingOptionsForModel('custom/unknown-model', mockProviders as never);
  assert.equal(customModel.supported, true);
  assert.equal(customModel.isGeneric, true);
});