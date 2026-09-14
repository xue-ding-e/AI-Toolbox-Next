import React from 'react';
import { createRoot } from 'react-dom/client';
import { App, Button, ConfigProvider, theme } from 'antd';
import BackupSettingsModal from '@/features/settings/components/BackupSettingsModal';
import RemoteBackupRestoreModal from '@/features/settings/components/RemoteBackupRestoreModal';
import { useSettingsStore } from '@/stores/settingsStore';
import i18n from '@/i18n';
import '@/App.css';

const parameters = new URLSearchParams(location.search);
const mode = parameters.get('theme') || 'light';
const resolvedTheme = mode === 'system'
  ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
  : mode;
document.documentElement.dataset.theme = resolvedTheme;
await i18n.changeLanguage(parameters.get('language') || 'zh-CN');
const state = {
  requests: [],
  closed: false,
  failSave: false,
  deferRepositoryLoad: parameters.has('deferLoad'),
  pendingLoads: [],
  deferFiles: parameters.has('deferFiles'),
  pendingFileLoads: [],
  repository: parameters.has('blank') ? {
    platform: 'github', owner: '', repository: '', branch: '', directory: '',
  } : {
    platform: 'github', owner: 'fixture-owner', repository: 'fixture-repo',
    branch: 'main', directory: '',
  },
  hasToken: !parameters.has('blank'),
  hasPassword: false,
  files: Array.from({ length: 24 }, (_, index) => ({
    filename: 'ai-toolbox-backup-20260914-1200' + String(index).padStart(2, '0')
      + '-abcdef01_Office-with-a-long-host-label' + (index % 2 ? '.zip.enc' : '.zip'),
    size: 1024 * (index + 1),
    sha: 'fixture-sha-' + index,
    encrypted: !!(index % 2),
  })).reverse(),
};
const repositoryView = () => ({ config: structuredClone(state.repository), has_token: state.hasToken });
window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    state.requests.push({ command, args: JSON.parse(JSON.stringify(args)) });
    if (command === 'get_backup_repository_settings') {
      const response = repositoryView();
      if (state.deferRepositoryLoad) {
        state.deferRepositoryLoad = false;
        return new Promise(resolve => state.pendingLoads.push(() => resolve(response)));
      }
      return response;
    }
    if (command === 'get_backup_encryption_status') {
      return { enabled: false, has_password: state.hasPassword, password_known: true };
    }
    if (command === 'list_backup_file_filter_path_options') return [];
    if (command === 'test_backup_repository_connection' || command === 'test_webdav_connection') return;
    if (command === 'save_backup_settings') {
      if (state.failSave) {
        state.failSave = false;
        throw JSON.stringify({ type: 'fixture', suggestion: 'settings.backupSettings.repository.errors.network' });
      }
      const payload = args.payload;
      if (payload.backup_type === 'repository') state.repository = structuredClone(payload.repository);
      if (payload.repository_token) state.hasToken = true;
      if (payload.encryption_password) state.hasPassword = true;
      return {
        repository: repositoryView(),
        encryption: {
          enabled: payload.backup_encryption_enabled,
          has_password: state.hasPassword,
          password_known: true,
        },
      };
    }
    throw new Error('Unexpected fixture command: ' + command);
  },
};
useSettingsStore.setState({
  isInitialized: true,
  backupType: parameters.get('channel') || 'local',
  localBackupPath: 'C:/Fixture/Backups',
  webdav: { url: 'https://dav.example.invalid', username: 'fixture', password: 'fixture-password', remotePath: '/backups', hostLabel: 'Office' },
});
const tick = () => new Promise(resolve => setTimeout(resolve, 50));
const visible = element => element.getClientRects().length && getComputedStyle(element).visibility !== 'hidden';
const setInput = async (element, value) => {
  if (!element) throw new Error('Missing fixture input');
  element.focus();
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(element, value);
  element.dispatchEvent(new Event('input', { bubbles: true }));
  element.blur();
  await tick();
};
window.backupFixture = {
  state,
  async click(key) {
    const label = i18n.t(key).replace(/\s/g, '');
    const button = [...document.querySelectorAll('button')].find(element =>
      visible(element) && element.textContent.replace(/\s/g, '') === label);
    if (!button) throw new Error('Missing button: ' + key);
    button.click();
    await tick();
  },
  async open(which = 'settings') {
    state.closed = false;
    document.querySelector('[data-testid="open-' + which + '"]').click();
    await tick();
  },
  async channel(value) {
    const keys = { local: 'local', webdav: 'webdav', repository: 'repositoryChannel' };
    const label = i18n.t('settings.backupSettings.' + keys[value]);
    const control = [...document.querySelectorAll('[role="radio"]')].find(element => element.textContent === label);
    if (!control) throw new Error('Missing storage channel: ' + value);
    control.click();
    await tick();
  },
  async platform(value) {
    document.querySelector('#repository_platform').dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    await tick();
    const option = [...document.querySelectorAll('.ant-select-item-option')].find(element => element.textContent === value);
    if (!option) throw new Error('Missing platform: ' + value);
    option.click();
    await tick();
  },
  async input(selector, value) { await setInput(document.querySelector(selector), value); },
  async number(key, value) {
    const item = [...document.querySelectorAll('.ant-form-item')].find(element =>
      element.querySelector('label')?.textContent === i18n.t(key));
    await setInput(item?.querySelector('input'), value);
  },
  async toggle(key) {
    const label = i18n.t(key);
    const control = [...document.querySelectorAll('[role="switch"]')].find(element => element.getAttribute('aria-label') === label);
    if (!control) throw new Error('Missing switch: ' + key);
    control.click();
    await tick();
  },
  async resolveLoad() { state.pendingLoads.shift()?.(); await tick(); },
  async resolveFiles() { state.pendingFileLoads.shift()?.(); await tick(); },
  savedRequests() { return state.requests.filter(request => request.command === 'save_backup_settings'); },
  autoSettings() {
    const saved = useSettingsStore.getState();
    return { enabled: saved.autoBackupEnabled, interval: saved.autoBackupIntervalDays, maxKeep: saved.autoBackupMaxKeep };
  },
  canSave() {
    return [...document.querySelectorAll('.ant-modal-footer button')].find(element =>
      element.textContent.replace(/\s/g, '') === i18n.t('common.save').replace(/\s/g, ''))?.disabled === false;
  },
};

