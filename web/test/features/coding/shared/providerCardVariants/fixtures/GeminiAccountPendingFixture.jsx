import './ProviderCardSpacingStubs.js';
import { createRoot } from 'react-dom/client';
import { MemoryRouter } from 'react-router-dom';
import { ConfigProvider } from 'antd';
import i18n from '@/i18n';
import GeminiCliProviderCard from '@/features/coding/geminicli/components/GeminiCliProviderCard';
import '@/App.css';

await i18n.changeLanguage('zh-CN');
document.documentElement.dataset.theme = 'light';

const provider = {
  id: 'fixture-gemini-pending',
  name: 'Synthetic Gemini Official',
  category: 'official',
  settingsConfig: '{"env":{}}',
  isApplied: true,
  isDisabled: false,
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};
const accounts = [
  {
    id: '__local__', providerId: provider.id, name: 'Synthetic local login',
    email: 'pending-local@example.test', kind: 'local', isVirtual: true, isApplied: false,
    createdAt: provider.createdAt, updatedAt: provider.updatedAt,
  },
  {
    id: 'fixture-saved-account', providerId: provider.id, name: 'Synthetic saved login',
    email: 'pending-saved@example.test', kind: 'oauth', isVirtual: false, isApplied: true,
    createdAt: provider.createdAt, updatedAt: provider.updatedAt,
  },
];
const state = { runId: new URLSearchParams(location.search).get('runId'), callbacks: [], unexpectedIPC: [] };
window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    state.unexpectedIPC.push({ command, args });
    throw new Error('Unexpected Gemini pending fixture IPC: ' + command);
  },
};
const noop = () => {};
const record = action => (selectedProvider, account) => {
  // Deliberately no pending guard here: the real card/button must suppress it.
  state.callbacks.push({ action, providerId: selectedProvider.id, accountId: account.id });
};
const targetButton = action => {
  const account = accounts[action === 'refresh' ? 1 : 0];
  const row = [...document.querySelectorAll('[style*="border-bottom"]')]
    .find(node => [...node.querySelectorAll('*')].some(child =>
      !child.children.length && child.textContent.trim() === account.email));
  const label = i18n.t(action === 'refresh' ? 'common.officialAccount.refresh' : 'common.officialAccount.saveCurrent');
  const button = [...(row?.querySelectorAll('button') ?? [])].find(node => node.textContent.trim() === label);
  if (!button) throw new Error('Missing real pending action: ' + action);
  return button;
};

window.geminiAccountPendingFixture = {
  state,
  expandAccounts() {
    const button = [...document.querySelectorAll('button')].find(node => node.textContent.includes('账号列表'));
    if (!button) throw new Error('Missing actual account fold');
    button.click();
  },
  snapshot() {
    return {
      refresh: { label: targetButton('refresh').textContent.trim(), loading: targetButton('refresh').classList.contains('ant-btn-loading') },
      save: { label: targetButton('save').textContent.trim(), loading: targetButton('save').classList.contains('ant-btn-loading') },
    };
  },
  repeatPendingClicks() {
    for (const action of ['refresh', 'save']) {
      targetButton(action).click();
      targetButton(action).click();
    }
  },
};

// These independent pending props are accepted by the real page/card contract.
// No storage, refresh, save service, or component behavior is reimplemented.
createRoot(document.getElementById('root')).render(
  <ConfigProvider>
    <MemoryRouter>
      <main style={{ padding: 16 }}>
        <GeminiCliProviderCard provider={provider} isApplied viewMode="list" dragDisabled
          onEdit={noop} onDelete={noop} onCopy={noop} onShare={noop} onSelect={noop} onToggleDisabled={noop}
          officialAccounts={accounts}
          refreshingOfficialAccountId="fixture-saved-account"
          savingOfficialAccountId="__local__"
          onOfficialAccountRefresh={record('refresh')}
          onOfficialLocalAccountSave={record('save')}
          gatewayStatus={null} gatewayTakeoverActive={false}
        />
      </main>
    </MemoryRouter>
  </ConfigProvider>,
);
