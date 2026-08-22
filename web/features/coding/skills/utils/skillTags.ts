// Shared helpers for the skill tag system.
//
// Color decision (Why): a tag's color is derived from a deterministic hash of
// the tag text itself, NOT from its index in the global tag list. Index-based
// coloring recolors every pill whenever the tag set changes; hashing keeps one
// tag one color everywhere it appears.

/** Sentinel filter value meaning "skills without any tags". */
export const UNTAGGED_FILTER = '__untagged__';

/** Number of deterministic color classes available for tag pills. */
export const TAG_COLOR_COUNT = 8;

/** Minimal shape needed for tag aggregation; ManagedSkill satisfies it. */
export interface TagSource {
  readonly tags: ReadonlyArray<string>;
}

/**
 * FNV-1a style deterministic hash mapped into the color class range.
 * Same tag text always yields the same index in [0, TAG_COLOR_COUNT).
 */
export function hashTagColorIndex(tag: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < tag.length; i += 1) {
    hash ^= tag.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash % TAG_COLOR_COUNT;
}

/** Trim entries, drop empties and duplicates while preserving first-seen order. */
export function normalizeTagList(tags: ReadonlyArray<string>): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of tags) {
    const trimmed = raw.trim();
    if (!trimmed || seen.has(trimmed)) continue;
    seen.add(trimmed);
    result.push(trimmed);
  }
  return result;
}

/** Collect every distinct tag across skills, sorted alphabetically. */
export function collectAllTags(skills: ReadonlyArray<TagSource>): string[] {
  const seen = new Set<string>();
  for (const skill of skills) {
    for (const raw of skill.tags ?? []) {
      const trimmed = raw.trim();
      if (trimmed) seen.add(trimmed);
    }
  }
  return Array.from(seen).sort((a, b) => a.localeCompare(b));
}

/**
 * AND semantics: a skill passes when it satisfies every selected filter.
 * The UNTAGGED_FILTER sentinel matches skills whose normalized tag set is empty.
 */
export function matchesTagFilters(
  skill: TagSource,
  filters: ReadonlyArray<string>,
): boolean {
  if (filters.length === 0) return true;
  const normalized = normalizeTagList(skill.tags ?? []);
  const hasUntagged = normalized.length === 0;
  return filters.every((filter) =>
    filter === UNTAGGED_FILTER ? hasUntagged : normalized.includes(filter),
  );
}

/**
 * Drop filter entries that no longer exist in the current tag space so the
 * filter UI never keeps dead pills. Keeps UNTAGGED_FILTER only while some
 * visible skill is still untagged.
 */
export function pruneStaleTagFilters(
  prev: ReadonlyArray<string>,
  availableTags: ReadonlyArray<string>,
  hasUntagged: boolean,
): string[] {
  return prev.filter((value) =>
    value === UNTAGGED_FILTER ? hasUntagged : availableTags.includes(value),
  );
}
