import type { SessionMessage, SessionMessageBlock } from '../../types';

import { getMessageBlocks, isToolBlock } from './messageBlocks';
import { getNormalizedToolName, getToolVariant, inferNormalizedToolNameFromInput } from './toolCatalog';

export function pairToolBlocks(blocks: SessionMessageBlock[]): SessionMessageBlock[] {
  const result: SessionMessageBlock[] = [];
  const pendingCallIndex = new Map<string, number>();

  blocks.forEach((block) => {
    if (block.kind === 'tool_call') {
      const normalizedBlock = normalizeToolBlock(block);
      if (normalizedBlock.toolId) {
        pendingCallIndex.set(normalizedBlock.toolId, result.length);
      }
      result.push(normalizedBlock);
      return;
    }

    if (block.kind === 'tool_result' && block.toolId) {
      const callIndex = pendingCallIndex.get(block.toolId);
      if (callIndex !== undefined) {
        const callBlock = result[callIndex];
        result[callIndex] = normalizeToolBlock({
          ...callBlock,
          kind: 'tool_execution',
          output: block.output,
          isError: block.isError,
          status: inferDisplayStatus({ ...callBlock, output: block.output, isError: block.isError, status: block.status }),
        });
        pendingCallIndex.delete(block.toolId);
        return;
      }
    }

    result.push(normalizeToolBlock(block));
  });

  return result;
}

export function enrichSessionMessagesWithToolExecutions(messages: SessionMessage[]): SessionMessage[] {
  const toolCallIndex = new Map<string, SessionMessageBlock>();
  const toolResultIndex = new Map<string, IndexedToolResult>();

  messages.forEach((message, messageIndex) => {
    getMessageBlocks(message).forEach((block, blockIndex) => {
      if ((block.kind === 'tool_call' || block.kind === 'tool_execution') && block.toolId) {
        toolCallIndex.set(block.toolId, normalizeToolBlock(block));
      }
      if (block.kind === 'tool_result' && block.toolId && !toolResultIndex.has(block.toolId)) {
        toolResultIndex.set(block.toolId, {
          block,
          key: getBlockInstanceKey(messageIndex, blockIndex),
        });
      }
    });
  });

  const resultKeysLinkedToCalls = new Set(
    Array.from(toolResultIndex.entries())
      .filter(([toolId]) => toolCallIndex.has(toolId))
      .map(([, resultEntry]) => resultEntry.key),
  );
  const nextMessages: SessionMessage[] = [];

  messages.forEach((message, messageIndex) => {
    const blocks = getMessageBlocks(message);
    if (blocks.length === 0) {
      nextMessages.push(message);
      return;
    }

    let changed = false;
    const enrichedBlocks: SessionMessageBlock[] = [];

    blocks.forEach((block, blockIndex) => {
      if ((block.kind === 'tool_call' || block.kind === 'tool_execution') && block.toolId) {
        const resultEntry = toolResultIndex.get(block.toolId);
        if (resultEntry) {
          changed = true;
          enrichedBlocks.push(mergeToolResultIntoCall(block, resultEntry.block));
          return;
        }
      }

      if (block.kind === 'tool_result') {
        const blockKey = getBlockInstanceKey(messageIndex, blockIndex);
        if (resultKeysLinkedToCalls.has(blockKey)) {
          changed = true;
          return;
        }

        if (block.toolId) {
          const callBlock = toolCallIndex.get(block.toolId);
          if (callBlock) {
            changed = true;
            enrichedBlocks.push(mergeToolResultIntoCall(callBlock, block));
            return;
          }
        }
      }

      enrichedBlocks.push(normalizeToolBlock(block));
    });

    if (enrichedBlocks.length === 0 && blocks.every((block) => block.kind === 'tool_result')) {
      return;
    }

    if (!changed && areBlocksEquivalent(blocks, enrichedBlocks)) {
      nextMessages.push(message);
      return;
    }

    nextMessages.push({
      ...message,
      blocks: enrichedBlocks,
    });
  });

  return nextMessages;
}

export function normalizeToolBlock(block: SessionMessageBlock): SessionMessageBlock {
  if (!['tool_call', 'tool_result', 'tool_execution'].includes(block.kind)) {
    return block;
  }

  const normalizedToolName = normalizeBlockToolName(block);
  return {
    ...block,
    normalizedToolName,
    variant: block.variant || getToolVariant(block.toolName, normalizedToolName),
    status: inferDisplayStatus({ ...block, normalizedToolName }),
  };
}

function mergeToolResultIntoCall(callBlock: SessionMessageBlock, resultBlock: SessionMessageBlock): SessionMessageBlock {
  const normalizedCallBlock = normalizeToolBlock(callBlock);
  return normalizeToolBlock({
    ...normalizedCallBlock,
    kind: 'tool_execution',
    output: resultBlock.output,
    isError: resultBlock.isError,
    status: inferDisplayStatus({
      ...normalizedCallBlock,
      output: resultBlock.output,
      isError: resultBlock.isError,
      status: resultBlock.status,
    }),
    metadata: mergeToolMetadata(normalizedCallBlock.metadata, resultBlock.metadata),
  });
}

function mergeToolMetadata(callMetadata: unknown, resultMetadata: unknown): unknown {
  if (callMetadata === undefined) {
    return resultMetadata;
  }
  if (resultMetadata === undefined) {
    return callMetadata;
  }
  if (isPlainRecord(callMetadata) && isPlainRecord(resultMetadata)) {
    return {
      ...callMetadata,
      ...resultMetadata,
    };
  }
  return resultMetadata;
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function areBlocksEquivalent(previousBlocks: SessionMessageBlock[], nextBlocks: SessionMessageBlock[]): boolean {
  if (previousBlocks.length !== nextBlocks.length) {
    return false;
  }

  return previousBlocks.every((block, index) => {
    const nextBlock = nextBlocks[index];
    return nextBlock
      && isToolBlock(block) === isToolBlock(nextBlock)
      && block.kind === nextBlock.kind
      && block.normalizedToolName === nextBlock.normalizedToolName
      && block.variant === nextBlock.variant
      && block.status === nextBlock.status;
  });
}

interface IndexedToolResult {
  block: SessionMessageBlock;
  key: string;
}

function getBlockInstanceKey(messageIndex: number, blockIndex: number): string {
  return `${messageIndex}:${blockIndex}`;
}

export function inferDisplayStatus(block: SessionMessageBlock): string {
  if (block.isError) {
    return 'error';
  }

  const status = block.status?.toLowerCase();
  if (status && ['error', 'failed', 'failure', 'interrupted'].includes(status)) {
    return 'error';
  }
  if (status && ['warning', 'warn'].includes(status)) {
    return 'warning';
  }
  if (status && ['pending', 'running'].includes(status)) {
    return 'pending';
  }
  if (status && ['success', 'ok', 'completed'].includes(status)) {
    return 'success';
  }

  return block.output === undefined ? 'pending' : 'success';
}

function normalizeBlockToolName(block: SessionMessageBlock): string {
  const directName = getNormalizedToolName(block.toolName);
  if (directName !== 'unknown') {
    return directName;
  }

  const providedName = getNormalizedToolName(block.normalizedToolName);
  if (providedName !== 'unknown') {
    return providedName;
  }

  return inferNormalizedToolNameFromInput(block.input);
}
