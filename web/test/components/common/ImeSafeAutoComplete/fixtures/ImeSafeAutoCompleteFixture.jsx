import React from 'react';
import { createRoot } from 'react-dom/client';
import { AutoComplete, ConfigProvider, theme } from 'antd';
import ImeSafeAutoComplete from '@/components/common/ImeSafeAutoComplete';

const parameters = new URLSearchParams(location.search);
const mode = parameters.get('theme') || 'light';
const resolvedTheme = mode === 'system' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : mode;
document.documentElement.dataset.theme = resolvedTheme;

// The wrapper must never reach for the Tauri backend; importing it is enough to
// prove the component stays a pure antd composition.
window.__TAURI_INTERNALS__ = {
  invoke: async command => { throw new Error('Unexpected command: ' + command); },
};

// Nothing else in the fixture mutates the inputs: the browser checks drive the
// DOM directly through `window.imeSafeAutoCompleteFixture`, so every recorded
// change was produced by the component under test.
const state = {
  runId: parameters.get('runId'),
  changes: { plain: [], guarded: [] },
};

const controls = {};

function Fixture() {
  const [values, setValues] = React.useState({ plain: '', guarded: '' });
  state.values = values;

  const recordChange = kind => value => {
    state.changes[kind].push(value);
    setValues(previous => ({ ...previous, [kind]: value }));
  };
  controls.plain = recordChange('plain');
  controls.guarded = recordChange('guarded');

  return (
    <main style={{ maxWidth: 520, margin: '12px auto', padding: 12, display: 'grid', gap: 12 }}>
      <section id="fixture-plain">
        <label htmlFor="plain-input">plain AutoComplete</label>
        <AutoComplete id="plain-input" value={values.plain} onChange={recordChange('plain')} options={[]} />
      </section>
      <section id="fixture-guarded">
        <label htmlFor="guarded-input">guarded AutoComplete</label>
        <ImeSafeAutoComplete id="guarded-input" value={values.guarded} onChange={recordChange('guarded')} options={[]} />
      </section>
      <pre id="fixture-state">{JSON.stringify(state.changes)}</pre>
    </main>
  );
}

createRoot(document.getElementById('root')).render(
  <ConfigProvider theme={{ algorithm: resolvedTheme === 'dark' ? theme.darkAlgorithm : theme.defaultAlgorithm }}>
    <Fixture />
  </ConfigProvider>,
);

// Writing `.value` directly would be swallowed by React's value tracker; going
// through the native setter reproduces a real keystroke's `input` event.
const setInputValue = (selector, value) => {
  const input = document.querySelector(selector);
  if (!input) throw new Error('Missing input: ' + selector);
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, value);
  input.dispatchEvent(new InputEvent('input', { bubbles: true }));
};

const dispatchComposition = (selector, type, data) => {
  const input = document.querySelector(selector);
  if (!input) throw new Error('Missing input: ' + selector);
  input.dispatchEvent(new CompositionEvent(type, { data, bubbles: true }));
};

window.imeSafeAutoCompleteFixture = {
  state,
  focus: selector => document.querySelector(selector).focus(),
  setInputValue,
  dispatchComposition,
  inputValue: selector => document.querySelector(selector).value,
};
