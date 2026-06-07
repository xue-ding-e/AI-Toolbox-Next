import React from 'react';
import { Brain, ChevronDown, FileText, Image, Info, Lock } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import MarkdownPreview from '@/components/common/MarkdownPreview';
import type { SessionMessage, SessionMessageBlock } from '../types';
import { hasSessionCommandTags } from './domain/commandTags';
import { getBlockText, isToolBlock, valueToSearchText } from './domain/messageBlocks';
import { getVisibleMessageBlockItems, type SessionContentFilter } from './domain/messageFilters';
import { getToolTargetId } from './domain/messageTargets';
import SessionCommandBlock from './SessionCommandBlock';
import SessionRendererCard from './SessionRendererCard';
import SessionSearchHighlight from './SessionSearchHighlight';
import SessionToolExecutionCard from './SessionToolExecutionCard';
import styles from './SessionDetailWorkbench.module.less';

const COLLAPSED_LINE_COUNT = 5;
const PLAIN_TEXT_COLLAPSE_LENGTH_THRESHOLD = 180;
const MARKDOWN_COLLAPSE_LENGTH_THRESHOLD = 480;
const MARKDOWN_PREVIEW_EXTRA_CHARACTERS = 160;

interface SessionMessageBlockRendererProps {
  message: SessionMessage;
  query: string;
  contentFilter: SessionContentFilter;
  onCopyText: (text: string, successText: string) => void | Promise<void>;
  onContentLayoutChange?: () => void;
  messageIndex: number;
  setTargetRef: (targetId: string, node: HTMLElement | null) => void;
}

const SessionMessageBlockRenderer: React.FC<SessionMessageBlockRendererProps> = ({
  message,
  query,
  contentFilter,
  onCopyText,
  onContentLayoutChange,
  messageIndex,
  setTargetRef,
}) => {
  const blockItems = getVisibleMessageBlockItems(message, contentFilter);

  return (
    <div className={styles.blockStack}>
      {blockItems.map(({ block, index }) => {
        const targetId = isToolBlock(block)
          ? getToolTargetId(message, messageIndex, block, index)
          : undefined;
        return (
          <div
            key={`${block.kind}-${block.toolId ?? block.title ?? index}`}
            ref={(node) => {
              if (targetId) {
                setTargetRef(targetId, node);
              }
            }}
            className={styles.blockAnchor}
            data-session-entry-id={targetId}
          >
            <BlockRenderer
              block={block}
              role={message.role}
              query={query}
              onCopyText={onCopyText}
              onContentLayoutChange={onContentLayoutChange}
            />
          </div>
        );
      })}
    </div>
  );
};

interface BlockRendererProps {
  block: SessionMessageBlock;
  role: string;
  query: string;
  onCopyText: (text: string, successText: string) => void | Promise<void>;
  onContentLayoutChange?: () => void;
}

