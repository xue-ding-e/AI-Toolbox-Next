import React from 'react';
import { Button } from 'antd';
import { Copy } from 'lucide-react';
import type { TFunction } from 'i18next';

import type { SessionMessage } from '../types';
import { getRoleLabel } from '../utils';
import type { SessionContentFilter } from './domain/messageFilters';
import { getMessageTargetId } from './domain/messageTargets';
import SessionMessageBlockRenderer from './SessionMessageBlockRenderer';
import styles from './SessionDetailWorkbench.module.less';

interface SessionMessageCardProps {
  message: SessionMessage;
  index: number;
  active: boolean;
  query: string;
  contentFilter: SessionContentFilter;
  assistantLabel: string;
  t: TFunction;
  onCopyText: (text: string, successText: string) => void | Promise<void>;
  onContentLayoutChange?: () => void;
  setMessageRef: (index: number, node: HTMLElement | null) => void;
  setTargetRef: (targetId: string, node: HTMLElement | null) => void;
}

const SessionMessageCard: React.FC<SessionMessageCardProps> = ({
  message,
  index,
  active,
  query,
  contentFilter,
  assistantLabel,
  t,
  onCopyText,
  onContentLayoutChange,
  setMessageRef,
  setTargetRef,
}) => {
  const role = message.role.toLowerCase();
  const messageTargetId = getMessageTargetId(message, index);
  const bindMessageRef = React.useCallback((node: HTMLElement | null) => {
    setMessageRef(index, node);
    setTargetRef(messageTargetId, node);
  }, [index, messageTargetId, setMessageRef, setTargetRef]);

  return (
    <article
      ref={bindMessageRef}
      id={messageTargetId}
      data-session-entry-id={messageTargetId}
      className={`${styles.messageCard} ${styles[`messageRole${capitalizeRole(role)}`] ?? styles.messageRoleUnknown}${active ? ` ${styles.messageCardActive}` : ''}`}
    >
      <header className={styles.messageMetaRow}>
        <div className={styles.messageHeaderMeta}>
          <span className={styles.messageRoleLabel}>{getDisplayRoleLabel(message.role, assistantLabel, t)}</span>
          {message.ts ? <span>{formatMessageTime(message.ts)}</span> : null}
          {message.usage ? (
            <span>{formatUsage(message.usage.inputTokens, message.usage.outputTokens)}</span>
          ) : null}
        </div>
        <div className={styles.messageHeaderRight}>
          {message.model ? <code className={styles.messageModelBadge}>{message.model}</code> : null}
          <Button
            type="text"
            size="small"
            icon={<Copy size={14} />}
            className={styles.messageCopyButton}
            aria-label={t('common.copy')}
            title={t('common.copy')}
            onClick={() => void onCopyText(message.content, t('sessionManager.copyMessageSuccess'))}
          />
        </div>
      </header>
      <div className={styles.messageContentShell}>
        <div className={styles.messageContent}>
          <SessionMessageBlockRenderer
            message={message}
            query={query}
            contentFilter={contentFilter}
            onCopyText={onCopyText}
            onContentLayoutChange={onContentLayoutChange}
            messageIndex={index}
            setTargetRef={setTargetRef}
          />
        </div>
      </div>
    </article>
  );
};

function capitalizeRole(role: string): string {
  if (role === 'user') {
    return 'User';
  }
  if (role === 'assistant') {
    return 'Assistant';
  }
  if (role === 'tool') {
    return 'Tool';
  }
  if (role === 'system') {
    return 'System';
  }
  return 'Unknown';
}

function getDisplayRoleLabel(role: string, assistantLabel: string, t: TFunction): string {
  if (role.toLowerCase() === 'assistant') {
    return assistantLabel;
  }
  return getRoleLabel(role, t);
}

function formatUsage(inputTokens?: number, outputTokens?: number): string {
  const input = inputTokens ?? 0;
  const output = outputTokens ?? 0;
  return `${input}/${output} tokens`;
}

function formatMessageTime(timestamp: number): string {
  return MESSAGE_TIME_FORMATTER.format(new Date(timestamp));
}

const MESSAGE_TIME_FORMATTER = new Intl.DateTimeFormat(undefined, {
  hour: 'numeric',
  minute: '2-digit',
});

export default React.memo(SessionMessageCard);
