import type { SessionMessage, SessionMessageBlock } from '../../types';

import {
  getBlockText,
  getMessageBlocks,
  getMessagePreview,
  isMeaningfulPreview,
  isToolBlock,
} from './messageBlocks';
import type { SessionFilteredMessageItem } from './messageFilters';
import { countQueryMatches, getMessageSearchText, type SessionSearchScope } from './messageSearch';
import { getMessageTargetId, getToolTargetId } from './messageTargets';
import { getToolDisplayName, inferNormalizedToolNameFromInput } from './toolCatalog';
import { pairToolBlocks } from './toolPairing';

export interface SessionNavigatorEntry {
  id: string;
  targetId: string;
  messageIndex: number;
  turnIndex: number;
  role: string;
  kind: string;
  label: string;
  preview: string;
  ts?: number;
  hasToolUse: boolean;
  matchCount: number;
}

export function buildNavigatorEntries(
  messages: SessionMessage[],
  query: string,
  searchScope: SessionSearchScope = 'content',
): SessionNavigatorEntry[] {
  return buildNavigatorEntriesFromItems(
    messages.map((message, index) => ({ message, index })),
    query,
    searchScope,
  );
}

export function buildNavigatorEntriesFromItems(
  items: SessionFilteredMessageItem[],
  query: string,
  searchScope: SessionSearchScope = 'content',
): SessionNavigatorEntry[] {
  const entries = items.flatMap(({ message, index }) => {
    const blocks = pairToolBlocks(getMessageBlocks(message));
    const matchCount = countQueryMatches(getMessageSearchText(message, searchScope), query);
    const preview = getMessagePreview(message);
    const baseTargetId = getMessageTargetId(message, index);
    const baseEntry: SessionNavigatorEntry = {
      id: baseTargetId,
      targetId: baseTargetId,
      messageIndex: index,
      turnIndex: 0,
      role: message.role,
      kind: message.role.toLowerCase(),
      label: getRoleLabel(message.role),
      preview,
      ts: message.ts,
      hasToolUse: blocks.some(isToolBlock),
      matchCount,
    };

    const toolEntries = blocks
      .flatMap((block, blockIndex) => (
        isToolBlock(block) ? [createToolEntry(baseEntry, message, block, blockIndex, preview)] : []
      ))
      .filter((entry): entry is SessionNavigatorEntry => Boolean(entry));

    if (shouldIncludeBaseEntry(message, blocks, baseEntry.preview, toolEntries.length, matchCount)) {
      return [baseEntry, ...toolEntries];
    }

    return toolEntries;
  });

  return entries.map((entry, index) => ({
    ...entry,
    turnIndex: index + 1,
  }));
}

function createToolEntry(
  baseEntry: SessionNavigatorEntry,
  message: SessionMessage,
  block: SessionMessageBlock,
  blockIndex: number,
  fallbackPreview: string,
): SessionNavigatorEntry | null {
  const label = getToolEntryLabel(block);
  const preview = getToolEntryPreview(block, fallbackPreview, label);
  if (!isMeaningfulPreview(label) && !isMeaningfulPreview(preview)) {
    return null;
  }

  const targetId = getToolTargetId(message, baseEntry.messageIndex, block, blockIndex);
  return {
    id: targetId,
    targetId,
    messageIndex: baseEntry.messageIndex,
    turnIndex: 0,
    role: baseEntry.role,
    kind: 'tool',
    label: isMeaningfulPreview(label) ? label : 'Tool',
    preview: isMeaningfulPreview(preview) ? preview : label,
    ts: baseEntry.ts,
    hasToolUse: true,
    matchCount: baseEntry.matchCount,
  };
}

function shouldIncludeBaseEntry(
  message: SessionMessage,
  blocks: SessionMessageBlock[],
  preview: string,
  toolEntryCount: number,
  matchCount: number,
): boolean {
  if (!isMeaningfulPreview(preview)) {
    return false;
  }

  const normalizedRole = message.role.toLowerCase();
  if (normalizedRole === 'user' || normalizedRole === 'system') {
    return true;
  }
  if (blocks.some((block) => block.kind === 'summary')) {
    return true;
  }
  if (hasReadableNonToolBlock(blocks)) {
    return true;
  }
  if (matchCount > 0 && toolEntryCount === 0) {
    return true;
  }

  return toolEntryCount === 0;
}

function hasReadableNonToolBlock(blocks: SessionMessageBlock[]): boolean {
  return blocks.some((block) => {
    if (isToolBlock(block)) {
      return false;
    }
    return isMeaningfulPreview(getBlockText(block));
  });
}

function getToolEntryLabel(block: SessionMessageBlock): string {
  const directName = meaningfulName(block.toolName);
  if (directName) {
    return directName;
  }

  const normalizedName = meaningfulName(block.normalizedToolName);
  if (normalizedName) {
    return getToolDisplayName(normalizedName, undefined);
  }

  const title = meaningfulName(block.title);
  if (title) {
    return title;
  }

  const inferred = inferNormalizedToolNameFromInput(block.input);
  return inferred !== 'unknown' ? getToolDisplayName(inferred, undefined) : '';
}

function getToolEntryPreview(block: SessionMessageBlock, fallbackPreview: string, label: string): string {
  const title = meaningfulName(block.title);
  if (title) {
    return title;
  }

  const text = trimPreview(block.text || '');
  if (isMeaningfulPreview(text)) {
    return text;
  }

  if (isMeaningfulPreview(label)) {
    return label;
  }

  return isMeaningfulPreview(fallbackPreview) ? fallbackPreview : '';
}

function meaningfulName(value: string | undefined): string {
  const collapsed = trimPreview(value || '');
  return isMeaningfulPreview(collapsed) ? collapsed : '';
}

function trimPreview(value: string): string {
  const collapsed = value.replace(/\s+/g, ' ').trim();
  return collapsed.length > 96 ? `${collapsed.slice(0, 96)}...` : collapsed;
}

function getRoleLabel(role: string): string {
  const normalizedRole = role.toLowerCase();
  if (normalizedRole === 'user') {
    return 'User';
  }
  if (normalizedRole === 'assistant') {
    return 'Assistant';
  }
  if (normalizedRole === 'tool') {
    return 'Tool';
  }
  if (normalizedRole === 'system') {
    return 'System';
  }
  return role;
}
