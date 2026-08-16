import type { FC } from 'react';
import MonacoEditor from 'react-monaco-editor';
import type { editor } from 'monaco-editor';
import { useThemeStore } from '@/stores/themeStore';

export interface PlainTextEditorProps {
  /** Text content to display. */
  value: string;
  /** Whether the editor is read-only. */
  readOnly?: boolean;
  /** Monaco language id. Defaults to `plaintext`; use `yaml` or `markdown` for file previews. */
  language?: string;
  /** Editor height. Accepts numbers or CSS strings like `calc(...)`. */
  height?: number | string;
  /** Additional CSS class name. */
  className?: string;
}

/**
 * A lightweight Monaco editor for plain-text / YAML / Markdown source preview.
 * Unlike JsonEditor or TomlEditor it does not strictly validate the content; it
 * is intended for read-only file previews where showing the original text is
 * more important than format validation.
 */
const PlainTextEditor: FC<PlainTextEditorProps> = ({
  value,
  readOnly = true,
  language = 'plaintext',
  height = 300,
  className,
}) => {
  const { resolvedTheme } = useThemeStore();

  const options: editor.IStandaloneEditorConstructionOptions = {
    readOnly,
    minimap: { enabled: false },
    lineNumbers: 'on',
    lineNumbersMinChars: 3,
    scrollBeyondLastLine: false,
    wordWrap: 'on',
    automaticLayout: true,
    fontSize: 13,
    tabSize: 2,
    renderLineHighlight: 'none',
    scrollbar: {
      vertical: 'auto',
      horizontal: 'auto',
      verticalScrollbarSize: 8,
      horizontalScrollbarSize: 8,
    },
    padding: { top: 8, bottom: 8 },
    folding: true,
    lineDecorationsWidth: 8,
  };

  return (
    <div className={className} style={{ height }}>
      <div
        style={{
          height: '100%',
          border: '1px solid var(--color-border)',
          borderRadius: 6,
          overflow: 'hidden',
        }}
      >
        <MonacoEditor
          width="100%"
          height={height}
          language={language}
          theme={resolvedTheme === 'dark' ? 'vs-dark' : 'vs'}
          value={value}
          options={options}
        />
      </div>
    </div>
  );
};

export default PlainTextEditor;
