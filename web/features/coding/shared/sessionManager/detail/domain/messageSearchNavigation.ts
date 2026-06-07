import type { SessionMessage } from '../../types';

import { messageMatchesQuery, type SessionSearchScope } from './messageSearch';

export const NO_ACTIVE_MATCH_OFFSET = -1;

export interface IndexedSessionMessage {
  message: SessionMessage;
  index: number;
}

export function getVisibleMatchedMessageIndexes(
  items: IndexedSessionMessage[],
  query: string,
  searchScope: SessionSearchScope,
): number[] {
  if (!query.trim()) {
    return [];
  }

  return items
    .filter(({ message }) => messageMatchesQuery(message, query, searchScope))
    .map(({ index }) => index);
}

export function getNextMatchOffset(activeMatchOffset: number, matchCount: number): number {
  if (matchCount <= 0) {
    return NO_ACTIVE_MATCH_OFFSET;
  }
  if (activeMatchOffset < 0 || activeMatchOffset >= matchCount) {
    return 0;
  }
  return (activeMatchOffset + 1) % matchCount;
}

export function getPreviousMatchOffset(activeMatchOffset: number, matchCount: number): number {
  if (matchCount <= 0) {
    return NO_ACTIVE_MATCH_OFFSET;
  }
  if (activeMatchOffset < 0 || activeMatchOffset >= matchCount) {
    return matchCount - 1;
  }
  return (activeMatchOffset - 1 + matchCount) % matchCount;
}

export function getActiveMatchPosition(activeMatchOffset: number, matchCount: number): number {
  return activeMatchOffset >= 0 && activeMatchOffset < matchCount
    ? activeMatchOffset + 1
    : 0;
}
