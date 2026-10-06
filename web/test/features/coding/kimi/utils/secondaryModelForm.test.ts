import test from 'node:test';
import assert from 'node:assert/strict';

import {
  applyKimiSecondaryModelToml,
  buildKimiSecondaryModelToml,
  emptyKimiSecondaryModelConfig,
  isKimiSecondaryModelConfigEmpty,
  parseKimiSecondaryModelConfig,
  validateKimiSecondaryModelConfig,
} from '../../../../../features/coding/kimi/utils/secondaryModelForm.ts';

test('parse returns the empty config when the section is absent', () => {
  assert.deepEqual(parseKimiSecondaryModelConfig(''), emptyKimiSecondaryModelConfig());
  assert.deepEqual(
    parseKimiSecondaryModelConfig('default_model = "x/y"\n'),
    emptyKimiSecondaryModelConfig(),
  );
});

test('parse returns the empty config for malformed TOML instead of throwing', () => {
  assert.deepEqual(parseKimiSecondaryModelConfig('[broken'), emptyKimiSecondaryModelConfig());
});

test('parse reads default_model, the pool keys and force', () => {
  const parsed = parseKimiSecondaryModelConfig(`
[secondary_model]
default_model = "moonshotai/kimi-k2.6"

[secondary_model.models]
"moonshotai/kimi-k3" = ""
"moonshotai/kimi-k2.6" = ""
`);
  assert.equal(parsed.defaultModel, 'moonshotai/kimi-k2.6');
  assert.deepEqual(parsed.models, ['moonshotai/kimi-k3', 'moonshotai/kimi-k2.6']);
  assert.equal(parsed.force, false);
});

test('parse honors the legacy v1 model key as the default', () => {
  const parsed = parseKimiSecondaryModelConfig('[secondary_model]\nmodel = "legacy/alias"\n');
  assert.equal(parsed.defaultModel, 'legacy/alias');
});

test('validate mirrors the CLI schema rules', () => {
  // force + pool are mutually exclusive.
  assert.equal(
    validateKimiSecondaryModelConfig({ defaultModel: 'a/b', models: ['a/c'], force: true }),
    'forceWithModels',
  );
  // force without a default is rejected.
  assert.equal(
    validateKimiSecondaryModelConfig({ defaultModel: '', models: [], force: true }),
    'forceWithoutDefault',
  );
  // A pool without a default is rejected.
  assert.equal(
    validateKimiSecondaryModelConfig({ defaultModel: '', models: ['a/c'], force: false }),
    'modelsWithoutDefault',
  );
  // "primary" is reserved.
  assert.equal(
    validateKimiSecondaryModelConfig({ defaultModel: 'a/b', models: ['primary'], force: false }),
    'reservedPrimaryKey',
  );
  // A plain default-only config is valid.
  assert.equal(
    validateKimiSecondaryModelConfig({ defaultModel: 'a/b', models: [], force: false }),
    null,
  );
});

test('validate rejects aliases the applied catalog cannot resolve', () => {
  // A pool key with no matching [models.<key>] makes the CLI refuse to start.
  assert.equal(
    validateKimiSecondaryModelConfig(
      { defaultModel: 'a/b', models: ['ghost/missing'], force: false },
      ['a/b'],
    ),
    'unresolvableAlias',
  );
  // The default itself must resolve too.
  assert.equal(
    validateKimiSecondaryModelConfig(
      { defaultModel: 'ghost/missing', models: [], force: false },
      ['a/b'],
    ),
    'unresolvableAlias',
  );
  // "primary" is legal as a value even though it is not a catalog key.
  assert.equal(
    validateKimiSecondaryModelConfig(
      { defaultModel: 'primary', models: [], force: false },
      ['a/b'],
    ),
    null,
  );
  // Everything resolvable passes.
  assert.equal(
    validateKimiSecondaryModelConfig(
      { defaultModel: 'a/b', models: ['a/c'], force: false },
      ['a/b', 'a/c'],
    ),
    null,
  );
  // Without a known catalog the resolvability check is skipped rather than
  // blocking a save on missing data.
  assert.equal(
    validateKimiSecondaryModelConfig(
      { defaultModel: 'a/b', models: ['ghost/missing'], force: false },
      undefined,
    ),
    null,
  );
});

test('build renders an empty string for an empty config so the section is dropped', () => {
  assert.equal(buildKimiSecondaryModelToml(emptyKimiSecondaryModelConfig()), '');
  assert.equal(
    buildKimiSecondaryModelToml({ defaultModel: '  ', models: [], force: false }),
    '',
  );
});

