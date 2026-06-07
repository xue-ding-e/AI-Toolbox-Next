import React from 'react';
import { AlertTriangle, CheckCircle2, Copy, Info, Terminal } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { parseSessionCommandTags } from './domain/commandTags';
import SessionAnsiText from './SessionAnsiText';
import SessionRendererCard from './SessionRendererCard';
import SessionSearchHighlight from './SessionSearchHighlight';
import styles from './SessionDetailWorkbench.module.less';

interface SessionCommandBlockProps {
  text: string;
  query: string;
  onCopyText: (text: string, successText: string) => void | Promise<void>;
}

const SessionCommandBlock: React.FC<SessionCommandBlockProps> = ({ text, query, onCopyText }) => {
  const { t } = useTranslation();
  const parsedCommand = React.useMemo(() => parseSessionCommandTags(text), [text]);
  const title = parsedCommand.commandName || t('sessionManager.commandExecution');
  const commandName = parsedCommand.commandName || '';
  const hasDetails = Boolean(
    parsedCommand.commandArgs
      || parsedCommand.commandMessage
      || parsedCommand.localStdoutBlocks.length > 0
      || parsedCommand.outputTags.length > 0
      || parsedCommand.caveats.length > 0
      || parsedCommand.remainingText,
  );

  return (
    <SessionRendererCard
      icon={Terminal}
      title={title}
      variant="code"
      meta={commandName ? (
        <InlineCopyButton
          label={t('sessionManager.copyCommand')}
          onClick={() => void onCopyText(commandName, t('sessionManager.copyCommandSuccess'))}
        />
      ) : null}
    >
      {hasDetails ? (
        <div className={styles.toolBodyStack}>
          {parsedCommand.commandArgs ? (
            <CommandInfoRow
              label={t('sessionManager.commandArguments')}
              value={parsedCommand.commandArgs}
              query={query}
              code
            />
          ) : null}
          {parsedCommand.commandMessage ? (
            <CommandInfoRow
              label={t('sessionManager.commandStatus')}
              value={parsedCommand.commandMessage}
              query={query}
            />
          ) : null}
          {parsedCommand.localStdoutBlocks.map((output, index) => (
            <CommandOutputBlock
              key={`local-${index}`}
              title={t('sessionManager.commandOutput')}
              text={output}
              query={query}
            />
          ))}
          {parsedCommand.outputTags.map((output, index) => (
            <CommandOutputBlock
              key={`${output.name}-${index}`}
              title={output.type === 'stderr'
                ? t('sessionManager.commandErrorOutput')
                : t('sessionManager.commandExecutionResult')}
              name={output.name}
              text={output.content}
              query={query}
              tone={output.type === 'stderr' ? 'error' : 'default'}
            />
          ))}
          {parsedCommand.caveats.map((caveat, index) => (
            <div key={`caveat-${index}`} className={styles.commandCaveat}>
              <Info size={14} aria-hidden="true" />
              <span>
                <SessionSearchHighlight text={caveat} query={query} />
              </span>
            </div>
          ))}
          {parsedCommand.remainingText ? (
            <div className={styles.messageText}>
              <SessionSearchHighlight text={parsedCommand.remainingText} query={query} />
            </div>
          ) : null}
        </div>
      ) : null}
    </SessionRendererCard>
  );
};

interface InlineCopyButtonProps {
  label: string;
  onClick: () => void;
}

const InlineCopyButton: React.FC<InlineCopyButtonProps> = ({ label, onClick }) => (
  <button
    type="button"
    className={styles.toolPayloadCopyButton}
    title={label}
    aria-label={label}
    onClick={(event) => {
      event.preventDefault();
      event.stopPropagation();
      onClick();
    }}
  >
    <Copy size={12} aria-hidden="true" />
  </button>
);

interface CommandInfoRowProps {
  label: string;
  value: string;
  query: string;
  code?: boolean;
}

const CommandInfoRow: React.FC<CommandInfoRowProps> = ({ label, value, query, code }) => (
  <div className={styles.commandInfoRow}>
    <span>{label}</span>
    {code ? (
      <code>
        <SessionSearchHighlight text={value} query={query} />
      </code>
    ) : (
      <span>
        <SessionSearchHighlight text={value} query={query} />
      </span>
    )}
  </div>
);

interface CommandOutputBlockProps {
  title: string;
  name?: string;
  text: string;
  query: string;
  tone?: 'default' | 'error';
}

const CommandOutputBlock: React.FC<CommandOutputBlockProps> = ({
  title,
  name,
  text,
  query,
  tone = 'default',
}) => {
  const Icon = tone === 'error' ? AlertTriangle : CheckCircle2;
  return (
    <div className={styles.commandOutputBlock}>
      <div className={`${styles.preBlockTitle} ${tone === 'error' ? styles.commandOutputErrorTitle : ''}`}>
        <Icon size={12} aria-hidden="true" />
        <span>{name ? `${title} (${name})` : title}</span>
      </div>
      <pre className={`${styles.preBlock}${tone === 'error' ? ` ${styles.preBlockError}` : ''}`}>
        {query.trim() ? <SessionSearchHighlight text={text} query={query} /> : <SessionAnsiText text={text} />}
      </pre>
    </div>
  );
};

export default SessionCommandBlock;
