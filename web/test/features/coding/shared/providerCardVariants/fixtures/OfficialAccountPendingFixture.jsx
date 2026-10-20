import './ProviderCardSpacingStubs.js';
import React from 'react';
import { createRoot } from 'react-dom/client';
import { ConfigProvider } from 'antd';
import i18n from '@/i18n';
import { OfficialAccountsSection } from '@/features/coding/shared/officialAccounts';
import '@/App.css';

await i18n.changeLanguage('zh-CN');
document.documentElement.dataset.theme = 'light';

const accounts = [
  { id: 'local', label: 'synthetic-local@example.test', isVirtual: true, isApplied: false },
  { id: 'saved', label: 'synthetic-saved@example.test', isVirtual: false, isApplied: false },
];
const cases = {
  'legacy-null': null,
  'legacy-save': { accountId: 'local', action: 'save' },
  'legacy-apply': { accountId: 'saved', action: 'apply' },
  'legacy-delete': { accountId: 'saved', action: 'delete' },
  'array-different-rows': [
    { accountId: 'saved', action: 'refresh' }, { accountId: 'local', action: 'save' },
  ],
  'array-same-row': [
    { accountId: 'local', action: 'refresh' }, { accountId: 'local', action: 'save' },
  ],
  'array-save-only': [{ accountId: 'local', action: 'save' }],
  'array-empty': [],
};
const state = {
  runId: new URLSearchParams(location.search).get('runId'),
  activeCase: null, callbacks: [], unexpectedIPC: [],
};
window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    state.unexpectedIPC.push({ command, args });
    throw new Error('Unexpected shared account-pending fixture IPC: ' + command);
  },
};
const record = action => account => {
  // No guard or state mutation: only the real section/Button can suppress clicks.
  state.callbacks.push({ accountId: account.id, action });
};
const labels = {
  refresh: 'common.officialAccount.refresh', save: 'common.officialAccount.saveCurrent',
  apply: 'common.officialAccount.switch', delete: 'common.delete',
};
const targets = [
  ['local', 'refresh'], ['local', 'save'], ['saved', 'refresh'], ['saved', 'apply'], ['saved', 'delete'],
];
const button = (accountId, action) => {
  const account = accounts.find(item => item.id === accountId);
  const row = [...document.querySelectorAll('[data-pending-section] [style*="border-bottom"]')]
    .find(node => [...node.querySelectorAll('*')].some(child =>
      !child.children.length && child.textContent.trim() === account.label));
  const matches = [...(row?.querySelectorAll('button') ?? [])]
    .filter(node => node.textContent.trim() === i18n.t(labels[action]));
  if (matches.length !== 1) throw new Error('Expected one actual account button: ' + accountId + '/' + action);
  return matches[0];
};
window.officialAccountPendingFixture = {
  state,
  selectCase(name) {
    const control = [...document.querySelectorAll('[data-pending-case]')]
      .find(node => node.dataset.pendingCase === name);
    if (!control) throw new Error('Unknown pending case: ' + name);
    control.click();
  },
  resetCallbacks() { state.callbacks.length = 0; },
  snapshot() {
    return targets.map(([accountId, action]) => ({
      accountId, action, loading: button(accountId, action).classList.contains('ant-btn-loading'),
    }));
  },
  repeatClicks(entries) {
    for (const { accountId, action } of entries) {
      button(accountId, action).click();
      button(accountId, action).click();
    }
  },
};

function FixtureHost() {
  const [activeCase, setActiveCase] = React.useState('legacy-null');
  React.useLayoutEffect(() => { state.activeCase = activeCase; }, [activeCase]);
  return (
    <ConfigProvider>
      <main style={{ padding: 16 }}>
        <nav>
          {Object.keys(cases).map(name => (
            <button key={name} data-pending-case={name} onClick={() => setActiveCase(name)}>{name}</button>
          ))}
        </nav>
        <section data-pending-section>
          <OfficialAccountsSection variant="embedded" listTitle="Synthetic accounts" emptyText="No synthetic accounts"
            accounts={accounts} pending={cases[activeCase]}
            onRefresh={record('refresh')} onSaveLocal={record('save')}
            onApply={record('apply')} onDelete={record('delete')} />
        </section>
      </main>
    </ConfigProvider>
  );
}
createRoot(document.getElementById('root')).render(<FixtureHost />);