test('build writes default_model, force and the pool sub-table', () => {
  const toml = buildKimiSecondaryModelToml({
    defaultModel: 'moonshotai/kimi-k2.6',
    models: ['moonshotai/kimi-k3'],
    force: false,
  });
  assert.match(toml, /\[secondary_model\]/);
  assert.match(toml, /default_model = "moonshotai\/kimi-k2\.6"/);
  assert.match(toml, /\[secondary_model\.models\]/);
  assert.match(toml, /"moonshotai\/kimi-k3" = ""/);
  assert.doesNotMatch(toml, /force/);
});

test('apply replaces an existing section and leaves other lines intact', () => {
  const stored = `# user comment
default_model = "moonshotai/kimi-k3"
telemetry = false

[secondary_model]
default_model = "old/alias"

[secondary_model.models]
"old/alias" = ""

[loop_control]
max_attempts_per_step = 10
`;
  const result = applyKimiSecondaryModelToml(stored, {
    defaultModel: 'moonshotai/kimi-k2.6',
    models: ['moonshotai/kimi-k3'],
    force: false,
  });

  // Unrelated lines survive byte-for-byte, including the comment and the
  // trailing table that followed the removed section.
  assert.match(result, /# user comment/);
  assert.match(result, /default_model = "moonshotai\/kimi-k3"/);
  assert.match(result, /telemetry = false/);
  assert.match(result, /\[loop_control\]/);
  assert.match(result, /max_attempts_per_step = 10/);
  // The old pool is gone, the new one is present.
  assert.doesNotMatch(result, /old\/alias/);
  assert.match(result, /"moonshotai\/kimi-k3" = ""/);
});

test('apply drops the section entirely when the config is emptied', () => {
  const stored = `default_model = "moonshotai/kimi-k3"

[secondary_model]
default_model = "old/alias"
force = true
`;
  const result = applyKimiSecondaryModelToml(stored, emptyKimiSecondaryModelConfig());

  assert.doesNotMatch(result, /\[secondary_model\]/);
  assert.doesNotMatch(result, /force/);
  assert.match(result, /default_model = "moonshotai\/kimi-k3"/);
});

test('apply recognizes every legal spelling of the section header', () => {
  const config = { defaultModel: 'new/alias', models: [], force: false };
  // A trailing comment, a quoted table name, and the dotted-key form are all
  // valid TOML; missing any of them would leave a duplicate table behind.
  const spellings = [
    '[secondary_model] # swarm pool\ndefault_model = "old/alias"\n',
    '["secondary_model"]\ndefault_model = "old/alias"\n',
    "['secondary_model']\ndefault_model = \"old/alias\"\n",
    'secondary_model.default_model = "old/alias"\n',
  ];

  for (const stored of spellings) {
    const result = applyKimiSecondaryModelToml(stored, config);
    assert.doesNotMatch(result, /old\/alias/, `stale value survived: ${stored}`);
    assert.equal(
      result.match(/\[secondary_model\]/g)?.length ?? 0,
      1,
      `expected exactly one section for: ${stored}`,
    );
  }
});

test('apply removes the section when only a trailing-comment header matched', () => {
  const stored = '[secondary_model] # swarm pool\ndefault_model = "old/alias"\n';
  const result = applyKimiSecondaryModelToml(stored, emptyKimiSecondaryModelConfig());

  assert.doesNotMatch(result, /\[secondary_model\]/);
  assert.doesNotMatch(result, /old\/alias/);
});

test('apply does not treat a section name inside a multi-line string as a header', () => {
  const stored = [
    'notes = """',
    '[secondary_model]',
    'not a real table',
    '"""',
    'default_model = "keep/me"',
    '',
  ].join('\n');

  const result = applyKimiSecondaryModelToml(stored, {
    defaultModel: 'new/alias',
    models: [],
    force: false,
  });

  // The string body survives intact and the real assignment is kept.
  assert.match(result, /not a real table/);
  assert.match(result, /keep\/me/);
  assert.match(result, /new\/alias/);
});

test('apply is idempotent across repeated saves', () => {
  const config = {
    defaultModel: 'moonshotai/kimi-k2.6',
    models: ['moonshotai/kimi-k3'],
    force: false,
  };
  const once = applyKimiSecondaryModelToml('default_model = "a/b"\n', config);
  const twice = applyKimiSecondaryModelToml(once, config);
  assert.equal(twice, once);
});

test('isKimiSecondaryModelConfigEmpty only ignores whitespace-only defaults', () => {
  assert.equal(isKimiSecondaryModelConfigEmpty({ defaultModel: '  ', models: [], force: false }), true);
  assert.equal(isKimiSecondaryModelConfigEmpty({ defaultModel: 'a/b', models: [], force: false }), false);
  assert.equal(isKimiSecondaryModelConfigEmpty({ defaultModel: '', models: ['a/b'], force: false }), false);
  assert.equal(isKimiSecondaryModelConfigEmpty({ defaultModel: '', models: [], force: true }), false);
});
