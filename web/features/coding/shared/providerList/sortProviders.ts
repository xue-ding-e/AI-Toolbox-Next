/**
 * Pure helpers for filtering and sorting provider lists in coding tabs.
 *
 * Sorting operates on already-loaded arrays and never mutates the original
 * order: "custom" keeps the backend order (sort_index / config file order,
 * i.e. the drag-sort result), and non-custom modes return a new array.
 * Array#sort is stable (ES2019+), so items without a timestamp keep their
 * original relative order and sink below tracked ones.
 */

export const PROVIDER_SORT_MODES = ['custom', 'recent', 'created', 'name'] as const;

/**
 * Sort modes for tabs whose providers carry no creation timestamp
 * (config-file based tabs: opencode/pi/oh_my_pi/hermes/dsh/openclaw).
 */
export const PROVIDER_SORT_MODES_BASIC = ['custom', 'recent', 'name'] as const;

export type ProviderSortMode = (typeof PROVIDER_SORT_MODES)[number];

export const isProviderSortMode = (value: unknown): value is ProviderSortMode =>
  typeof value === 'string' && (PROVIDER_SORT_MODES as readonly string[]).includes(value);

export interface ProviderSortAccessors<T> {
  name: (item: T) => string;
  /** RFC3339 creation timestamp; items without one sink below tracked items. */
  createdAt?: (item: T) => string | undefined;
}

export function filterProviderItems<T>(
  items: T[],
  keyword: string,
  textFields: (item: T) => string[],
): T[] {
  const trimmed = keyword.trim().toLowerCase();
  if (!trimmed) {
    return items;
  }
  return items.filter((item) =>
    textFields(item).some((text) => text.toLowerCase().includes(trimmed)),
  );
}

export function sortProviderItems<T>(
  items: T[],
  mode: ProviderSortMode,
  accessors: ProviderSortAccessors<T>,
  lastUsedAt?: (item: T) => string | undefined,
): T[] {
  if (mode === 'custom') {
    return items;
  }
  const sorted = [...items];
  if (mode === 'name') {
    sorted.sort((left, right) => accessors.name(left).localeCompare(accessors.name(right)));
    return sorted;
  }
  const timestampOf =
    mode === 'recent' ? lastUsedAt : accessors.createdAt ? accessors.createdAt : undefined;
  sorted.sort((left, right) => {
    const leftTime = Date.parse(timestampOf?.(left) ?? '');
    const rightTime = Date.parse(timestampOf?.(right) ?? '');
    if (Number.isNaN(leftTime)) return Number.isNaN(rightTime) ? 0 : 1;
    if (Number.isNaN(rightTime)) return -1;
    return rightTime - leftTime;
  });
  return sorted;
}
