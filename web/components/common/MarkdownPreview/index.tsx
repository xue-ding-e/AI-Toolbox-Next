import React from 'react';
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark, oneLight } from 'react-syntax-highlighter/dist/esm/styles/prism';
import remarkGfm from 'remark-gfm';
import { useThemeStore } from '@/stores/themeStore';
import 'github-markdown-css/github-markdown.css';
import styles from './index.module.less';

export interface MarkdownPreviewProps {
  content?: string | null;
  className?: string;
  style?: React.CSSProperties;
}

const MarkdownPreview: React.FC<MarkdownPreviewProps> = ({
  content,
  className,
  style,
}) => {
  const { resolvedTheme } = useThemeStore();
  const syntaxHighlighterStyle = resolvedTheme === 'dark' ? oneDark : oneLight;

  const markdownComponents = {
    pre: ({ children }: React.HTMLAttributes<HTMLPreElement>) => <>{children}</>,
    code: ({
      children,
      className: codeClassName,
      ...props
    }: React.HTMLAttributes<HTMLElement> & { children?: React.ReactNode }) => {
      const rawText = String(children ?? '').replace(/\n$/, '');
      const languageMatch = /language-([\w-]+)/.exec(codeClassName || '');
      const language = languageMatch?.[1];
      const isBlockCode = Boolean(language) || rawText.includes('\n');

      if (isBlockCode) {
        return (
          <SyntaxHighlighter
            language={language || 'text'}
            style={syntaxHighlighterStyle}
            PreTag="pre"
            customStyle={{
              margin: '8px 0',
              padding: '12px 16px',
              borderRadius: 8,
              border: '1px solid var(--color-border)',
              background: 'var(--color-bg-base)',
              fontSize: 12,
              lineHeight: 1.7,
              overflowX: 'auto',
            }}
            codeTagProps={{
              style: {
                background: 'transparent',
                fontFamily: 'SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace',
              },
            }}
          >
            {rawText}
          </SyntaxHighlighter>
        );
      }

      return (
        <code {...props} className={codeClassName}>
          {children}
        </code>
      );
    },
  };

  return (
    <div className={`${styles.previewWrapper} ${className || ''}`.trim()} style={style}>
      <div className="markdown-body">
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          components={markdownComponents}
        >
          {content || ''}
        </ReactMarkdown>
      </div>
    </div>
  );
};

export default MarkdownPreview;
