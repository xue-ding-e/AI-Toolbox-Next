import React from 'react';
import { createRoot } from 'react-dom/client';
import { App, ConfigProvider, theme } from 'antd';
import GeneralSettingsPage from '@/features/settings/pages/GeneralSettingsPage';
import { useSettingsStore } from '@/stores/settingsStore';
import { useAppStore } from '@/stores/appStore';
import { useThemeStore } from '@/stores/themeStore';
import i18n from '@/i18n';
import {
  fixtureData, installTauriBoundary, boundarySnapshot,
  setLocalSelection, replaceSelectionSources, settleRestore,
} from './tauri.mjs';
import '@/App.css';

const parameters = new URLSearchParams(location.search);
const mode = parameters.get('theme') || 'light';
document.documentElement.dataset.theme = mode;
await i18n.changeLanguage(parameters.get('language') || 'en-US');
installTauriBoundary();
// Seed a local starting point. Subsequent channel changes go through the real
// page's settings dialog, saveBackupSettingsUnified and backupSettingsStatePatch.
useSettingsStore.setState({
  isInitialized: true, backupType: 'local', localBackupPath: fixtureData.localDirectory,
  webdav: structuredClone(fixtureData.webdav), autoCheckUpdate: false,
});
useAppStore.setState({ isInitialized: true, language: i18n.language, languagePreference: i18n.language });
useThemeStore.setState({ isInitialized: true, mode, resolvedTheme: mode });

const visible = element => !!element && element.getClientRects().length > 0
  && getComputedStyle(element).visibility !== 'hidden';
const text = key => i18n.t(key);
const compact = value => value.replace(/\s/g, '');
const dialogs = () => [...document.querySelectorAll('.ant-modal')].filter(visible);
const dialog = key => dialogs().find(element => {
  const title = element.querySelector('.ant-modal-title, .ant-modal-confirm-title');
  return title?.textContent === text(key);
});
const findButton = (root, key) => [...(root?.querySelectorAll('button') || [])].find(element =>
  visible(element) && compact(element.textContent) === compact(text(key)));
const clickButton = (root, key) => {
  const button = findButton(root, key);
  if (!button || button.disabled) throw new Error('Missing or disabled button: ' + key);
  button.scrollIntoView({ block: 'center' });
  button.click();
};

window.settingsRestoreFixture = {
  // These are DOM drivers and service-boundary controls, not page callbacks.
  // The fixture does not import/call any restore helper or backup service.
  text,
  dialog,
  setLocalSelection,
  replaceSelectionSources,
  settleRestore,
  clickPage(key) { clickButton(document.getElementById('root'), key); },
  clickDialog(titleKey, buttonKey) { clickButton(dialog(titleKey), buttonKey); },
  selectChannel(channel) {
    const keys = { local: 'local', webdav: 'webdav', repository: 'repositoryChannel' };
    const label = text('settings.backupSettings.' + keys[channel]);
    const control = [...(dialog('settings.backupSettings.title')?.querySelectorAll('[role="radio"]') || [])]
      .find(element => element.textContent === label);
    if (!control || control.getAttribute('aria-disabled') === 'true') throw new Error('Missing channel: ' + channel);
    control.click();
  },
  selectRemote(filename) {
    const row = [...(dialog('settings.backupSettings.selectBackupFile')?.querySelectorAll('.ant-list-item') || [])]
      .find(element => visible(element) && element.querySelector('.ant-list-item-meta-description')?.textContent === filename);
    if (!row) throw new Error('Missing visible backup row: ' + filename);
    row.scrollIntoView({ block: 'center' });
    row.click();
  },
  closeRemote() {
    const close = dialog('settings.backupSettings.selectBackupFile')?.querySelector('.ant-modal-close');
    if (!close) throw new Error('Missing remote list close button');
    close.click();
  },
  toggleOption() {
    const checkbox = dialog('settings.backupSettings.confirmRestore')?.querySelector('input[type="checkbox"]');
    if (!checkbox) throw new Error('Missing restore option');
    checkbox.click();
  },
  focusPassword() {
    const input = dialog('settings.backupSettings.encryption.passwordPromptTitle')?.querySelector('input[type="password"]');
    if (!input) throw new Error('Missing password input');
    input.focus();
  },
  snapshot() {
    const settings = dialog('settings.backupSettings.title');
    const confirmation = dialog('settings.backupSettings.confirmRestore');
    const password = dialog('settings.backupSettings.encryption.passwordPromptTitle');
    const success = dialog('settings.backupSettings.restoreSuccess');
    const restore = findButton(document.getElementById('root'), 'settings.backupSettings.restoreBackup');
    const save = findButton(settings, 'common.save');
    return {
      ...boundarySnapshot(), channel: useSettingsStore.getState().backupType,
      mounted: !!restore,
      canSave: !!save && !save.disabled,
      dialogs: dialogs().length,
      confirmOpen: !!confirmation,
      option: confirmation?.querySelector('input[type="checkbox"]')?.checked ?? null,
      passwordOpen: !!password,
      password: password?.querySelector('input[type="password"]')?.value ?? null,
      wrongPassword: password?.querySelector('.ant-typography-danger')?.textContent ?? null,
      successOpen: !!success,
      restoreLoading: !!restore?.classList.contains('ant-btn-loading'),
      remoteFiles: [...(dialog('settings.backupSettings.selectBackupFile')?.querySelectorAll('.ant-list-item') || [])]
        .filter(visible).map(element => element.querySelector('.ant-list-item-meta-description')?.textContent),
    };
  },
};

const providerProps = { theme: { algorithm: mode === 'dark' ? theme.darkAlgorithm : theme.defaultAlgorithm } };
ConfigProvider.config({ holderRender: children => React.createElement(ConfigProvider, providerProps, children) });
createRoot(document.getElementById('root')).render(
  React.createElement(ConfigProvider, providerProps,
    React.createElement(App, null, React.createElement(GeneralSettingsPage))),
);
