import React from 'react';
import { createRoot } from 'react-dom/client';
import { App, ConfigProvider, theme } from 'antd';
import i18n from '@/i18n';
import { useAppStore } from '@/stores/appStore';
import { updatePresetModels } from '@/constants/presetModels';
import { updateGatewayProviderProfiles } from '@/features/coding/shared/gateway/providerProfiles';
import CodexProviderFormModal from '@/features/coding/codex/components/CodexProviderFormModal';
import presets from '../../../../../../../tauri/resources/preset_models.json';
import gatewayProfiles from '../../../../../../../tauri/resources/gateway_provider_profiles.json';
import '@/App.css';

const parameters = new URLSearchParams(location.search);
const scenario = parameters.get('scenario') || 'custom';
const language = parameters.get('language') || 'zh-CN';
const themeMode = parameters.get('theme') || 'light';
const resolvedTheme = themeMode === 'system'
  ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
  : themeMode;
await i18n.changeLanguage(language);
useAppStore.setState({ language });
document.documentElement.dataset.theme = resolvedTheme;
updatePresetModels(presets);
updateGatewayProviderProfiles(gatewayProfiles);

const builtinProfile = gatewayProfiles.profiles.find(profile => profile.tools.codex?.endpoints.some(endpoint => endpoint.modelCatalog?.models?.length));
const builtinEndpoint = builtinProfile.tools.codex.endpoints.find(endpoint => endpoint.modelCatalog?.models?.length);
const existingModel = {
  model: 'gpt-6-astra', displayName: 'My GPT', contextWindow: 64000, reasoningLevels: ['low'],
  defaultReasoningLevel: 'low', serviceTiers: ['priority'], supportsImage: false,
};
const initialModels = scenario === 'builtin' ? builtinEndpoint.modelCatalog.models
  : scenario === 'preset' || scenario === 'manual' ? [] : [existingModel];
const createProvider = (apiFormat = 'openai_responses') => ({
  id: 'model-import-fixture', name: 'Model import regression', category: 'custom',
  settingsConfig: JSON.stringify({
    auth: { OPENAI_API_KEY: 'fixture-key' },
    config: 'model_provider = "custom"\nmodel = "default-model"\n[model_providers.custom]\nname = "Fixture"\nwire_api = "responses"\nbase_url = "'
      + (apiFormat === 'gemini_native' ? 'https://models.example.test' : 'https://models.example.test/v1') + '"',
    modelCatalog: { models: initialModels },
  }),
  meta: scenario === 'builtin'
    ? { gatewayProfile: { tool: 'codex', profileId: builtinProfile.id, endpointId: builtinEndpoint.id } }
    : { apiFormat },
});
const state = {
  scenario, runId: parameters.get('runId'), requests: [], saved: null,
  models: [
    { id: 'model-alpha', name: 'API Alpha', ownedBy: 'openai' },
    { id: 'model-beta', name: 'API Beta', ownedBy: 'openai' },
    { id: 'claude-opus-4-8', name: 'claude-opus-4-8', ownedBy: 'anthropic' },
    ...initialModels.map(model => ({ id: model.model, ownedBy: 'openai' })),
  ],
};
let pendingFetch;
let deferNextFetch = false;
let nextFetchError;
window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    state.requests.push({ command, args: structuredClone(args) });
    if (command === 'read_opencode_config') return { status: 'success', config: { provider: {} } };
    if (command === 'fetch_provider_models') {
      if (nextFetchError) { const error = nextFetchError; nextFetchError = undefined; throw new Error(error); }
      if (deferNextFetch) {
        deferNextFetch = false;
        return new Promise(resolve => { pendingFetch = resolve; });
      }
      return { models: structuredClone(state.models), total: state.models.length };
    }
    throw new Error('Unexpected fixture command: ' + command);
  },
};

