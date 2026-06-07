import type { SessionMessage, SessionMessageBlock } from '../../types';

export function getMessageTargetId(message: SessionMessage, messageIndex: number): string {
  const normalizedMessageId = sanitizeNavigatorTargetId(message.id || `index-${messageIndex}`);
  return `session-message-${normalizedMessageId}-${messageIndex}`;
}

export function getToolTargetId(
  message: SessionMessage,
  messageIndex: number,
  block: SessionMessageBlock,
  blockIndex: number,
): string {
  const normalizedBlockId = sanitizeNavigatorTargetId(block.toolId || block.toolName || 'block');
  return `${getMessageTargetId(message, messageIndex)}-tool-${blockIndex}-${normalizedBlockId}`;
}

function sanitizeNavigatorTargetId(value: string): string {
  const sanitized = value.trim().replace(/[^A-Za-z0-9_-]/g, '_');
  return sanitized || 'message';
}
