import assert from 'node:assert/strict';
import test from 'node:test';

import {
  mergeOpenCodeAgentConfigs,
  replaceOpenCodeMarkdownAgentFrontmatter,
  replaceOpenCodeMarkdownAgentPrompt,
  resolveOpenCodeAgentConfigFieldSource,
  setOpenCodeMarkdownAgentFrontmatterField,
} from '../../../../../features/coding/opencode/utils/openCodeMarkdownAgent.ts';

const markdown = `---
description: Reviews code
permission:
  edit: deny
model: old/model
variant: low
---

Original prompt.
`;

test('updates model and variant without removing unknown frontmatter', () => {
  const withModel = setOpenCodeMarkdownAgentFrontmatterField(markdown, 'model', 'new/model');
  const withoutVariant = setOpenCodeMarkdownAgentFrontmatterField(withModel, 'variant', undefined);
  assert.match(withoutVariant, /permission:\n  edit: deny/);
  assert.match(withoutVariant, /model: "new\/model"/);
  assert.doesNotMatch(withoutVariant, /^variant:/m);
  assert.match(withoutVariant, /Original prompt\./);
});

test('replaces and clears YAML block scalar fields without leaving continuation lines', () => {
  for (const indicator of ['|', '>', '|-', '>+']) {
    const blockScalarMarkdown = `---\ndescription: Reviews code\nmodel: ${indicator}\n  provider/model-a\n\n  continued\nmode: all\n---\nPrompt`;
    const replaced = setOpenCodeMarkdownAgentFrontmatterField(
      blockScalarMarkdown,
      'model',
      'provider/model-b',
    );
    const cleared = setOpenCodeMarkdownAgentFrontmatterField(
      blockScalarMarkdown,
      'model',
      undefined,
    );

    assert.match(replaced, /model: "provider\/model-b"\nmode: all/);
    assert.doesNotMatch(replaced, /provider\/model-a|continued/);
    assert.doesNotMatch(cleared, /^model:/m);
    assert.doesNotMatch(cleared, /provider\/model-a|continued/);
    assert.match(cleared, /description: Reviews code\nmode: all/);
  }
});

test('preserves CRLF while replacing YAML block scalar fields', () => {
  const blockScalarMarkdown = '---\r\ndescription: Reviews code\r\nvariant: >\r\n  high\r\nmode: all\r\n---\r\nPrompt';
  const result = setOpenCodeMarkdownAgentFrontmatterField(
    blockScalarMarkdown,
    'variant',
    'low',
  );

  assert.match(result, /variant: "low"\r\nmode: all/);
  assert.doesNotMatch(result, /\r?\n  high/);
});

test('replaces prompt while preserving frontmatter', () => {
  const result = replaceOpenCodeMarkdownAgentPrompt(markdown, 'New prompt.');
  assert.match(result, /description: Reviews code/);
  assert.match(result, /permission:\n  edit: deny/);
  assert.match(result, /---\n\nNew prompt\.$/);
});

test('replaces frontmatter while preserving prompt', () => {
  const result = replaceOpenCodeMarkdownAgentFrontmatter(markdown, 'description: Updated\nmode: subagent');
  assert.match(result, /^---\ndescription: Updated\nmode: subagent\n---/);
  assert.match(result, /Original prompt\./);
});

test('markdown config overlays json config in source order', () => {
  assert.deepEqual(mergeOpenCodeAgentConfigs(
    { description: 'JSON', model: 'json/model', temperature: 0.2 },
    [{ description: 'Markdown' }, { model: 'markdown/model' }],
  ), {
    description: 'Markdown',
    model: 'markdown/model',
    temperature: 0.2,
  });
});

test('resolves the source that currently owns an agent field', () => {
  assert.deepEqual(
    resolveOpenCodeAgentConfigFieldSource(
      { model: 'json/model', variant: 'json-variant' },
      [{ description: 'first' }, { model: 'markdown/model' }],
      'model',
    ),
    { type: 'markdown', index: 1 },
  );
  assert.deepEqual(
    resolveOpenCodeAgentConfigFieldSource(
      { model: 'json/model', variant: 'json-variant' },
      [{ description: 'first' }, { model: 'markdown/model' }],
      'variant',
    ),
    { type: 'json' },
  );
  assert.equal(
    resolveOpenCodeAgentConfigFieldSource(undefined, [{ description: 'first' }], 'model'),
    undefined,
  );
});
