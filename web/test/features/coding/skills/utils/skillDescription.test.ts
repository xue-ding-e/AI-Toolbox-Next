import assert from 'node:assert/strict';
import test from 'node:test';

import { flattenDescription } from '../../../../../features/coding/skills/utils/skillDescription.ts';

test('flattenDescription returns empty string for null/undefined/empty', () => {
  assert.equal(flattenDescription(null), '');
  assert.equal(flattenDescription(undefined), '');
  assert.equal(flattenDescription(''), '');
  assert.equal(flattenDescription('   '), '');
});

test('flattenDescription passes through a single-line description', () => {
  assert.equal(flattenDescription('Review frontend code'), 'Review frontend code');
});

test('flattenDescription collapses a multi-line block scalar into one line', () => {
  const multi = [
    'Full-stack frontend development combining premium UI design, cinematic animations,',
    'AI-generated media assets, persuasive copywriting, and visual art.',
  ].join('\n');
  assert.equal(
    flattenDescription(multi),
    'Full-stack frontend development combining premium UI design, cinematic animations, AI-generated media assets, persuasive copywriting, and visual art.',
  );
});

test('flattenDescription strips whitespace around newlines, not just the newline', () => {
  // Lines indented as in a YAML block scalar; leading/trailing spaces around
  // each newline must collapse to a single space.
  const indented = '  first line\n  second line  \n  third';
  assert.equal(flattenDescription(indented), 'first line second line third');
});

test('flattenDescription collapses multiple consecutive blank lines into one space', () => {
  assert.equal(flattenDescription('a\n\n\nb'), 'a b');
});

test('flattenDescription trims leading and trailing whitespace', () => {
  assert.equal(flattenDescription('  \n  hello world  \n  '), 'hello world');
});