function Fixture() {
  const [settingsOpen, setSettingsOpen] = React.useState(false);
  const [remoteOpen, setRemoteOpen] = React.useState(false);
  return <App>
    <Button data-testid="open-settings" onClick={() => setSettingsOpen(true)}>Open settings</Button>
    <Button data-testid="open-remote" onClick={() => setRemoteOpen(true)}>Open backups</Button>
    <BackupSettingsModal open={settingsOpen} onClose={() => { state.closed = true; setSettingsOpen(false); }} />
    <RemoteBackupRestoreModal
      open={remoteOpen}
      onClose={() => setRemoteOpen(false)}
      currentHostLabel=""
      loadFiles={async () => {
        if (state.deferFiles) {
          state.deferFiles = false;
          return new Promise(resolve => state.pendingFileLoads.push(() => resolve(structuredClone(state.files))));
        }
        return structuredClone(state.files);
      }}
      deleteFile={async file => { state.files = state.files.filter(item => item.sha !== file.sha); }}
      onSelect={selection => { state.selection = selection; }}
    />
  </App>;
}
createRoot(document.getElementById('root')).render(
  <ConfigProvider modal={{ centered: true }} theme={{
    algorithm: resolvedTheme === 'dark' ? theme.darkAlgorithm : theme.defaultAlgorithm,
    token: { colorPrimary: '#1890ff' },
  }}>
    <Fixture />
  </ConfigProvider>,
);
