import React from 'react';
import {flushSync} from 'react-dom';
import {createRoot} from 'react-dom/client';
import {ConfigProvider} from 'antd';
import i18next from 'i18next';
import {I18nextProvider} from 'react-i18next';
import {SSHSyncModal} from '@/features/settings/components/SSHSyncModal';
import {boundary} from './boundary.mjs';
const i18n=i18next.createInstance();await i18n.init({lng:'en',fallbackLng:'en',resources:{en:{translation:{}}},initImmediate:false});
let barrier;
function Fixture(){const[open,setOpen]=React.useState(true);const[present,setPresent]=React.useState(true);const[tick,setTick]=React.useState(0);const complete=React.useRef();barrier=()=>new Promise(r=>{complete.current=r;flushSync(()=>setTick(x=>x+1));});React.useEffect(()=>{complete.current?.();complete.current=undefined;},[tick]);return <I18nextProvider i18n={i18n}><ConfigProvider><button id="reopen" onClick={()=>setOpen(true)}>Reopen</button><button id="unmount" onClick={()=>setPresent(false)}>Unmount</button><output id="barrier">{tick}</output>{present&&<SSHSyncModal open={open} onClose={()=>setOpen(false)}/>}</ConfigProvider></I18nextProvider>;}
const root=createRoot(document.getElementById('root'));root.render(<Fixture/>);
// A same-root passive-effect commit follows delivered boundary promises. Repeat only
// the read-only barrier until config/event counters and request inventory stabilize;
// never resolve an additional request or retry an interaction in this drain.
async function drain(){
 let previous='';let stable=0;
 for(let i=0;i<10;i++){
  await Promise.resolve();await barrier();await Promise.resolve();
  const state=boundary.snapshot();const current=JSON.stringify(state);
  stable=current===previous?stable+1:0;previous=current;
  if(stable===2)return state;
 }
 throw Error('Config/event effects did not stabilize');
}
function visible(e){const r=e.getBoundingClientRect();return r.width>0&&r.height>0&&getComputedStyle(e).visibility!=='hidden';}
window.sshRequestFixture={...boundary,drain,async settle(id,reject){await boundary.settle(id,reject);await drain();},snapshot(){
 const dialogs=[...document.querySelectorAll('.ant-modal')].filter(visible);
 return {...boundary.snapshot(),text:dialogs.map(x=>x.textContent).join('\n'),visible:dialogs.length>0,
 spinners:dialogs.reduce((n,d)=>n+[...d.querySelectorAll('.ant-spin-spinning')].filter(visible).length,0),
 animations:dialogs.flatMap(d=>d.getAnimations({subtree:true}).filter(a=>a.playState==='running'&&Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>({type:a.constructor.name,playState:a.playState}))),
 barrier:Number(document.querySelector('#barrier')?.textContent),caseId:new URLSearchParams(location.search).get('case')};}};
