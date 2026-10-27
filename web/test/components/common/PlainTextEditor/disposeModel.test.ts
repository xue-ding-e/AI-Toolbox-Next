import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';
import ts from 'typescript';

const source = readFileSync(new URL('../../../../components/common/PlainTextEditor/index.tsx', import.meta.url), 'utf8');
const output = ts.transpileModule(source, {
  compilerOptions: { jsx: ts.JsxEmit.ReactJSX, module: ts.ModuleKind.CommonJS, esModuleInterop: true },
}).outputText;
const exported: Record<string, any> = {};
vm.runInNewContext(output, {
  exports: exported,
  require: (name: string) => {
    if (name === 'react/jsx-runtime') return { jsx: (type: unknown, props: unknown) => ({ type, props }) };
    if (name === 'react-monaco-editor') return { __esModule: true, default: 'Monaco' };
    if (name === '@/stores/themeStore') return { useThemeStore: () => ({ resolvedTheme: 'dark' }) };
    throw new Error(`Unexpected import ${name}`);
  },
});
const editorProps = (disposeModelOnUnmount?: boolean) => exported.default({
  value: 'synthetic-secret', language: 'plaintext', disposeModelOnUnmount,
}).props.children.props.children.props;

test('opt-in callback disposes its private model on each unmount reason', () => {
  // The same actual callback is called by react-monaco-editor cleanup for all
  // unmounts. Parent lifecycle gates are covered by CodexAuthExport.test.ts.
  for (const reason of ['hide', 'close', 'account/root key change']) {
    let disposed = false;
    let calls = 0;
    const model = { isDisposed: () => disposed, dispose: () => { disposed = true; calls++; } };
    const props = editorProps(true);
    assert.equal(props.uri, undefined, 'no shared model URI');
    assert.equal(props.options.model, undefined, 'no external model');
    props.editorWillUnmount({ getModel: () => model });
    assert.equal(calls, 1, reason);
    props.editorWillUnmount({ getModel: () => model });
    assert.equal(calls, 1, 'already disposed models are not disposed twice');
  }
});

test('default consumers remain unchanged and null models are safe', () => {
  let reads = 0;
  editorProps().editorWillUnmount({ getModel: () => { reads++; return null; } });
  assert.equal(reads, 0);
  assert.doesNotThrow(() => editorProps(true).editorWillUnmount({ getModel: () => null }));
});
