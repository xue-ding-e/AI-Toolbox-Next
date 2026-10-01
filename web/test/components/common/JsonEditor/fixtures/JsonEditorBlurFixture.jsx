import React from 'react';
import { createRoot } from 'react-dom/client';
import { ConfigProvider, theme } from 'antd';
import i18n from '@/i18n';
import JsonEditor from '@/components/common/JsonEditor';
import JsoncEditor from '@/components/common/JsoncEditor';
import TomlEditor from '@/components/common/TomlEditor';
import MarkdownEditor from '@/components/common/MarkdownEditor';
import '@/App.css';

const parameters = new URLSearchParams(location.search);
const mode = parameters.get('theme') || 'light';
const resolvedTheme = mode === 'system' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : mode;
document.documentElement.dataset.theme = resolvedTheme;
await i18n.changeLanguage(parameters.get('language') || 'zh-CN');

// Every Monaco-based editor registers its blur listener once, when the editor
// mounts. The checks change the callback identity afterwards and verify that a
// blur dispatches the *current* prop instead of the mount-time one (issue #406).
const EDITORS = {
  json: { value: { permission: { external_directory: { '*': 'deny' } } } },
  jsonc: { value: '{\n  "permission": true\n}' },
  toml: { value: '[env]\nKEEP = "old"\n' },
  markdown: { value: '# Title\n' },
};

const state = {
  runId: parameters.get('runId'),
  blurs: Object.fromEntries(Object.keys(EDITORS).map(name => [name, []])),
  // Mirror of the latest committed render, only for the checks to wait on. The
  // callbacks below read the render-scope value, so a frozen mount-time closure
  // keeps the revision of its own render (mutating this object in place would
  // hide exactly the staleness the checks look for).
  revisions: Object.fromEntries(Object.keys(EDITORS).map(name => [name, 1])),
};

window.__TAURI_INTERNALS__ = {
  invoke: async command => { throw new Error('Unexpected command: ' + command); },
};

const controls = {};

const editorProps = (name, onBlur) => {
  const common = {
    onBlur,
    height: 140,
    minHeight: 120,
    maxHeight: 300,
    resizable: false,
    placeholder: 'fixture',
  };
  if (name === 'json') return { ...common, value: EDITORS.json.value, mode: 'text' };
  if (name === 'jsonc') return { ...common, value: EDITORS.jsonc.value };
  if (name === 'toml') return { ...common, value: EDITORS.toml.value };
  return { ...common, value: EDITORS.markdown.value };
};

const EditorOf = ({ name }) => {
  if (name === 'json') return JsonEditor;
  if (name === 'jsonc') return JsoncEditor;
  if (name === 'toml') return TomlEditor;
  return MarkdownEditor;
};

function Fixture() {
  const [revisions, setRevisions] = React.useState({ ...state.revisions });
  state.revisions = revisions;

  controls.revise = (name, nextRevision) => {
    setRevisions(previous => ({ ...previous, [name]: nextRevision }));
  };

  return (
    <main style={{ maxWidth: 720, margin: '12px auto', padding: 12 }}>
      {Object.keys(EDITORS).map(name => {
        const Editor = EditorOf({ name });
        // A new callback identity per revision, like the pages whose config
        // state changes after the editor was mounted.
        const onBlur = (...args) => {
          state.blurs[name].push({ revision: revisions[name], args });
        };
        return (
          <section key={name} id={'fixture-' + name}>
            <button type="button" id={'fixture-outside-' + name}>click outside the {name} editor</button>
            <Editor {...editorProps(name, onBlur)} />
          </section>
        );
      })}
      <pre id="fixture-state">{JSON.stringify({ revisions, blurs: Object.fromEntries(Object.keys(state.blurs).map(name => [name, state.blurs[name].length])) })}</pre>
    </main>
  );
}

createRoot(document.getElementById('root')).render(
  <ConfigProvider theme={{ algorithm: resolvedTheme === 'dark' ? theme.darkAlgorithm : theme.defaultAlgorithm }}>
    <Fixture />
  </ConfigProvider>,
);

const focusTarget = name => {
  const section = document.getElementById('fixture-' + name);
  if (!section) throw new Error('Missing fixture section: ' + name);
  return section.querySelector('.monaco-editor .native-edit-context')
    ?? section.querySelector('.monaco-editor textarea');
};

window.jsonEditorBlurFixture = {
  state,
  revise: (name, nextRevision) => controls.revise(name, nextRevision),
  editorIsFocused: name => document.activeElement === focusTarget(name),
  focusEditor: name => {
    const target = focusTarget(name);
    if (!target) throw new Error('Monaco focus target is missing for ' + name);
    target.focus();
  },
  blurEditor: name => {
    const outside = document.getElementById('fixture-outside-' + name);
    if (!outside) throw new Error('Outside element is missing for ' + name);
    outside.focus();
  },
};
