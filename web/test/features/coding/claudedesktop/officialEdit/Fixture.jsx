import { scenario,calls,denied,runtimeErrors } from './transport.mjs';
import React from 'react';
import {createRoot} from 'react-dom/client';
import {App,ConfigProvider} from 'antd';
import i18next from 'i18next';
import {I18nextProvider} from 'react-i18next';
import enUS from '@/i18n/locales/en-US.json';
import zhCN from '@/i18n/locales/zh-CN.json';
import gatewayProfiles from '@/../tauri/resources/gateway_provider_profiles.json';
import {updateGatewayProviderProfiles} from '@/features/coding/shared/gateway/providerProfiles';
import {createPageBoundary,sourceIdentity} from './page-boundary.generated.mjs';
updateGatewayProviderProfiles(gatewayProfiles);
const {default:FormModal}=await import('@/features/coding/claudedesktop/components/ClaudeDesktopProviderFormModal');
const {default:ImportModal}=await import('@/features/coding/shared/ccSwitch/ImportFromCcSwitchModal');
const i18n=i18next.createInstance();
await i18n.init({lng:'en-US',fallbackLng:'en-US',resources:{'en-US':{translation:enUS},'zh-CN':{translation:zhCN}},initImmediate:false,interpolation:{escapeValue:false}});
const page=createPageBoundary(scenario.provider?[scenario.provider]:[],Boolean(scenario.copy));
const submitted=[];let committed=false,closed=false;
function Fixture(){
 const [importOpen,setImportOpen]=React.useState(Boolean(scenario.candidate));
 const [provider,setProvider]=React.useState(scenario.provider?page.edit(scenario.provider.id):null);
 const [formOpen,setFormOpen]=React.useState(Boolean(provider)||Boolean(scenario.create));
 React.useLayoutEffect(()=>{committed=true;return()=>{committed=false;};},[]);
 return <I18nextProvider i18n={i18n}><ConfigProvider><App>
  <ImportModal open={importOpen} appType="claude" existingProviderIds={[]} onClose={()=>setImportOpen(false)} onImport={async candidates=>{
   await page.importCandidates(candidates);const p=page.snapshot().providers[0];if(!p)throw new Error('No synthetic import selected');
   setImportOpen(false);setProvider(page.edit(p.id));setFormOpen(true);
  }}/>
  <FormModal open={formOpen} provider={provider} isCopy={Boolean(scenario.copy)} onCancel={()=>{closed=true;setFormOpen(false);}} onSubmit={async values=>{submitted.push(structuredClone(values));await page.submit(values);}}/>
 </App></ConfigProvider></I18nextProvider>;
}
const visible=e=>Boolean(e&&e.getBoundingClientRect().width&&e.getBoundingClientRect().height&&getComputedStyle(e).visibility!=='hidden');
window.officialEditFixture={
 snapshot:()=>({case:scenario.id,sourceIdentity,committed,closed,page:page.snapshot(),submitted:structuredClone(submitted),calls:structuredClone(calls),denied:[...denied],runtimeErrors:[...runtimeErrors],
  fields:Object.fromEntries(['name','baseUrl','apiKey'].map(k=>{const e=document.getElementById(k);return[k,e?{value:e.value,disabled:e.disabled,visible:visible(e)}:null];})),
  dialogs:[...document.querySelectorAll('[role="dialog"]')].filter(visible).map(e=>e.textContent),
  validationFields:[...document.querySelectorAll('.ant-form-item-explain-error')].filter(visible).map(e=>e.closest('.ant-form-item')?.querySelector('input')?.id||'unknown'),
  validationErrors:[...document.querySelectorAll('.ant-form-item-explain-error')].filter(visible).map(e=>e.textContent),
  endpointDisabled:Boolean(document.getElementById('providerEndpointKey')?.closest('.ant-select')?.classList.contains('ant-select-disabled')),
 }),translate:(key,params)=>i18n.t(key,params),
};
createRoot(document.getElementById('root')).render(<Fixture/>);
