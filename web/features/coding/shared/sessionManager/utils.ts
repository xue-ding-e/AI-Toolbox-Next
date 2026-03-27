import type { TFunction } from 'i18next';

import type { SessionMessage, SessionMeta, SessionTocItem } from './types';

export function formatSessionTitle(session: SessionMeta): string {
  if (session.title?.trim()) {
    return session.title.trim();
  }

  if (session.projectDir?.trim()) {
    const normalized = session.projectDir.replace(/[\\/]+$/, '');
    const basename = normalized.split(/[\\/]/).pop();
    if (basename?.trim()) {
      return basename.trim();
    }
  }

  return shortSessionId(session.sessionId);
}

export function shortSessionId(sessionId: string): string {
  if (sessionId.length <= 12) {
    return sessionId;
  }
  return `${sessionId.slice(0, 8)}...${sessionId.slice(-4)}`;
}

export function formatTimestamp(timestamp?: number): string {
  if (!timestamp) {
    return '';
  }

  return new Date(timestamp).toLocaleString();
}

export function formatRelativeTime(timestamp: number | undefined, t: TFunction): string {
  if (!timestamp) {
    return t('common.notSet');
  }

  const date = new Date(timestamp);
  const diffMs = Date.now() - timestamp;
  const diffMinutes = Math.floor(diffMs / 60_000);
  if (diffMinutes < 1) {
    return t('sessionManager.justNow');
  }
  if (diffMinutes < 60) {
    return t('sessionManager.minutesAgo', { count: diffMinutes });
  }

  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) {
    return t('sessionManager.hoursAgo', { count: diffHours });
  }

  const diffDays = Math.floor(diffHours / 24);
  if (diffDays > 7) {
    return date.toLocaleString();
  }
  return t('sessionManager.daysAgo', { count: diffDays });
}

export function getRoleLabel(role: string, t: TFunction): string {
  const normalizedRole = role.toLowerCase();
  if (normalizedRole === 'user') {
    return t('sessionManager.roles.user');
  }
  if (normalizedRole === 'assistant') {
    return t('sessionManager.roles.assistant');
  }
  if (normalizedRole === 'tool') {
    return t('sessionManager.roles.tool');
  }
  if (normalizedRole === 'system') {
    return t('sessionManager.roles.system');
  }
  return role;
}

export function getToolLabel(tool: SessionMeta['providerId'], t: TFunction): string {
  switch (tool) {
    case 'claudecode':
      return t('subModules.claudecode');
    case 'openclaw':
      return t('subModules.openclaw');
    case 'opencode':
      return t('subModules.opencode');
    case 'codex':
    default:
      return t('subModules.codex');
  }
}

export function buildSessionTocItems(messages: SessionMessage[]): SessionTocItem[] {
  return messages
    .map((message, index) => ({ message, index }))
    .filter(({ message }) => message.role.toLowerCase() === 'user')
    .map(({ message, index }) => ({
      index,
      preview: createPreview(message.content),
      ts: message.ts,
    }));
}

export function createPreview(content: string, maxLength = 80): string {
  const collapsed = content.replace(/\s+/g, ' ').trim();
  if (collapsed.length <= maxLength) {
    return collapsed;
  }
  return `${collapsed.slice(0, maxLength)}...`;
}

export function shouldCollapseMessage(content: string): boolean {
  const lineCount = content.split('\n').length;
  return lineCount > 20 || content.length > 1500;
}
