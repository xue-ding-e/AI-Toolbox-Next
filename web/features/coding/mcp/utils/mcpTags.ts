import type { McpServer } from '../types';

// MCP reuses the Skills tag helpers so the two management pages share the
// same color hash, normalization, filtering and sentinel semantics.
export {
  UNTAGGED_FILTER,
  hashTagColorIndex,
  normalizeTagList,
  collectAllTags,
  matchesTagFilters,
  pruneStaleTagFilters,
  TAG_COLOR_COUNT,
} from '@/features/coding/skills/utils/skillTags';

/** Minimal shape adapter so `matchesTagFilters` can consume McpServer directly. */
export interface McpTagSource {
  readonly tags: ReadonlyArray<string>;
}

export function isMcpTagSource(server: McpServer): McpTagSource {
  return { tags: server.tags ?? [] };
}