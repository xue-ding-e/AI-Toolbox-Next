import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';
import ts from 'typescript';

const source = readFileSync(new URL('../../../../../features/coding/codex/components/CodexAuthExport.tsx', import.meta.url), 'utf8');

// Execute the real component and handlers, replacing only rendering and IPC.
// No credentials, host profile, browser clipboard or native application is used.
function fixture(exportAuth: (...args: unknown[]) => Promise<string | null>) {
  const states: unknown[] = [];
  const refs: { current: unknown }[] = [];
  const cleanups: (() => void)[] = [];
  const messages: string[] = [];
  let cursor = 0;
  let refCursor = 0;
  let mounted = false;
  const react = {
    createElement: (type: unknown, props: object | null, ...children: unknown[]) => ({ type, props: { ...props, children } }),
    useState: (initial: unknown) => {
      const i = cursor++;
      if (!(i in states)) states[i] = initial;
      return [states[i], (value: unknown) => { states[i] = value; }];
    },
    useRef: (initial: unknown) => refs[refCursor++] ?? (refs[refCursor - 1] = { current: initial }),
    useEffect: (effect: () => () => void) => { if (!mounted) cleanups.push(effect()); },
  };
  const output = ts.transpileModule(source, { compilerOptions: { jsx: ts.JsxEmit.React, module: ts.ModuleKind.CommonJS, esModuleInterop: true } }).outputText;
  const module = { exports: {} as { default: (props: object) => unknown } };
  vm.runInNewContext(output, {
    exports: module.exports,
    require: (name: string) => {
      if (name === 'react') return react;
      if (name === 'antd') return { Alert: 'Alert', Button: 'Button', Space: 'Space', App: { useApp: () => ({ message: { success: (s: string) => messages.push(s), error: (s: string) => messages.push(s) } }) } };
      if (name === 'lucide-react') return { Copy: 'Copy', Eye: 'Eye', EyeOff: 'EyeOff' };
      if (name === 'react-i18next') return { useTranslation: () => ({ t: (key: string) => key }) };
      if (name.endsWith('PlainTextEditor')) return { __esModule: true, default: 'Editor' };
      if (name.endsWith('codexApi')) return { exportCodexOfficialAccountAuthJson: exportAuth };
      throw new Error(`Unexpected dependency: ${name}`);
    },
  });
  const render = () => {
    cursor = 0; refCursor = 0;
    const tree = module.exports.default({ providerId: 'provider-a', accountId: 'account-a' });
    mounted = true;
    return tree;
  };
  const nodes = (node: any): any[] => !node || typeof node !== 'object' ? []
    : Array.isArray(node) ? node.flatMap(nodes) : [node, ...nodes(node.props?.children)];
  const button = (key: string) => nodes(render()).find((n) => n.type === 'Button' && n.props.children.includes(key));
  return { states, messages, render, button, nodes, close: () => cleanups.forEach((fn) => fn()) };
}
const settle = () => new Promise((resolve) => setImmediate(resolve));
const previewKey = 'codex.provider.authExportPreview';
const copyKey = 'codex.provider.authExportCopy';

test('explicit preview preserves the complete backend payload; hide removes it', async () => {
  const calls: unknown[][] = [];
  const payload = JSON.stringify({ auth_mode: 'chatgpt', $schema: 'https://schema.example.invalid/never-fetch.json', tokens: { access_token: 'synthetic', refresh_token: 'synthetic-refresh', account_id: 'a' }, future: false }, null, 2);
  const f = fixture(async (...args) => { calls.push(args); return payload; });
  f.render(); assert.equal(calls.length, 0);
  f.button(previewKey).props.onClick(); await settle();
  assert.deepEqual(calls, [['provider-a', 'account-a', false]]);
  const editor = f.nodes(f.render()).find((n) => n.type === 'Editor');
  assert.equal(editor.props.value, payload); assert.equal(editor.props.readOnly, true);
  assert.equal(editor.props.language, 'plaintext', 'secret JSON must not enter the JSON schema/language service');
  assert.equal(editor.props.disposeModelOnUnmount, true);
  f.button('codex.provider.authExportHide').props.onClick();
  assert.equal(f.nodes(f.render()).some((n) => n.type === 'Editor'), false);
});