const wait = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const visible = element => element.getBoundingClientRect().height > 0 && getComputedStyle(element).visibility !== 'hidden';
const visibleElements = selector => Array.from(document.querySelectorAll(selector)).filter(visible);
const mappingInputs = () => visibleElements('input').filter(input => input.getAttribute('aria-label') === i18n.t('codex.provider.modelMappingModel'));
const findButton = (key, options) => visibleElements('button').find(button => button.textContent.replace(/\s/g, '') === String(i18n.t(key, options)).replace(/\s/g, ''));
const setInput = (input, value) => {
  if (!input) throw new Error('Fixture input was not found');
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true }));
};
const fixture = window.modelImportFixture = {
  state,
  async click(key, options) {
    const button = findButton(key, options);
    if (!button || button.disabled) throw new Error('Available button was not found: ' + key);
    button.click(); await wait(180);
  },
  async select(modelId) {
    const checkbox = visibleElements('tr[data-row-key]').find(row => row.dataset.rowKey === modelId)?.querySelector('input[type=checkbox]');
    if (!checkbox || checkbox.disabled) throw new Error('Selectable model was not found: ' + modelId);
    checkbox.click(); await wait(100);
  },
  isModelDisabled(modelId) {
    return visibleElements('tr[data-row-key]').find(row => row.dataset.rowKey === modelId)?.querySelector('input[type=checkbox]')?.disabled;
  },
  async search(value) {
    setInput(visibleElements('input').find(input => input.placeholder === i18n.t('opencode.fetchModels.searchPlaceholder')), value);
    await wait(100);
  },
  async editMapping(index, value) { setInput(mappingInputs()[index], value); await wait(150); },
  async removeAllMappings() {
    let button;
    while ((button = visibleElements('button').find(item => item.getAttribute('aria-label') === i18n.t('codex.provider.modelMappingRemove')))) {
      button.click(); await wait(80);
    }
  },
  async setDiscoveryMode(mode) {
    const radio = visibleElements('input[type=radio]').find(input => input.value === mode);
    if (!radio) throw new Error('Discovery mode was not found: ' + mode);
    radio.click(); await wait(150);
  },
  async optIntoCleanup() {
    const checkbox = visibleElements('.ant-modal').at(-1)?.querySelector('input[type=checkbox]:not(.ant-table input)');
    if (!checkbox) throw new Error('Cleanup checkbox was not found');
    checkbox.click(); await wait(100);
  },
  async confirmImport() {
    const button = visibleElements('.ant-modal').at(-1)?.querySelector('.ant-modal-footer button.ant-btn-primary');
    if (!button || button.disabled) throw new Error('Import confirmation is disabled');
    button.click(); await wait(220);
  },
  async cancelImport() {
    visibleElements('.ant-modal').at(-1).querySelector('.ant-modal-close').click(); await wait(350);
  },
  mappingModels: () => mappingInputs().map(input => input.value),
  listedModels: () => visibleElements('tr[data-row-key]').map(row => row.dataset.rowKey),
  submittedSettings: () => state.saved ? JSON.parse(state.saved.settingsConfig) : undefined,
  setModels: models => { state.models = models; },
  deferNextFetch: () => { deferNextFetch = true; },
  failNextFetch: message => { nextFetchError = message; },
  async resolvePending(models) { pendingFetch({ models, total: models.length }); await wait(160); },
};

function FixtureApp() {
  const [open, setOpen] = React.useState(true);
  const [provider, setProvider] = React.useState(() => createProvider(
    scenario === 'google' ? 'gemini_native' : scenario === 'anthropic' ? 'anthropic_messages' : 'openai_responses',
  ));
  fixture.reopenSaved = async () => { setProvider(structuredClone(state.saved)); setOpen(true); await wait(250); };
  fixture.switchProtocol = async apiFormat => { setProvider(createProvider(apiFormat)); setOpen(true); await wait(250); };
  return (
    <ConfigProvider theme={{ algorithm: resolvedTheme === 'dark' ? theme.darkAlgorithm : theme.defaultAlgorithm }}>
      <App>
        <CodexProviderFormModal open={open} provider={provider} onCancel={() => setOpen(false)} onSubmit={async values => {
          state.saved = structuredClone({ ...provider, ...values });
        }} />
      </App>
    </ConfigProvider>
  );
}
createRoot(document.getElementById('root')).render(<FixtureApp />);
