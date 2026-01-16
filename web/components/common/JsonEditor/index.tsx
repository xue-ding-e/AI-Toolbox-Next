import React, { useRef, useCallback, useEffect, useState } from 'react';
import MonacoEditor from 'react-monaco-editor';
import type { editor } from 'monaco-editor';
import * as monaco from 'monaco-editor';

type EditorMode = 'tree' | 'text' | 'table';

export interface JsonEditorProps {
  /** JSON value - can be an object, array, or any JSON-compatible value */
  value: unknown;
  /** Callback when content changes */
  onChange?: (value: unknown, isValid: boolean) => void;
  /** Editor mode: 'tree', 'text', or 'table' (only 'text' is supported with Monaco) */
  mode?: EditorMode;
  /** Read-only mode */
  readOnly?: boolean;
  /** Editor height */
  height?: number | string;
  /** Minimum height when resizable */
  minHeight?: number;
  /** Maximum height when resizable */
  maxHeight?: number;
  /** Enable resize handle */
  resizable?: boolean;
  /** Additional CSS class name */
  className?: string;
  /** Show main menu bar (not applicable for Monaco, kept for API compatibility) */
  showMainMenuBar?: boolean;
  /** Show status bar (not applicable for Monaco, kept for API compatibility) */
  showStatusBar?: boolean;
  /** Placeholder text when editor is empty */
  placeholder?: string;
}

/**
 * 基于 Monaco Editor 的 JSON 编辑器组件
 */
