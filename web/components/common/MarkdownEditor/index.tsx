import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Button, Space } from 'antd';
import MonacoEditor from 'react-monaco-editor';
import type { editor } from 'monaco-editor';
import { useTranslation } from 'react-i18next';
import { useThemeStore } from '@/stores/themeStore';
import MarkdownPreview from '@/components/common/MarkdownPreview';

export interface MarkdownEditorProps {
  value?: string | null;
  onChange?: (value: string) => void;
  onBlur?: (value: string) => void;
  readOnly?: boolean;
  height?: number | string;
  minHeight?: number;
  maxHeight?: number;
  resizable?: boolean;
  className?: string;
  placeholder?: string;
}

const MarkdownEditor: React.FC<MarkdownEditorProps> = ({
  value,
  onChange,
  onBlur,
  readOnly = false,
  height = 300,
  minHeight = 150,
  maxHeight = 800,
  resizable = true,
  className,
  placeholder,
}) => {
  const { t } = useTranslation();
  const { resolvedTheme } = useThemeStore();
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const isUserEditingRef = useRef(false);
  const normalizedValue = value ?? '';
  const [editorContent, setEditorContent] = useState(normalizedValue);
  const [viewMode, setViewMode] = useState<'edit' | 'preview'>('edit');

  const initialHeight = typeof height === 'number' ? height : parseInt(height, 10) || 300;
  const [currentHeight, setCurrentHeight] = useState(initialHeight);
  const isResizingRef = useRef(false);
  const startYRef = useRef(0);
  const startHeightRef = useRef(0);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    isResizingRef.current = true;
    startYRef.current = e.clientY;
    startHeightRef.current = currentHeight;
    document.body.style.cursor = 'ns-resize';
    document.body.style.userSelect = 'none';
  }, [currentHeight]);

  useEffect(() => {
    if (!resizable) {
      return undefined;
    }

    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizingRef.current) {
        return;
      }
      const deltaY = e.clientY - startYRef.current;
      const nextHeight = Math.min(maxHeight, Math.max(minHeight, startHeightRef.current + deltaY));
      setCurrentHeight(nextHeight);
    };

    const handleMouseUp = () => {
      if (!isResizingRef.current) {
        return;
      }
      isResizingRef.current = false;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [maxHeight, minHeight, resizable]);

  const handleEditorDidMount = useCallback((editorInstance: editor.IStandaloneCodeEditor) => {
    editorRef.current = editorInstance;
    setEditorContent(normalizedValue);

    editorInstance.onDidFocusEditorText(() => {
      isUserEditingRef.current = true;
      editorInstance.updateOptions({ renderLineHighlight: 'line' });
    });

    editorInstance.onDidBlurEditorText(() => {
      isUserEditingRef.current = false;
      editorInstance.updateOptions({ renderLineHighlight: 'none' });
      if (onBlur) {
        onBlur(editorInstance.getValue());
      }
    });
  }, [normalizedValue, onBlur]);

  const handleChange = useCallback((newValue: string) => {
    setEditorContent(newValue);
    onChange?.(newValue);
  }, [onChange]);

  useEffect(() => {
    if (editorContent === normalizedValue || isUserEditingRef.current) {
      return;
    }

    setEditorContent(normalizedValue);
    if (editorRef.current && editorRef.current.getValue() !== normalizedValue) {
      editorRef.current.setValue(normalizedValue);
    }
  }, [editorContent, normalizedValue]);

  const containerStyle: React.CSSProperties = {
    position: 'relative',
    border: '1px solid var(--color-border)',
    borderRadius: 8,
    overflow: 'hidden',
    background: 'var(--color-bg-container)',
  };

  const FONT_SIZE = 13;
  const LINE_NUMBERS_MIN_CHARS = 3;
  const LINE_DECORATIONS_WIDTH = 8;
  const SWITCH_BAR_HEIGHT = 25;
  const EDITOR_TOP_PADDING = 8;
  const PLACEHOLDER_LEFT = LINE_NUMBERS_MIN_CHARS * (FONT_SIZE * 0.6) + LINE_DECORATIONS_WIDTH + 12;

  const placeholderStyle: React.CSSProperties = {
    position: 'absolute',
    top: SWITCH_BAR_HEIGHT + EDITOR_TOP_PADDING,
    left: PLACEHOLDER_LEFT,
    pointerEvents: 'none',
    userSelect: 'none',
    color: 'var(--color-text-tertiary)',
    fontSize: FONT_SIZE,
    lineHeight: 1.6,
    whiteSpace: 'pre-wrap',
    zIndex: 1,
    maxWidth: `calc(100% - ${PLACEHOLDER_LEFT + 16}px)`,
  };

  const switchBarStyle: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: 4,
    padding: '2px 4px',
    borderBottom: '1px solid var(--color-border)',
    background: 'var(--color-bg-container)',
  };

  const previewStyle: React.CSSProperties = {
    minHeight: currentHeight,
    maxHeight: currentHeight,
    overflowY: 'auto',
    padding: '8px 12px',
    background: 'var(--color-bg-container)',
    color: 'var(--color-text-primary)',
    fontSize: FONT_SIZE,
    lineHeight: 1.7,
  };

  const options: editor.IStandaloneEditorConstructionOptions = {
    readOnly,
    minimap: { enabled: false },
    lineNumbers: 'on',
    lineNumbersMinChars: LINE_NUMBERS_MIN_CHARS,
    scrollBeyondLastLine: false,
    wordWrap: 'on',
    automaticLayout: true,
    fontSize: FONT_SIZE,
    renderLineHighlight: 'none',
    scrollbar: {
      vertical: 'auto',
      horizontal: 'auto',
      verticalScrollbarSize: 8,
      horizontalScrollbarSize: 8,
    },
    padding: {
      top: EDITOR_TOP_PADDING,
      bottom: 8,
    },
    lineDecorationsWidth: LINE_DECORATIONS_WIDTH,
  };

  return (
    <div className={className}>
      <div style={containerStyle}>
        <div style={switchBarStyle}>
          <Space size={2}>
            <Button
              type="text"
              size="small"
              style={{
                padding: '0 4px',
                height: 20,
                fontSize: 11,
                color: viewMode === 'edit' ? 'var(--color-text-primary)' : 'var(--color-text-tertiary)',
              }}
              onClick={() => setViewMode('edit')}
            >
              {t('common.edit')}
            </Button>
            <Button
              type="text"
              size="small"
              style={{
                padding: '0 4px',
                height: 20,
                fontSize: 11,
                color: viewMode === 'preview' ? 'var(--color-text-primary)' : 'var(--color-text-tertiary)',
              }}
              onClick={() => setViewMode('preview')}
            >
              {t('common.preview')}
            </Button>
          </Space>
        </div>
        {!editorContent && placeholder && viewMode === 'edit' && (
          <div style={placeholderStyle}>{placeholder}</div>
        )}
        {viewMode === 'edit' ? (
          <MonacoEditor
            width="100%"
            height={currentHeight}
            language="markdown"
            theme={resolvedTheme === 'dark' ? 'vs-dark' : 'vs'}
            value={editorContent}
            onChange={handleChange}
            editorDidMount={handleEditorDidMount}
            options={options}
          />
        ) : (
          <MarkdownPreview content={editorContent || ''} style={previewStyle} />
        )}
        {resizable && (
          <div
            onMouseDown={handleMouseDown}
            style={{
              height: 10,
              cursor: 'ns-resize',
              background: 'var(--color-bg-hover)',
              borderTop: '1px solid var(--color-border)',
            }}
          />
        )}
      </div>
    </div>
  );
};

export default MarkdownEditor;
