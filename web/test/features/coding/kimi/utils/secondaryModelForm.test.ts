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
  assert.ok(validateKimiSecondaryModelConfig({
    defaultModel: 'a/b',
    models: ['a/c'],
    force: true,
  }));
  // force without a default is rejected.
  assert.ok(validateKimiSecondaryModelConfig({ defaultModel: '', models: [], force: true }));
  // A pool without a default is rejected.
  assert.ok(validateKimiSecondaryModelConfig({ defaultModel: '', models: ['a/c'], force: false }));
  // "primary" is reserved.
  assert.ok(validateKimiSecondaryModelConfig({
    defaultModel: 'a/b',
    models: ['primary'],
    force: false,
  }));
  // A plain default-only config is valid.
  assert.equal(
    validateKimiSecondaryModelConfig({ defaultModel: 'a/b', models: [], force: false }),
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