const JsonEditor: React.FC<JsonEditorProps> = ({
  value,
  onChange,
  mode: _mode = 'text',
  readOnly = false,
  height = 300,
  minHeight = 150,
  maxHeight = 800,
  resizable = true,
  className,
  showMainMenuBar: _showMainMenuBar = false,
  showStatusBar: _showStatusBar = false,
  placeholder,
}) => {
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const validateTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [editorContent, setEditorContent] = useState<string | null>(null);
  // 标记用户是否正在编辑器中输入
  const isUserEditingRef = useRef(false);

  // 规范化值为字符串
  const normalizedValue = value === undefined || value === null ? '' : value;

  // 转换为字符串显示
  const valueString = normalizedValue === ''
    ? ''
    : (typeof normalizedValue === 'string'
      ? normalizedValue
      : JSON.stringify(normalizedValue, null, 2));

  // 可调整大小的高度状态
  const initialHeight = typeof height === 'number' ? height : parseInt(height, 10) || 300;
  const [currentHeight, setCurrentHeight] = useState(initialHeight);

  // 调整大小相关
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
    if (!resizable) return;

    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizingRef.current) return;
      const deltaY = e.clientY - startYRef.current;
      const newHeight = Math.min(maxHeight, Math.max(minHeight, startHeightRef.current + deltaY));
      setCurrentHeight(newHeight);
    };

    const handleMouseUp = () => {
      if (isResizingRef.current) {
        isResizingRef.current = false;
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
      }
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [resizable, minHeight, maxHeight]);

  // 验证 JSON 内容并设置错误标记
  const validateAndSetMarkers = useCallback((content: string) => {
    if (!editorRef.current) return;

    const model = editorRef.current.getModel();
    if (!model) return;

    const trimmedContent = content.trim();
    if (trimmedContent === '') {
      // 空内容视为有效
      monaco.editor.setModelMarkers(model, 'json', []);
      return;
    }

    try {
      JSON.parse(content);
      // JSON 有效，清除错误标记
      monaco.editor.setModelMarkers(model, 'json', []);
    } catch (err: unknown) {
      if (err instanceof SyntaxError) {
        // 尝试从错误消息中提取位置
        const message = err.message;
        const posMatch = message.match(/position\s+(\d+)/i);
        let line = 1;
        let column = 1;

        if (posMatch) {
          const position = parseInt(posMatch[1], 10);
          // 计算行和列
          let currentPos = 0;
          const lines = content.split('\n');
          for (let i = 0; i < lines.length; i++) {
            if (currentPos + lines[i].length + 1 > position) {
              line = i + 1;
              column = position - currentPos + 1;
              break;
            }
            currentPos += lines[i].length + 1;
          }
        }

        monaco.editor.setModelMarkers(model, 'json', [
          {
            severity: monaco.MarkerSeverity.Error,
            startLineNumber: line,
            startColumn: column,
            endLineNumber: line,
            endColumn: model.getLineMaxColumn(line),
            message: message,
          },
        ]);
      }
    }
  }, []);

  const handleEditorDidMount = useCallback((
    editorInstance: editor.IStandaloneCodeEditor,
  ) => {
    editorRef.current = editorInstance;
    validateAndSetMarkers(valueString);
    setEditorContent(valueString);

    // 监听焦点事件，用于判断用户是否正在编辑
    editorInstance.onDidFocusEditorText(() => {
      isUserEditingRef.current = true;
    });
    editorInstance.onDidBlurEditorText(() => {
      isUserEditingRef.current = false;
    });
  }, [valueString, validateAndSetMarkers]);

  const handleChange = useCallback((newValue: string) => {
    setEditorContent(newValue);

    // 防抖验证
    if (validateTimeoutRef.current) {
      clearTimeout(validateTimeoutRef.current);
    }
    validateTimeoutRef.current = setTimeout(() => {
      validateAndSetMarkers(newValue);
    }, 300);

    if (!onChange) return;

    const trimmedValue = newValue.trim();
    if (trimmedValue === '') {
      // 空内容，回调 null 表示清空
      onChange(null, true);
      return;
    }

    try {
      const parsed = JSON.parse(newValue);
      onChange(parsed, true);
    } catch {
      // JSON 无效
      onChange(newValue, false);
    }
  }, [onChange, validateAndSetMarkers]);

  // 当外部 value 变化时更新编辑器
  useEffect(() => {
    // 比较是否真的变化了
    const newValueStr = typeof normalizedValue === 'string'
      ? normalizedValue
      : (normalizedValue === '' ? '' : JSON.stringify(normalizedValue, null, 2));

    console.log('[JsonEditor] useEffect triggered', {
      newValueStr,
      editorContent,
      isUserEditing: isUserEditingRef.current,
    });

    // 如果编辑器当前内容与新值相同，不需要更新
    if (editorContent === newValueStr) {
      console.log('[JsonEditor] Editor content matches new value, skipping');
      return;
    }

    // 如果用户正在编辑器中输入（编辑器有焦点），不要覆盖用户的输入
    if (isUserEditingRef.current) {
      console.log('[JsonEditor] User is editing, skipping external update');
      return;
    }

    console.log('[JsonEditor] Updating editorContent to:', newValueStr);
    setEditorContent(newValueStr);

    // 更新编辑器内容（如果编辑器已挂载）
    if (editorRef.current) {
      const model = editorRef.current.getModel();
      if (model) {
        model.setValue(newValueStr);
      }
    }
  }, [normalizedValue, editorContent]);

  useEffect(() => {
    return () => {
      if (validateTimeoutRef.current) {
        clearTimeout(validateTimeoutRef.current);
      }
    };
  }, []);

  const options: editor.IStandaloneEditorConstructionOptions = {
    readOnly,
    minimap: { enabled: false },
    lineNumbers: 'on',
    scrollBeyondLastLine: false,
    wordWrap: 'on',
    automaticLayout: true,
    fontSize: 13,
    tabSize: 2,
    renderLineHighlight: 'line',
    scrollbar: {
      vertical: 'auto',
      horizontal: 'auto',
      verticalScrollbarSize: 8,
      horizontalScrollbarSize: 8,
    },
    padding: { top: 8, bottom: 8 },
    folding: true,
    lineDecorationsWidth: 8,
    formatOnPaste: true,
    formatOnType: true,
  };

  const actualHeight = resizable ? currentHeight : (typeof height === 'number' ? height : parseInt(height, 10) || 300);

  // 判断是否显示 placeholder - 只要编辑器有任何字符就不显示
  // editorContent 始终与编辑器实际内容同步
  const showPlaceholder = placeholder && (editorContent === null || editorContent.trim() === '');

  console.log('[JsonEditor] Render', {
    value,
    normalizedValue,
    valueString,
    editorContent,
    showPlaceholder,
  });

  return (
    <div style={{ position: 'relative', height: actualHeight }}>
      <div
        className={className}
        style={{
          height: '100%',
          border: '1px solid #d9d9d9',
          borderRadius: 6,
          overflow: 'hidden',
        }}
      >
        <MonacoEditor
          width="100%"
          height={actualHeight}
          language="json"
          theme="vs"
          value={valueString}
          options={options}
          onChange={handleChange}
          editorDidMount={handleEditorDidMount}
        />
        {showPlaceholder && (
          <div
            style={{
              position: 'absolute',
              top: 9,
              left: 60,
              color: '#999',
              fontSize: 13,
              pointerEvents: 'none',
              userSelect: 'none',
              whiteSpace: 'pre',
              fontFamily: 'Menlo, Monaco, "Courier New", monospace',
            }}
          >
            {placeholder}
          </div>
        )}
      </div>
      {resizable && (
        <div
          onMouseDown={handleMouseDown}
          style={{
            position: 'absolute',
            bottom: 0,
            right: 0,
            width: 16,
            height: 16,
            cursor: 'ns-resize',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            opacity: 0.5,
            transition: 'opacity 0.2s',
          }}
          onMouseEnter={(e) => { e.currentTarget.style.opacity = '1'; }}
          onMouseLeave={(e) => { e.currentTarget.style.opacity = '0.5'; }}
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
            <path d="M8 2L2 8M8 5L5 8M8 8L8 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </div>
      )}
    </div>
  );
};

export default JsonEditor;
