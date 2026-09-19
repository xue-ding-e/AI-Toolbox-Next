import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const readSource = (relativePath: string) =>
  readFileSync(new URL(relativePath, import.meta.url), 'utf8');

const mainLayoutSource = readSource('../../../../components/layout/MainLayout/index.tsx');
const generalSettingsSource = readSource(
  '../../../../features/settings/pages/GeneralSettingsPage.tsx',
);
const settingsApiSource = readSource('../../../../services/settingsApi.ts');
const backendSettingsSource = readSource('../../../../../tauri/src/settings/types.rs');
const backendAdapterSource = readSource('../../../../../tauri/src/settings/adapter.rs');
const zhLocale = JSON.parse(readSource('../../../../i18n/locales/zh-CN.json')) as {
  subModules: Record<string, string>;
};
const enLocale = JSON.parse(readSource('../../../../i18n/locales/en-US.json')) as {
  subModules: Record<string, string>;
};

/**
 * The embedded browser has no route of its own, so `visibleTabs` is the only gate
 * on its toolbar entry. It is an opt-in tab: absent from the backend default
 * `visible_tabs` and from the frontend default settings, so a fresh install shows
 * no browser entry until the user turns it on in 设置 -> 模块显示（右侧）.
 */
test('the browser toolbar entry is opt-in and gated by visibleTabs', () => {
  assert.match(
    mainLayoutSource,
    /const isMiniBrowserVisible = visibleTabs\.includes\('miniBrowser'\);/,
  );
  assert.match(mainLayoutSource, /\{isMiniBrowserVisible && \(/);

  // Default hidden: neither default set may ship the key.
  const frontendDefault = settingsApiSource.match(/visible_tabs: \[([^\]]*)\]/)?.[1] ?? '';
  assert.notEqual(frontendDefault, '');
  assert.doesNotMatch(frontendDefault, /miniBrowser/);
  // The backend default and every historical baseline must stay free of the key:
  // adding it to either would make the tab visible on a fresh install.
  assert.doesNotMatch(backendSettingsSource, /miniBrowser/);
  assert.doesNotMatch(backendAdapterSource, /miniBrowser/);

  // The right-side toggle row exposes the key, and both locales label it.
  assert.match(generalSettingsSource, /const OTHER_TABS = \['miniBrowser',/);
  assert.equal(zhLocale.subModules.miniBrowser, '浏览器');
  assert.equal(enLocale.subModules.miniBrowser, 'Browser');
});
