import React from 'react';
import { createRoot } from 'react-dom/client';
import { AutoComplete, ConfigProvider, Form, Input, theme } from 'antd';
import ImeSafeAutoComplete from '@/components/common/ImeSafeAutoComplete';
import ImeSafeInput from '@/components/common/ImeSafeInput';

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
  changes: { plain: [], guarded: [], plainField: [], guardedField: [] },
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
        <AutoComplete id="plain-input" placeholder="fixture placeholder" value={values.plain} onChange={recordChange('plain')} options={[]} />
      </section>
      <section id="fixture-guarded">
        <label htmlFor="guarded-input">guarded AutoComplete</label>
        <ImeSafeAutoComplete id="guarded-input" placeholder="fixture placeholder" value={values.guarded} onChange={recordChange('guarded')} options={[]} />
      </section>
      {/* `size` must reach the custom input: with a customized input antd
          ignores it on AutoComplete, and a dense call site would grow back to
          the default height. */}
      <section id="fixture-plain-small">
        <label htmlFor="plain-small-input">plain AutoComplete (small)</label>
        <AutoComplete id="plain-small-input" size="small" value={values.plain} onChange={recordChange('plain')} options={[]} />
      </section>
      <section id="fixture-guarded-small">
        <label htmlFor="guarded-small-input">guarded AutoComplete (small)</label>
        <ImeSafeAutoComplete id="guarded-small-input" size="small" value={values.guarded} onChange={recordChange('guarded')} options={[]} />
      </section>

      {/* The second shape the guard covers: a plain form field whose value is
          written back from outside on every keystroke. Both scenarios carry the
          identical form wiring — only the input component differs. */}
      <FieldScenario kind="plainField" guarded={false} inputId="plain-field-input" label="plain form field (write-back)" />
      <FieldScenario kind="guardedField" guarded inputId="guarded-field-input" label="guarded form field (write-back)" />

      <pre id="fixture-state">{JSON.stringify(state.changes)}</pre>
    </main>
  );
}

// The Codex base-url loop reduced to its essence: every keystroke sanitizes the
// typed text into state, and an effect writes that state back into the very field
// being typed in. The sanitizer matches `useCodexConfigState`.
const sanitizeFieldValue = raw => raw.replace(/['"`]/g, '').trim();

function FieldScenario({ kind, guarded, inputId, label }) {
  const [form] = Form.useForm();
  const [sanitized, setSanitized] = React.useState('');

  React.useEffect(() => {
    form.setFieldsValue({ url: sanitized });
  }, [sanitized, form]);

  return (
    <Form
      form={form}
      layout="vertical"
      onValuesChange={changed => {
        state.changes[kind].push(changed.url);
        setSanitized(sanitizeFieldValue(changed.url ?? ''));
      }}
    >
      <Form.Item name="url" label={label}>
        {guarded
          ? <ImeSafeInput id={inputId} placeholder="https://example.com/v1" />
          : <Input id={inputId} placeholder="https://example.com/v1" />}
      </Form.Item>
    </Form>
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
