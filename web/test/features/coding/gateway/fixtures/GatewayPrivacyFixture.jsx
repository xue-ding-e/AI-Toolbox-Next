import React from 'react';
import { createRoot } from 'react-dom/client';
import { App, ConfigProvider, theme } from 'antd';
import { ShieldCheck } from 'lucide-react';
import i18n from '@/i18n';
import GatewayPrivacySettings from '@/features/coding/gateway/components/GatewayPrivacySettings';
import settingsStyles from '@/features/settings/pages/GatewaySettingsPanel.module.less';
import '@/App.css';

const parameters = new URLSearchParams(location.search);
const mode = parameters.get('theme') || 'light';
const resolvedTheme = mode === 'system' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : mode;
document.documentElement.dataset.theme = resolvedTheme;
await i18n.changeLanguage(parameters.get('language') || 'zh-CN');
const state = {
  runId: parameters.get('runId'), requests: [], failSave: false, deferPreview: false,
  settings: { enabled: false, rules: { builtins: ['credentials', 'private_keys', 'passwords', 'connection_strings'], custom: [], allowlist: [] } },
};
let pendingPreview;
window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    state.requests.push({ command, args: structuredClone(args) });
    if (command === 'proxy_gateway_get_privacy_settings') return structuredClone(state.settings);
    if (command === 'proxy_gateway_update_privacy_settings') {
      if (state.failSave) { state.failSave = false; throw new Error('Fixture save failed'); }
      if (args.update.rules?.custom.some(rule => !rule.name || !rule.pattern)) throw new Error('Fixture invalid rule');
      state.settings = { ...state.settings, ...structuredClone(args.update) };
      return structuredClone(state.settings);
    }
    if (command === 'proxy_gateway_preview_privacy') {
      if (state.deferPreview) { state.deferPreview = false; return new Promise(resolve => { pendingPreview = resolve; }); }
      return { redacted: 'fixture-placeholder', restored: args.text, detail: { matched_values: 1, restored_values: 1, rules: {}, failed: false, log_redacted: false } };
    }
    throw new Error('Unexpected command: ' + command);
  },
};
const tick = () => new Promise(resolve => setTimeout(resolve, 40));
window.privacyFixture = {
  state,
  async click(key) {
    const label = i18n.t(key);
    const selector = key.includes('.tabs.') ? '[role="tab"]' : 'button';
    const element = [...document.querySelectorAll(selector)].find(element => element.textContent.replace(/\s/g, '') === label.replace(/\s/g, '') && element.offsetParent !== null);
    if (!element) throw new Error('Missing control: ' + key);
    element.click(); await tick();
  },
  async input(selector, value) {
    const element = document.querySelector(selector);
    if (!element) throw new Error('Missing input: ' + selector);
    const prototype = element.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(prototype, 'value').set.call(element, value);
    element.dispatchEvent(new Event('input', { bubbles: true })); await tick();
  },
  async toggle() { document.querySelector('[aria-labelledby="gateway-privacy-toggle-label"]').click(); await tick(); },
  async selectRegex() {
    const input = document.querySelector('.ant-modal [role="combobox"]');
    input.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    await tick();
    const option = [...document.querySelectorAll('.ant-select-item-option')].find(element => element.textContent === i18n.t('gateway.privacy.regex'));
    if (!option) throw new Error('Missing regex option');
    option.click(); await tick();
  },
  async resolvePreview() { pendingPreview({ redacted: 'stale-preview', restored: 'stale-preview', detail: { matched_values: 1 } }); await tick(); },
};

createRoot(document.getElementById('root')).render(
  <ConfigProvider theme={{ algorithm: resolvedTheme === 'dark' ? theme.darkAlgorithm : theme.defaultAlgorithm }}>
    <App><main style={{ maxWidth: 620, margin: '24px auto', padding: 16 }}>
      <section className={settingsStyles.section}>
        <div className={settingsStyles.sectionHeader}>
          <span className={settingsStyles.sectionIcon}><ShieldCheck size={15} aria-hidden="true" /></span>
          <h3>{i18n.t('gateway.privacy.title')}</h3>
        </div>
        <div className={settingsStyles.sectionBody}><GatewayPrivacySettings running /></div>
      </section>
    </main></App>
  </ConfigProvider>,
);