test('copy is explicit and never stores or displays credentials', async () => {
  const calls: unknown[][] = [];
  const f = fixture(async (...args) => { calls.push(args); return null; });
  f.button(copyKey).props.onClick(); await settle();
  assert.deepEqual(calls, [['provider-a', 'account-a', true]]);
  assert.equal(f.states[0], null); assert.deepEqual(f.messages, ['common.copied']);
});

test('duplicate clicks are fenced and late preview after closing is ignored', async () => {
  let resolve!: (value: string) => void;
  let calls = 0;
  const f = fixture(() => { calls++; return new Promise<string>((r) => { resolve = r; }); });
  f.button(previewKey).props.onClick();
  assert.equal(f.button(copyKey).props.disabled, true);
  f.button(copyKey).props.onClick(); assert.equal(calls, 1);
  f.close(); resolve('synthetic-secret'); await settle();
  assert.equal(f.states[0], null); assert.deepEqual(f.messages, []);
});

test('errors do not expose credential payloads and allow retry', async () => {
  const f = fixture(async () => { throw new Error('synthetic-secret'); });
  f.button(previewKey).props.onClick(); await settle();
  assert.deepEqual(f.messages, ['codex.provider.authExportFailed']);
  assert.equal(f.button(previewKey).props.disabled, false);
  assert.equal(f.states[0], null);
});

test('service forwards the selected IDs and explicit preview/copy intent to one registered command', async () => {
  const api = readFileSync(new URL('../../../../../services/codexApi.ts', import.meta.url), 'utf8');
  const compiled = ts.transpileModule(api, { compilerOptions: { module: ts.ModuleKind.CommonJS } }).outputText;
  const exported: Record<string, any> = {};
  const calls: { name: string; input: unknown }[] = [];
  vm.runInNewContext(compiled, {
    exports: exported,
    require: (name: string) => {
      assert.equal(name, '@tauri-apps/api/core');
      return { invoke: async (command: string, input: unknown) => {
        calls.push({ name: command, input }); return 'synthetic-result';
      } };
    },
  });
  for (const copy of [false, true]) {
    assert.equal(await exported.exportCodexOfficialAccountAuthJson('p', 'a', copy), 'synthetic-result');
    const call = calls[calls.length - 1]!;
    assert.equal(call.name, 'export_codex_official_account_auth_json');
    assert.equal(JSON.stringify(call.input), JSON.stringify({ providerId: 'p', accountId: 'a', copyToClipboard: copy }));
  }
  const registry = readFileSync(new URL('../../../../../../tauri/src/lib.rs', import.meta.url), 'utf8');
  assert.ok(registry.includes('coding::codex::export_codex_official_account_auth_json,'));
});

test('real page export mount clears on inactivity/reload and keys each account/root context', () => {
  const page = readFileSync(new URL('../../../../../features/coding/codex/pages/CodexPage.tsx', import.meta.url), 'utf8');
  const ast = ts.createSourceFile('page.tsx', page, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let expression: ts.JsxExpression | undefined;
  function visit(node: ts.Node) {
    if (ts.isJsxExpression(node) && node.expression?.getText(ast).startsWith('isActive && !loading && (') && node.getText(ast).includes('<CodexAuthExport')) expression = node;
    ts.forEachChild(node, visit);
  }
  visit(ast); assert.ok(expression?.expression);
  const code = ts.transpileModule(`const render = (isActive, loading, officialAccountLoadGeneration, officialAccountDetails) => (${expression.expression.getText(ast)}); render;`, { compilerOptions: { jsx: ts.JsxEmit.React } }).outputText;
  const render = vm.runInNewContext(code, {
    React: { createElement: (type: unknown, props: unknown, ...children: unknown[]) => ({ type, props, children }) },
    Descriptions: { Item: 'Item' }, CodexAuthExport: 'Export',
  });
  const details = { provider: { id: 'p' }, account: { id: 'a', accountId: 'workspace', email: 'synthetic@example.invalid', tokenExpiresAt: 123 } };
  assert.equal(render(false, false, { current: 1 }, details), false);
  assert.equal(render(true, true, { current: 1 }, details), false);
  const key = (d = details, generation = 1) => render(true, false, { current: generation }, d).children[0].props.key;
  const original = key();
  assert.notEqual(key(details, 2), original);
  assert.notEqual(key({ ...details, provider: { id: 'other' } }), original);
  assert.notEqual(key({ ...details, account: { ...details.account, id: 'other' } }), original);
  assert.notEqual(key({ ...details, account: { ...details.account, accountId: 'other' } }), original);
});
