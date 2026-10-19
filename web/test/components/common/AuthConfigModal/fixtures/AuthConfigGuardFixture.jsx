import React from 'react';
import { createRoot } from 'react-dom/client';
import { App, ConfigProvider, theme } from 'antd';
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api';
import AuthConfigModal from '@/components/common/AuthConfigModal';
import { createAuthConfigMode } from '@/features/coding/opencode/utils/authConfigMode';
import i18n from '@/i18n';
import '@/App.css';

const parameters = new URLSearchParams(location.search);
const dark = parameters.get('theme') === 'dark';
document.documentElement.dataset.theme = dark ? 'dark' : 'light';
await i18n.changeLanguage('en-US');
window.__TAURI_INTERNALS__ = { invoke: async command => { throw new Error('Unexpected IPC: ' + command); } };

// The real shared modal, Monaco editor and production page mode controller.
// Only backend answers and the final write are synthetic; no user paths exist.
const mode = createAuthConfigMode();
const pending = new Map();
const state = { saved: 0, refreshed: 0, closed: 0, settled: [], submitted: null, original: null };
const controls = {};
window.authGuard = {
  state,
  begin(name) {
    let resolve, reject;
    const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
    pending.set(name, { resolve, reject });
    void mode.run(() => promise).catch(() => {}).finally(() => state.settled.push(name));
  },
  resolve(name, enabled) { pending.get(name).resolve(enabled); },
  reject(name) { pending.get(name).reject(new Error('synthetic mode failure')); },
  open: () => controls.open(),
  editorReadOnly: () => monaco.editor.getEditors()[0]?.getOption(monaco.editor.EditorOption.readOnly),
  edit: text => monaco.editor.getEditors()[0].getModel().setValue(text),
};

function Fixture() {
  React.useSyncExternalStore(mode.subscribe, mode.getSnapshot);
  const [open, setOpen] = React.useState(true);
  controls.open = () => setOpen(true);
  return <AuthConfigModal
    open={open}
    content={'{"synthetic":{"key":"original"}}'}
    path="/synthetic/auth.json"
    readOnly={mode.readOnly()}
    onSave={(submitted, original) => mode.save(async () => {
      state.saved++;
      state.submitted = submitted;
      state.original = original;
    })}
    onCancel={() => { state.closed++; setOpen(false); }}
    onSaved={() => { state.refreshed++; }}
  />;
}

createRoot(document.getElementById('root')).render(
  <ConfigProvider theme={{ algorithm: dark ? theme.darkAlgorithm : theme.defaultAlgorithm }}>
    <App><Fixture /></App>
  </ConfigProvider>,
);
