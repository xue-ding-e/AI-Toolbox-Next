import { boundary } from './boundary.mjs';
import React from 'react';
import { createRoot } from 'react-dom/client';
import { App, ConfigProvider } from 'antd';
import { MemoryRouter } from 'react-router-dom';
import i18next from 'i18next';
import { I18nextProvider } from 'react-i18next';
import bundledPresets from '@/../tauri/resources/preset_models.json';
import enUS from '@/i18n/locales/en-US.json';
import zhCN from '@/i18n/locales/zh-CN.json';

// The complete real page is imported only after official mockIPC is installed.
const { updatePresetModels } = await import('@/constants/presetModels');
updatePresetModels(bundledPresets);
// Match the application's route-module entry order without mounting App.
await import('@/app/routes');
const { default: GrokPage } = await import('@/features/coding/grok/pages/GrokPage');
const i18n = i18next.createInstance();
await i18n.init({ lng: 'en-US', fallbackLng: 'en-US', resources: {
  'en-US': { translation: enUS }, 'zh-CN': { translation: zhCN },
},
  initImmediate: false, interpolation: { escapeValue: false }, returnEmptyString: false });
let committed = false;
function Fixture() {
  React.useLayoutEffect(() => { committed = true; return () => { committed = false; }; }, []);
  return <I18nextProvider i18n={i18n}><ConfigProvider><App><MemoryRouter initialEntries={['/coding/grok']}>
    <main id="grok-page"><GrokPage /></main>
  </MemoryRouter></App></ConfigProvider></I18nextProvider>;
}
const visible = element => {
  const rect = element.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0 && getComputedStyle(element).visibility !== 'hidden';
};
window.grokFetchFixture = {
  translate: (key, params) => i18n.t(key, params),
  isReady: () => committed
    && boundary.snapshot().calls.includes('list_grok_official_accounts')
    && ![...document.querySelectorAll('.ant-spin-spinning')].some(visible)
    && Boolean(document.querySelector('#grok-page')?.textContent.includes('Synthetic Grok Fixture')),
  snapshot: () => ({ ...boundary.snapshot(), committed,
    text: document.querySelector('#grok-page')?.textContent || '',
    dialogs: [...document.querySelectorAll('[role="dialog"]')].filter(visible).map(element => element.textContent),
    alerts: [...document.querySelectorAll('.ant-message-notice,[role="alert"]')].filter(visible).map(element => element.textContent),
  }),
};
createRoot(document.getElementById('root')).render(<Fixture />);
