import assert from 'node:assert/strict';
import test from 'node:test';

import {
  TAG_COLOR_COUNT,
  UNTAGGED_FILTER,
  collectAllTags,
  hashTagColorIndex,
  matchesTagFilters,
  normalizeTagList,
  pruneStaleTagFilters,
} from '../../../../../features/coding/skills/utils/skillTags.ts';

test('hashTagColorIndex is deterministic and within range', () => {
  const samples = ['web', 'cli', '搜索', 'data-pipeline', 'a', '', 'x'.repeat(500)];
  for (const sample of samples) {
    const index = hashTagColorIndex(sample);
    assert.ok(index >= 0 && index < TAG_COLOR_COUNT);
    assert.equal(hashTagColorIndex(sample), index);
  }
});

test('hashTagColorIndex spreads distinct tags across buckets', () => {
  const indexes = new Set<number>();
  for (let i = 0; i < 200; i += 1) {
    indexes.add(hashTagColorIndex(`tag-${i}`));
  }
  // 200 pseudo-random inputs should cover most of the 8 buckets.
  assert.ok(indexes.size >= 6);
});

test('normalizeTagList trims, drops empty and duplicate entries', () => {
  assert.deepEqual(normalizeTagList([' web ', 'web', ' cli', '']), ['web', 'cli']);
  assert.deepEqual(normalizeTagList([]), []);
});

test('collectAllTags dedupes and sorts alphabetically', () => {
  const skills = [
    { tags: ['zeta', ' alpha '] },
    { tags: ['alpha', 'beta'] },
    { tags: [] },
    { tags: ['  '] },
  ];
  assert.deepEqual(collectAllTags(skills), ['alpha', 'beta', 'zeta']);
  assert.deepEqual(collectAllTags([]), []);
});

test('matchesTagFilters uses AND semantics with untagged sentinel', () => {
  const tagged = { tags: ['web', 'cli'] };
  const other = { tags: ['web'] };
  const untagged = { tags: [] };

  assert.equal(matchesTagFilters(tagged, []), true);
  assert.equal(matchesTagFilters(untagged, []), true);

  assert.equal(matchesTagFilters(tagged, ['web']), true);
  assert.equal(matchesTagFilters(other, ['web', 'cli']), false);

  assert.equal(matchesTagFilters(untagged, [UNTAGGED_FILTER]), true);
  assert.equal(matchesTagFilters(tagged, [UNTAGGED_FILTER]), false);
  assert.equal(matchesTagFilters(untagged, [UNTAGGED_FILTER, 'web']), false);
});

test('pruneStaleTagFilters keeps only existing values and valid untagged', () => {
  assert.deepEqual(
    pruneStaleTagFilters(['web', 'gone', 'cli'], ['web', 'cli'], true),
    ['web', 'cli'],
  );
  assert.deepEqual(
    pruneStaleTagFilters([UNTAGGED_FILTER, 'web'], ['web'], false),
    ['web'],
  );
  assert.deepEqual(pruneStaleTagFilters([UNTAGGED_FILTER], [], true), [
    UNTAGGED_FILTER,
  ]);
});
