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
}) => {
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const validateTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isInternalChangeRef = useRef(false);
  const lastExternalValueRef = useRef<string>('');

  // 规范化值为字符串
  const normalizedValue = value === undefined || value === null ? {} : value;
  const valueString = typeof normalizedValue === 'string'
    ? normalizedValue
    : JSON.stringify(normalizedValue, null, 2);

  // 初始化时保存外部值
  useEffect(() => {
    lastExternalValueRef.current = valueString;
  }, []);

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
  }, [valueString, validateAndSetMarkers]);

  const handleChange = useCallback((newValue: string) => {
    isInternalChangeRef.current = true;

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
      onChange({}, true);
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
    if (!editorRef.current) return;

    // 跳过内部变化
    if (isInternalChangeRef.current) {
      isInternalChangeRef.current = false;
      return;
    }

    // 比较是否真的变化了
    const newValueStr = typeof normalizedValue === 'string'
      ? normalizedValue
      : JSON.stringify(normalizedValue, null, 2);

    if (lastExternalValueRef.current === newValueStr) {
      return;
    }

    lastExternalValueRef.current = newValueStr;
    const model = editorRef.current.getModel();
    if (model) {
      model.setValue(newValueStr);
    }
  }, [normalizedValue]);

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
