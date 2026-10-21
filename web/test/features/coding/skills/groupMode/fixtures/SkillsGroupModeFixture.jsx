import {boundary} from './boundary.mjs';
import React from 'react';
import {createRoot} from 'react-dom/client';
import {ConfigProvider} from 'antd';
import i18next from 'i18next';
import {I18nextProvider} from 'react-i18next';
// Load real view only after official mockIPC installs the strict IPC boundary.
const {default:SkillsPage}=await import('@/features/coding/skills/pages/SkillsPage');
const i18n=i18next.createInstance();await i18n.init({lng:'en',fallbackLng:'en',resources:{en:{translation:{}}},initImmediate:false});
let barrier,ready=false;
function Fixture(){
 React.useLayoutEffect(()=>{ready=true;return()=>{ready=false;};},[]);
 const [key,setKey]=React.useState(0),[present,setPresent]=React.useState(true),[tick,setTick]=React.useState(0);const done=React.useRef();
 barrier=()=>new Promise(resolve=>{done.current=resolve;setTick(n=>n+1);});
 React.useLayoutEffect(()=>{done.current?.();done.current=undefined;},[tick]);
 return <I18nextProvider i18n={i18n}><ConfigProvider><button id="remount" onClick={()=>{setPresent(true);setKey(n=>n+1);}}>Remount</button><button id="unmount" onClick={()=>setPresent(false)}>Unmount</button><output id="barrier">{tick}</output><main id="skills-page">{present&&<SkillsPage key={key}/>}</main></ConfigProvider></I18nextProvider>;
}
createRoot(document.getElementById('root')).render(<Fixture/>);
async function drain(){let last='',stable=0;for(let i=0;i<12;i++){await Promise.resolve();await barrier();await Promise.resolve();const state=window.skillsGroupFixture.snapshot();const value=JSON.stringify({boundary:boundary.snapshot(),mode:state.mode,groupHeaders:state.groupHeaders,mounted:state.mounted,alerts:state.alerts});stable=value===last?stable+1:0;last=value;if(stable===2)return;}throw Error('Fixture effects did not stabilize');}
const visible=e=>{const r=e.getBoundingClientRect();return r.width>0&&r.height>0&&getComputedStyle(e).visibility!=='hidden';};
window.skillsGroupFixture={isReady:()=>ready,drain,async resolveInitial(){boundary.resolveInitial();await drain();},snapshot(){if(!ready)throw Error('Fixture first commit not ready');const selected=[...document.querySelectorAll('[aria-label="skills.groupControls.groupingLabel"] [role="radio"][aria-checked="true"]')].filter(visible);return {...boundary.snapshot(),mode:selected.map(e=>e.textContent),groupHeaders:[...document.querySelectorAll('#skills-page section button[aria-expanded] > span:first-of-type')].filter(visible).map(e=>e.textContent),text:document.querySelector('#skills-page').textContent,alerts:[...document.querySelectorAll('.ant-message-notice,[role="alert"]')].filter(visible).map(e=>e.textContent),mounted:document.querySelector('#skills-page').childElementCount>0,barrier:Number(document.querySelector('#barrier').textContent)};}};