const BlockRenderer: React.FC<BlockRendererProps> = ({
  block,
  role,
  query,
  onCopyText,
  onContentLayoutChange,
}) => {
  const blockText = block.text || valueToSearchText(block.output);
  if (blockText && hasSessionCommandTags(blockText)) {
    return <SessionCommandBlock text={blockText} query={query} onCopyText={onCopyText} />;
  }

  if (block.kind === 'tool_call' || block.kind === 'tool_result' || block.kind === 'tool_execution') {
    return <SessionToolExecutionCard block={block} query={query} onCopyText={onCopyText} />;
  }

  if (block.kind === 'thinking') {
    return (
      <SessionRendererCard icon={Brain} title={block.title || 'Thinking'} variant="thinking">
        <TextBlock
          text={block.text || getBlockText(block)}
          role="assistant"
          query={query}
          surface="plain"
          onContentLayoutChange={onContentLayoutChange}
        />
      </SessionRendererCard>
    );
  }

  if (block.kind === 'redacted_thinking') {
    return (
      <SessionRendererCard icon={Lock} title={block.title || 'Redacted thinking'} variant="neutral">
        <div className={styles.resultMuted}>
          <SessionSearchHighlight text={block.text || 'Reasoning content is hidden.'} query={query} />
        </div>
      </SessionRendererCard>
    );
  }

  if (block.kind === 'summary') {
    return (
      <SessionRendererCard icon={FileText} title={block.title || 'Summary'} variant="document">
        <TextBlock
          text={block.text || ''}
          role="assistant"
          query={query}
          surface="plain"
          onContentLayoutChange={onContentLayoutChange}
        />
      </SessionRendererCard>
    );
  }

  if (block.kind === 'system') {
    return (
      <SessionRendererCard icon={Info} title={block.title || 'System'} variant="system">
        <TextBlock
          text={block.text || valueToSearchText(block.output)}
          role="system"
          query={query}
          surface="plain"
          onContentLayoutChange={onContentLayoutChange}
        />
      </SessionRendererCard>
    );
  }

  if (block.kind === 'image') {
    const source = block.text || valueToSearchText(block.output);
    return (
      <SessionRendererCard icon={Image} title={block.title || 'Image'} variant="document">
        {source ? <img className={styles.imagePreview} src={source} alt={block.title || 'Session image'} /> : null}
      </SessionRendererCard>
    );
  }

  if (block.kind === 'unknown') {
    return (
      <SessionRendererCard icon={Info} title={block.title || 'Unknown block'} variant="neutral">
        <pre className={styles.preBlock}>
          <SessionSearchHighlight text={block.text || valueToSearchText(block.metadata) || valueToSearchText(block.output)} query={query} />
        </pre>
      </SessionRendererCard>
    );
  }

  return (
    <TextBlock
      text={block.text || valueToSearchText(block.output)}
      role={role}
      query={query}
      onContentLayoutChange={onContentLayoutChange}
    />
  );
};

interface TextBlockProps {
  text: string;
  role: string;
  query: string;
  surface?: 'bubble' | 'plain';
  onContentLayoutChange?: () => void;
}

const TextBlock: React.FC<TextBlockProps> = ({
  text,
  role,
  query,
  surface = 'bubble',
  onContentLayoutChange,
}) => {
  if (!text) {
    return null;
  }

  const normalizedRole = role.toLowerCase();
  const hasSearchQuery = query.trim().length > 0;
  const shouldRenderMarkdown = normalizedRole === 'assistant' && !hasSearchQuery;
  const content = shouldRenderMarkdown ? (
    <CollapsibleMarkdown content={text} onContentLayoutChange={onContentLayoutChange} />
  ) : (
    <CollapsiblePlainText text={text} query={query} onContentLayoutChange={onContentLayoutChange} />
  );

  if (surface === 'plain') {
    return content;
  }

  return (
    <div className={`${styles.textBubble} ${getTextBubbleClass(role)}`}>
      {content}
    </div>
  );
};

interface CollapsiblePlainTextProps {
  text: string;
  query: string;
  onContentLayoutChange?: () => void;
}

const CollapsiblePlainText: React.FC<CollapsiblePlainTextProps> = ({
  text,
  query,
  onContentLayoutChange,
}) => {
  const shouldCollapse = shouldUsePlainTextCollapse(text);

  return (
    <CollapsibleContent
      overflowing={shouldCollapse}
      resetKey={`${query.trim()}:${text.length}:${getExplicitLineCount(text)}`}
      onContentLayoutChange={onContentLayoutChange}
      renderContent={(expanded) => {
        const visibleText = shouldCollapse && !expanded ? createCollapsedTextPreview(text, query) : text;
        return (
          <div className={styles.messageText}>
            {query.trim() ? <SessionSearchHighlight text={visibleText} query={query} /> : visibleText}
          </div>
        );
      }}
    />
  );
};

interface CollapsibleMarkdownProps {
  content: string;
  onContentLayoutChange?: () => void;
}

const CollapsibleMarkdown: React.FC<CollapsibleMarkdownProps> = ({ content, onContentLayoutChange }) => {
  const shouldCollapse = shouldUseMarkdownCollapse(content);
  return (
    <CollapsibleContent
      overflowing={shouldCollapse}
      resetKey={`${content.length}:${getExplicitLineCount(content)}`}
      onContentLayoutChange={onContentLayoutChange}
      renderContent={(expanded) => (
        <MarkdownPreview
          content={shouldCollapse && !expanded ? createCollapsedMarkdownPreview(content) : content}
          className={styles.messageMarkdownPreview}
        />
      )}
    />
  );
};

