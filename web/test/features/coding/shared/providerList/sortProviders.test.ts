import assert from 'node:assert/strict';
import test from 'node:test';
import {
  PROVIDER_SORT_MODES,
  PROVIDER_SORT_MODES_BASIC,
  filterProviderItems,
  isProviderSortMode,
  sortProviderItems,
} from '../../../../../features/coding/shared/providerList/sortProviders';

interface Item {
  id: string;
  name: string;
  note?: string;
  createdAt?: string;
}

const items: Item[] = [
  { id: 'a', name: 'Alpha', note: 'first', createdAt: '2026-01-01T00:00:00Z' },
  { id: 'b', name: 'beta', createdAt: '2026-03-01T00:00:00Z' },
  { id: 'c', name: 'Gamma', note: 'third' },
  { id: 'd', name: 'alpha-2', createdAt: '2026-02-01T00:00:00Z' },
];

const accessors = {
  name: (item: Item) => item.name,
  createdAt: (item: Item) => item.createdAt,
};

test('filterProviderItems matches case-insensitively across fields', () => {
  assert.deepEqual(
    filterProviderItems(items, 'ALPHA', (item: Item) => [item.name]).map((item) => item.id),
    ['a', 'd'],
  );
  assert.deepEqual(
    filterProviderItems(items, 'third', (item: Item) => [item.name, item.note ?? '']).map((item) => item.id),
    ['c'],
  );
});

test('filterProviderItems with blank or whitespace keyword keeps original order', () => {
  assert.equal(filterProviderItems(items, '', (item: Item) => [item.name]), items);
  assert.deepEqual(
    filterProviderItems(items, '   ', (item: Item) => [item.name]).map((item) => item.id),
    ['a', 'b', 'c', 'd'],
  );
});

test('sortProviderItems custom mode returns the original array untouched', () => {
  assert.equal(sortProviderItems(items, 'custom', accessors), items);
});

test('sortProviderItems name mode sorts case-insensitively by locale order', () => {
  const sorted = sortProviderItems(items, 'name', accessors).map((item) => item.id);
  // localeCompare groups letter variants together; alpha/Alpha/alpha-2 precede beta/gamma.
  assert.equal(sorted[0], 'a');
  assert.ok(sorted.indexOf('b') < sorted.indexOf('c'));
});

test('sortProviderItems created mode puts newest first and untracked items last', () => {
  const sorted = sortProviderItems(items, 'created', accessors).map((item) => item.id);
  assert.deepEqual(sorted, ['b', 'd', 'a', 'c']);
});

test('sortProviderItems recent mode uses the last-used map and keeps untracked order', () => {
  const lastUsedAt = (item: Item) =>
    ({ b: '2026-03-05T00:00:00Z', c: '2026-03-01T00:00:00Z' })[item.id];
  const sorted = sortProviderItems(items, 'recent', accessors, lastUsedAt).map((item) => item.id);
  // Tracked items sorted newest first; untracked 'a' and 'd' keep original order after them.
  assert.deepEqual(sorted, ['b', 'c', 'a', 'd']);
});

test('sortProviderItems does not mutate the input array', () => {
  const copy = [...items];
  sortProviderItems(items, 'name', accessors);
  assert.deepEqual(items, copy);
});

test('isProviderSortMode rejects unknown mode values', () => {
  for (const mode of PROVIDER_SORT_MODES) {
    assert.ok(isProviderSortMode(mode));
  }
  assert.equal(isProviderSortMode('newest'), false);
  assert.equal(isProviderSortMode(undefined), false);
});

test('basic mode set (no creation timestamps) excludes the created mode', () => {
  assert.ok((PROVIDER_SORT_MODES as readonly string[]).includes('created'));
  assert.ok(!(PROVIDER_SORT_MODES_BASIC as readonly string[]).includes('created'));
  assert.deepEqual([...PROVIDER_SORT_MODES_BASIC], ['custom', 'recent', 'name']);
});
