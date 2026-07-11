import assert from 'node:assert/strict';
import test from 'node:test';

import {
  mergeOpenCodeAgentConfigs,
  replaceOpenCodeMarkdownAgentFrontmatter,
  replaceOpenCodeMarkdownAgentPrompt,
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
