import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {scenarios,editedNote}from'./scenarios.mjs';
const clone=x=>structuredClone(x);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
export function makeVerifier(variant,expectedIdentity){
 return async({send,evaluate,baseUrl,artifactRoot,exceptions})=>{
  const results=[],screenshots=[],diagnosticErrors=[],interactions=[],runDeadline=Date.now()+480000;
  const matrix=scenarios.filter(x=>variant==='green'||!x.greenOnly);
  let caseDeadline=0,currentCase=null;
  const bounded=async(promise,maxMs=Infinity)=>{let timer;try{return await Promise.race([promise,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Fixture operation deadline exceeded')),Math.max(1,Math.min(caseDeadline,runDeadline,Date.now()+maxMs)-Date.now()));})]);}finally{clearTimeout(timer);}};
  const ev=expression=>bounded(evaluate(expression));
  const cmd=(method,args)=>bounded(send(method,args));
  const snap=()=>ev('window.officialEditFixture?.snapshot()');
  const healthy=s=>{assert(s,'Fixture not initialized');assert.deepEqual(s.denied,[],'Forbidden IPC/network action');assert.deepEqual(s.runtimeErrors,[],'Browser runtime/setup failure');assert.deepEqual(s.page.errors,[],'Page callback/setup failure');assert.equal(exceptions.length,0,'Browser exception is not product RED');};
  const wait=async(label,predicate,maxMs=Infinity)=>{const deadline=Math.min(caseDeadline,runDeadline,Date.now()+maxMs);while(Date.now()<deadline){const value=await predicate();if(value)return value;await sleep(50);}throw new Error('Fixture wait timed out: '+label);};
  const dialog=`document.getElementById('name')?.closest('[role="dialog"]')`;
  // Capture only an open, real form. Closed-modal blank pages are not visual evidence.
  // Diagnostics may fail the evidence gate, but must never replace a product failure.
  const capture=async(item,moment,anchor)=>{
   const expression=`document.getElementById(${JSON.stringify(anchor)})`;
   const trace={case:item.id,kind:'screenshot',moment,phase:'observing',samples:[]};interactions.push(trace);
   // Preparation may send bounded wheel input. Its failure must stop all later
   // interaction, unlike a diagnostic-only screenshot capture/write failure.
   try{trace.before=await prepareTarget(expression,trace,false);}
   catch(error){trace.error=String(error);diagnosticErrors.push({case:item.id,moment,stage:'prepare-target',error:String(error)});throw error;}
   try{
    // A disabled official-name input may be photographed, never interacted with.
    trace.phase='capture-requested';
    const {data}=await cmd('Page.captureScreenshot',{format:'png'});
    const bytes=Buffer.from(data,'base64');
    assert(bytes.length>0,'Empty screenshot');
    const file=path.join('screenshots',variant+'-'+item.id+'-'+moment+'.png');
    fs.mkdirSync(path.join(artifactRoot,'screenshots'),{recursive:true});
    fs.writeFileSync(path.join(artifactRoot,file),bytes);
    screenshots.push({case:item.id,moment,file,bytes:bytes.length,sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
    trace.phase='captured';trace.after=await observeTarget(expression);
   }catch(error){trace.error=String(error);diagnosticErrors.push({case:item.id,moment,error:String(error)});}
  };
  const observeTarget=expression=>ev(`(()=>{
   const e=${expression};if(!e)return{ready:false,reason:'missing-target'};
   const describe=n=>n?{tag:n.tagName,id:n.id||'',className:n.getAttribute('class')||''}:null;
   const rect=n=>{const r=n.getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height};};
   const r=rect(e),point={x:r.x+r.width/2,y:r.y+r.height/2},hit=document.elementFromPoint(point.x,point.y);
   const ancestors=[];for(let n=e;n;n=n.parentElement)ancestors.push(n);
   const hidden=ancestors.some(n=>{const s=getComputedStyle(n);return s.display==='none'||s.visibility!=='visible';});
   const animations=ancestors.flatMap(n=>n.getAnimations().filter(a=>a.pending||a.playState==='running'||a.playState==='paused').map(a=>({element:describe(n),state:a.playState,pending:a.pending})));
   const d=e.closest('[role="dialog"]'),ds=d?getComputedStyle(d):null;
   const disabled=Boolean(e.disabled||e.matches(':disabled')||e.getAttribute('aria-disabled')==='true');
   const hitTarget=hit===e||e.contains(hit);
   const dialog=d?{...describe(d),style:d.getAttribute('style')||'',rect:rect(d),opacity:ds.opacity,
    buttons:[...d.querySelectorAll('button')].map(b=>({text:b.textContent.trim(),disabled:b.disabled})),
    checkboxes:[...d.querySelectorAll('input[type="checkbox"]')].map(c=>({checked:c.checked,disabled:c.disabled}))}:null;
   const w=d?.closest('.ant-modal-wrap'),ws=w?getComputedStyle(w):null;
   const wr=w?rect(w):null;
   // This actual modal scrollport, not the locked body, receives normal wheel input.
   const origin=w?{x:Math.max(8,Math.min(innerWidth-8,wr.x+8)),y:Math.max(8,Math.min(innerHeight-8,(Math.max(0,wr.y)+Math.min(innerHeight,wr.y+wr.height))/2))}:null;
   const wheelHit=origin?document.elementFromPoint(origin.x,origin.y):null;
   const scrollport=w?{...describe(w),rect:wr,overflowY:ws.overflowY,scrollTop:w.scrollTop,clientHeight:w.clientHeight,scrollHeight:w.scrollHeight,
    origin,originHit:describe(wheelHit),originHitsScrollport:wheelHit===w||w.contains(wheelHit)}:null;
   // Native checkbox inputs may intentionally have opacity:0. Hit-testing the
   // real input is authoritative; only the whole dialog must be fully visible.
   const layoutReady=e.isConnected&&!hidden&&r.width>0&&r.height>0&&animations.length===0&&Boolean(d&&Number(ds.opacity)>=0.999);
   const visualReady=layoutReady&&point.x>=0&&point.y>=0&&point.x<innerWidth&&point.y<innerHeight&&hitTarget;
   return{ready:visualReady&&!disabled,visualReady,layoutReady,target:describe(e),disabled,rect:r,point,hit:describe(hit),hitTarget,hidden,animations,dialog,scrollport,viewport:{width:innerWidth,height:innerHeight},activeElement:describe(document.activeElement)};
  })()`);
  const waitForStableTarget=async(expression,trace,interactive=true,layoutOnly=false)=>{
   let previous=null,stable=0;
   return wait('stable unobstructed '+(interactive?'interaction target':'screenshot form'),async()=>{
    const state=await observeTarget(expression);trace.samples.push(state);if(trace.samples.length>4)trace.samples.shift();
    const key=layoutOnly?'layoutReady':interactive?'ready':'visualReady';
    const same=state[key]&&previous?.[key]&&['x','y','width','height'].every(k=>Math.abs(state.rect[k]-previous.rect[k])<0.25&&Math.abs(state.dialog.rect[k]-previous.dialog.rect[k])<0.25);
    stable=state[key]?(same?stable+1:1):0;previous=state;return stable>=3?state:false;
   });
  };
  const prepareTarget=async(expression,trace,interactive=true)=>{
   // First let opening motion/focus settle, even if the target is below the viewport.
   // Never assign scrollTop or call scrollIntoView during entrance animation.
   let state=await waitForStableTarget(expression,trace,interactive,true);
   if(interactive)assert.equal(state.disabled,false,'Disabled interaction target');
   trace.wheels=[];
   for(let step=0;step<=2;step++){
    assert(state.point.x>=0&&state.point.x<state.viewport.width,'Target is horizontally outside the viewport');
    if(state.point.y>=0&&state.point.y<state.viewport.height)return waitForStableTarget(expression,trace,interactive);
    assert(step<2,'Target remains outside the viewport after two bounded wheel actions');
    const port=state.scrollport;
    assert(port&&/^(auto|scroll)$/.test(port.overflowY)&&port.scrollHeight>port.clientHeight,'Target has no scrollable modal wrap');
    assert(port.originHitsScrollport,'Wheel origin does not hit the actual modal scrollport');
    const wanted=Math.max(-600,Math.min(600,state.point.y-state.viewport.height/2));
    const deltaY=wanted>0?Math.min(wanted,port.scrollHeight-port.clientHeight-port.scrollTop):Math.max(wanted,-port.scrollTop);
    assert(Math.abs(deltaY)>=1,'Modal scrollport has no remaining range toward the target');
    const wheel={step:step+1,before:state,origin:port.origin,deltaY};trace.wheels.push(wheel);trace.phase='wheel-requested';
    // Each native wheel plus observed scroll progress has a 3s budget, inside
    // the unchanged per-case/run deadline. No click is retried or forced.
    const deadline=Math.min(caseDeadline,runDeadline,Date.now()+3000);
    const withinWheel=operation=>{assert(Date.now()<deadline,'Wheel deadline exceeded');return bounded(operation(),deadline-Date.now());};
    await withinWheel(()=>send('Input.dispatchMouseEvent',{type:'mouseMoved',...port.origin}));
    await withinWheel(()=>send('Input.dispatchMouseEvent',{type:'mouseWheel',...port.origin,deltaX:0,deltaY}));
    trace.phase='wheel-dispatched';
    wheel.afterProgress=await wait('native wheel changed modal scrollTop',async()=>{
     const observed=await withinWheel(()=>observeTarget(expression));
     const moved=observed.scrollport&&observed.scrollport.scrollTop-port.scrollTop;
     return typeof moved==='number'&&Math.abs(moved)>0.25&&Math.sign(moved)===Math.sign(deltaY)?observed:false;
    },deadline-Date.now());
    state=await waitForStableTarget(expression,trace,interactive,true);wheel.afterSettled=state;trace.phase='observing';
   }
  };
  const click=async expression=>{
   const trace={case:currentCase,kind:'click',expression,phase:'observing',samples:[]};interactions.push(trace);
   try{
    // Normal bounded wheel input may reveal the target; the original strict
    // stable geometry, hit-test and disabled gates still precede the only click.
    const before=await prepareTarget(expression,trace);
    trace.before=before;trace.phase='press-requested';
    await cmd('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...before.point});
    trace.phase='release-requested';
    await cmd('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...before.point});
    trace.phase='released';trace.after=await observeTarget(expression);
   }catch(error){trace.error=String(error);throw error;}
  };
  const fill=async(id,value)=>{
   await click(`document.getElementById(${JSON.stringify(id)})`);
   await cmd('Input.dispatchKeyEvent',{type:'keyDown',key:'a',code:'KeyA',windowsVirtualKeyCode:65,modifiers:2});
   await cmd('Input.dispatchKeyEvent',{type:'keyUp',key:'a',code:'KeyA',windowsVirtualKeyCode:65,modifiers:2});
   await cmd('Input.dispatchKeyEvent',{type:'keyDown',key:'Backspace',code:'Backspace',windowsVirtualKeyCode:8});
   await cmd('Input.dispatchKeyEvent',{type:'keyUp',key:'Backspace',code:'Backspace',windowsVirtualKeyCode:8});
   if(value!=='')await cmd('Input.insertText',{text:value});
   assert.equal(await ev(`document.getElementById(${JSON.stringify(id)}).value`),value,'Native input did not receive exact fixture text');
  };
  const button=key=>`[...(${dialog})?.querySelectorAll('button')||[]].find(e=>e.textContent.trim()===window.officialEditFixture.translate(${JSON.stringify(key)}))`;
  const save=()=>click(button('common.save'));
  const notes=async()=>{await click(`[...(${dialog})?.querySelectorAll('[role="button"]')||[]].find(e=>e.textContent.trim()==='备注')`);await fill('notes',editedNote);};
  const initialCount=s=>({creates:s.page.creates.length,writes:s.page.writes.length,submitted:s.submitted.length});
  const expectNoSave=(s,before)=>{assert.deepEqual(initialCount(s),initialCount(before),'Unexpected save/create before validation/cancel');};
  const expectInvalid=async(item,before)=>{
   await save();const s=await wait('actual form validation error',async()=>{const x=await snap();healthy(x);return x.validationErrors.length?x:false;});
   expectNoSave(s,before);assert.equal(s.closed,false);
   let fields=item.id.includes('token-missing-url')||item.copy?['baseUrl']:['apiKey'];
   if(item.id==='changed-both-nel')fields=['baseUrl','apiKey'];
   for(const field of fields)assert(s.validationFields.includes(field),'Wrong field validation: '+JSON.stringify(s.validationFields));
   await sleep(150);const stable=await snap();healthy(stable);expectNoSave(stable,before);return stable;
  };
  try{
   for(const item of matrix){
    currentCase=item.id;
    caseDeadline=Date.now()+30000;
    await cmd('Page.navigate',{url:baseUrl+'/?case='+encodeURIComponent(item.id)});
    await wait('fixture mounted',async()=>{const s=await snap();return s?.committed?s:false;});
    if(item.candidate){
     await wait('real import candidate',async()=>{const s=await snap();healthy(s);return s.dialogs.some(x=>x.includes(item.candidate.name))&&s.calls.some(x=>x.command==='list_cc_switch_providers');});
     const importDialog=`[...document.querySelectorAll('[role="dialog"]')].find(e=>e.textContent.includes(${JSON.stringify(item.candidate.name)})&&e.getBoundingClientRect().width)`;
     await click(`(${importDialog})?.querySelector('input[type="checkbox"]')`);
     await wait('real import selection',()=>ev(`(()=>{const d=${importDialog};return [...d?.querySelectorAll('button')||[]].some(e=>e.textContent.trim().startsWith(window.officialEditFixture.translate('common.ccSwitch.importSelected'))&&!e.disabled);})()`));
     await click(`[...(${importDialog})?.querySelectorAll('button')||[]].find(e=>e.textContent.trim().startsWith(window.officialEditFixture.translate('common.ccSwitch.importSelected'))&&!e.disabled)`);
    }
    const expectedName=item.candidate?.name||item.provider?.name||'';
    const before=await wait('real form initialized',async()=>{const s=await snap();healthy(s);return s.fields.name?.visible&&s.fields.name.value===expectedName?s:false;});
    assert.deepEqual(before.sourceIdentity,expectedIdentity,'Stale or wrong source callback/form build');
    const original=clone(before.page.providers[0]);
    assert.equal(before.submitted.length,0);assert.equal(before.page.writes.length,0);
    assert.equal(before.endpointDisabled,!item.create&&!item.copy,'Edit selector behavior changed');
    const isRestore=item.id==='pure-restore-notes';
    const baselineOfficial=variant==='red'&&(original?.category==='official');
    if(baselineOfficial||isRestore){assert.equal(before.fields.name.disabled,true);assert.equal(before.fields.baseUrl,null);assert.equal(before.fields.apiKey,null);}
    else if(!item.create){
     const env=JSON.parse(original.settingsConfig).env||{};
     assert.equal(before.fields.baseUrl?.value,env.ANTHROPIC_BASE_URL||'','Initializer changed the raw URL');
     assert.equal(before.fields.apiKey?.value,env.ANTHROPIC_AUTH_TOKEN||env.ANTHROPIC_API_KEY||'','Initializer changed the raw token');
     assert.equal(before.fields.name.disabled,false);
    }
    if(item.id==='official-both-notes')await capture(item,'initialized','name');
    if(item.action==='cancel'){
     await click(button('common.cancel'));await wait('form cancellation',async()=>{const s=await snap();return s.closed?s:false;});
     await sleep(150);const after=await snap();healthy(after);expectNoSave(after,before);
     results.push({case:item.id,verdict:'control-pass',before,after});continue;
    }
    for(const[id,value]of Object.entries(item.changes||{}))await fill(id,value);
    if(item.action==='invalid'||item.action==='revert'){
     const invalid=await expectInvalid(item,before);
     if(item.id==='url-only-added-space-key')await capture(item,'validation-error','apiKey');
     if(item.action==='invalid'){results.push({case:item.id,verdict:'validation-control-pass',before,after:invalid});continue;}
     for(const[id,value]of Object.entries(item.revert))await fill(id,value);
     await wait('cross-field validation clears after exact revert',async()=>{const s=await snap();healthy(s);return s.validationErrors.length===0;});
    }
    if(item.action!=='unchanged')await notes();
    if(variant==='green'&&item.id==='pure-restore-notes')await capture(item,'before-save','notes');
    await save();
    const after=await wait('one completed page update',async()=>{const s=await snap();healthy(s);return s.page.writes.length===1&&s.closed?s:false;});
    assert.equal(after.submitted.length,1,'Duplicate or missing form submit');assert.equal(after.page.creates.length,before.page.creates.length);
    assert.deepEqual(after.validationErrors,[],'Validation error cannot be product RED');
    const saved=after.page.writes[0],oldEnv=JSON.parse(original.settingsConfig).env||{},env=JSON.parse(saved.settingsConfig).env||{};
    assert.equal(saved.category,original.category,'Notes edit silently changed raw category');
    assert.equal(saved.name,original.name);assert.equal(saved.notes,item.action==='unchanged'?original.notes:editedNote);
    for(const key of ['sourceProviderId','websiteUrl','icon','iconColor','sortIndex'])assert.deepEqual(saved[key],original[key]);
    if(original.meta?.opaqueFixtureSentinel)assert.deepEqual(saved.meta?.opaqueFixtureSentinel,original.meta.opaqueFixtureSentinel);
    if(variant==='red'&&item.red){
     assert(Object.keys(oldEnv).some(k=>['ANTHROPIC_BASE_URL','ANTHROPIC_AUTH_TOKEN'].includes(k)&&oldEnv[k]!==env[k]),'Expected data loss did not reproduce');
     assert.deepEqual(env,{},'Unexpected failure shape cannot count as expected RED');
     results.push({case:item.id,verdict:'expected-product-red',before,after});
    }else{
     for(const key of ['ANTHROPIC_BASE_URL','ANTHROPIC_AUTH_TOKEN'])assert.equal(env[key],oldEnv[key],'Exact raw credential preservation failed');
     if(original.meta?.claudeDesktopModelRoutes)assert.deepEqual(saved.meta.claudeDesktopModelRoutes,original.meta.claudeDesktopModelRoutes);
     for(const key of ['customHeaders','modelRewrites'])if(original.meta?.[key])assert.deepEqual(saved.meta[key],original.meta[key]);
     if(isRestore)assert.deepEqual(env,{});
     results.push({case:item.id,verdict:'control-pass',before,after});
    }
   }
   assert.equal(results.length,matrix.length,'Missing scenario result');
   assert.equal(results.filter(x=>x.verdict==='expected-product-red').length,variant==='red'?matrix.filter(x=>x.red).length:0,'RED count mismatch');
   fs.writeFileSync(path.join(artifactRoot,'strict-results.json'),JSON.stringify({variant,count:results.length,sourceIdentity:expectedIdentity,results,screenshots,diagnosticErrors,interactions},null,2)+'\n');
   assert.deepEqual(diagnosticErrors,[],'Screenshot evidence is incomplete; no visual pass');
   assert.equal(screenshots.length,variant==='green'?3:1,'Required screenshot evidence is missing');
   return results.map(x=>({name:x.case,passed:true,verdict:x.verdict,screenshots:screenshots.filter(s=>s.case===x.case)}));
  }catch(error){
   try{fs.writeFileSync(path.join(artifactRoot,'strict-failure.json'),JSON.stringify({variant,status:'fixture-or-unexpected-product-failure',error:String(error),completed:results,screenshots,diagnosticErrors,interactions},null,2)+'\n');}
   catch(diagnosticsError){console.error('Could not save failure diagnostics; original error retained:',String(diagnosticsError));}
   throw error;
  }
 };
}