interface CollapsibleContentProps {
  overflowing: boolean;
  resetKey: string;
  renderContent: (expanded: boolean) => React.ReactNode;
  onContentLayoutChange?: () => void;
}

const CollapsibleContent: React.FC<CollapsibleContentProps> = ({
  overflowing,
  resetKey,
  renderContent,
  onContentLayoutChange,
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = React.useState(false);
  const content = renderContent(expanded);

  React.useEffect(() => {
    setExpanded(false);
  }, [resetKey]);

  React.useEffect(() => {
    onContentLayoutChange?.();
  }, [expanded, onContentLayoutChange]);

  if (!overflowing) {
    return <>{content}</>;
  }

  return (
    <div className={styles.markdownCollapse}>
      <div
        className={[
          styles.markdownCollapseContent,
          expanded ? styles.markdownCollapseContentExpanded : '',
        ].filter(Boolean).join(' ')}
      >
        {content}
      </div>
      <button
        type="button"
        className={styles.markdownCollapseToggle}
        aria-expanded={expanded}
        onClick={() => setExpanded((current) => !current)}
      >
        <ChevronDown
          size={12}
          aria-hidden="true"
          className={`${styles.markdownCollapseToggleIcon}${expanded ? ` ${styles.markdownCollapseToggleIconExpanded}` : ''}`}
        />
        {expanded ? t('sessionManager.collapseMarkdown') : t('sessionManager.expandMarkdown')}
      </button>
    </div>
  );
};

function shouldUsePlainTextCollapse(text: string): boolean {
  return getExplicitLineCount(text) > COLLAPSED_LINE_COUNT || text.length > PLAIN_TEXT_COLLAPSE_LENGTH_THRESHOLD;
}

function shouldUseMarkdownCollapse(content: string): boolean {
  return getExplicitLineCount(content) > COLLAPSED_LINE_COUNT || content.length > MARKDOWN_COLLAPSE_LENGTH_THRESHOLD;
}

function createCollapsedTextPreview(text: string, query: string): string {
  const normalizedQuery = query.trim().toLowerCase();
  if (normalizedQuery) {
    const matchIndex = text.toLowerCase().indexOf(normalizedQuery);
    if (matchIndex >= 0) {
      const start = Math.max(0, matchIndex - 80);
      const end = Math.min(text.length, matchIndex + normalizedQuery.length + 120);
      return `${start > 0 ? '...' : ''}${text.slice(start, end).trim()}${end < text.length ? '...' : ''}`;
    }
  }

  const lines = text.split(/\r\n|\r|\n/);
  if (lines.length > COLLAPSED_LINE_COUNT) {
    return lines.slice(0, COLLAPSED_LINE_COUNT).join('\n');
  }
  return text.length > PLAIN_TEXT_COLLAPSE_LENGTH_THRESHOLD
    ? `${text.slice(0, PLAIN_TEXT_COLLAPSE_LENGTH_THRESHOLD).trimEnd()}...`
    : text;
}

function createCollapsedMarkdownPreview(content: string): string {
  const lines = content.split(/\r\n|\r|\n/);
  if (lines.length > COLLAPSED_LINE_COUNT) {
    return lines.slice(0, COLLAPSED_LINE_COUNT).join('\n');
  }

  const previewLength = Math.min(content.length, MARKDOWN_COLLAPSE_LENGTH_THRESHOLD + MARKDOWN_PREVIEW_EXTRA_CHARACTERS);
  return content.length > previewLength
    ? `${content.slice(0, previewLength).trimEnd()}\n\n...`
    : content;
}

function getExplicitLineCount(text: string): number {
  return text.split(/\r\n|\r|\n/).length;
}

function getTextBubbleClass(role: string): string {
  const normalizedRole = role.toLowerCase();
  if (normalizedRole === 'user') {
    return styles.textBubbleUser;
  }
  if (normalizedRole === 'assistant') {
    return styles.textBubbleAssistant;
  }
  if (normalizedRole === 'system') {
    return styles.textBubbleSystem;
  }
  return styles.textBubbleNeutral;
}

export default React.memo(SessionMessageBlockRenderer);
