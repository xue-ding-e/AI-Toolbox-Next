import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import path from 'node:path';
import {createSSHFixtureInteractions, waitForSSHFixtureState} from './sshRequestBrowserInteractions.mjs';
export async function verifySSHRequestRace({send,evaluate,baseUrl,exceptions,artifactRoot}) {
 const results=[];let serial=0;let phase='setup';let selectorCategory=null;let observations=[];
 const snapshot=()=>evaluate('sshRequestFixture.snapshot()');
 const wait=waitForSSHFixtureState;
 const drain=async()=>{await evaluate('sshRequestFixture.drain()');await wait(async()=>!(await snapshot()).animations.length);};
 const observe=async label=>{const state=await snapshot();observations.push({label,state});return state;};
 const {click,chooseB:selectB}=createSSHFixtureInteractions({send,evaluate,snapshot,drain,
  onPhase:(step,category)=>{phase='interaction:'+step;selectorCategory=category;}});
 const pending=async id=>(await snapshot()).calls.flatMap((c,i)=>c.id===id&&!c.settled?[i]:[]);
 const settle=async(indices,reject=false)=>{phase='settlement';await observe('before settlement '+indices.join(','));await evaluate(`sshRequestFixture.settle(${JSON.stringify(indices)},${reject})`);await observe('after settlement');};
 const check=async fn=>{const state=await observe('invariant');phase='invariant';fn(state);phase='interaction';};
 const chooseB=async()=>{
  await selectB();await observe('B selected after config-event drain');
 };
 async function run(name,control,fn){
  phase='setup';selectorCategory=null;observations=[];const exceptionStart=exceptions.length;
  try{
   const id=String(serial++);await send('Page.navigate',{url:baseUrl+'/?case='+id});
   await wait(()=>evaluate(`!!window.sshRequestFixture&&sshRequestFixture.snapshot().caseId===${JSON.stringify(id)}&&sshRequestFixture.snapshot().calls.some(c=>c.id==='A')`));
   await drain();await observe('ready');await fn();
   phase='safety';assert.equal((await snapshot()).forbidden,0);assert.equal(exceptions.length,exceptionStart,'Page exception');
   results.push({name,control,passed:true,observations,pageExceptions:[]});
  }catch(e){
   console.error(JSON.stringify({suite:'ssh-request-race',case:name,phase,selectorCategory,error:String(e)}));
   const pageExceptions=exceptions.slice(exceptionStart);let lastObservation;try{lastObservation=await snapshot();}catch{}
   const diagnosticErrors=[];
   try{await writeFile(path.join(artifactRoot,`case-${serial-1}-failure.html`),await evaluate('document.documentElement.outerHTML'));}catch(error){diagnosticErrors.push('DOM: '+String(error));}
   try{const shot=await send('Page.captureScreenshot',{format:'png'});await writeFile(path.join(artifactRoot,`case-${serial-1}-failure.png`),Buffer.from(shot.data,'base64'));}catch(error){diagnosticErrors.push('screenshot: '+String(error));}
   results.push({name,control,passed:false,classification:phase==='invariant'&&!control&&!pageExceptions.length&&lastObservation?.forbidden===0?'product-invariant':'fixture-or-control-error',phase,selectorCategory,error:String(e),observations,lastObservation,pageExceptions,diagnosticErrors});
  }
 }
 await run('current success control',true,async()=>{const count=(await snapshot()).calls.length;await settle(await pending('A'));await check(s=>{assert.ok(s.text.includes('A-result'));assert.equal(s.spinners,0);assert.equal(s.calls.length,count,'stable config commit must not trigger another request');});});
 await run('current failure control',true,async()=>{await settle(await pending('A'),true);await check(s=>{assert.ok(s.text.includes('synthetic-A-error'));assert.equal(s.spinners,0);});});
 await run('B result survives late A success',false,async()=>{const a=await pending('A');await chooseB();await settle(await pending('B'));await settle(a);await check(s=>{assert.ok(s.text.includes('B-result'));assert.ok(!s.text.includes('A-result'));assert.equal(s.spinners,0);});});
 await run('late A rejection preserves pending B spinner',false,async()=>{const a=await pending('A');await chooseB();await settle(a,true);await check(s=>{assert.ok(s.spinners>0);assert.ok(!s.text.includes('synthetic-A-error'));});});
 await run('close reopen rejects old A result',false,async()=>{const a=await pending('A');await click('.ant-modal-close');await wait(async()=>!(await snapshot()).visible);await click('#reopen');await drain();await wait(async()=> (await snapshot()).visible&&(await pending('A')).some(i=>!a.includes(i)));await settle(a);await check(s=>{assert.ok(!s.text.includes('A-result'));assert.ok(s.spinners>0);});});
 await run('disable reenable ignores old result',false,async()=>{const a=await pending('A');await click('.ant-modal button[role="switch"]');await drain();await wait(()=>evaluate(`document.querySelector('.ant-modal button[role="switch"]').getAttribute('aria-checked')==='false'`));await click('.ant-modal button[role="switch"]');await drain();await wait(async()=> (await pending('A')).some(i=>!a.includes(i)));await settle(a);await check(s=>{assert.ok(!s.text.includes('A-result'));assert.ok(s.spinners>0);});});
 await run('unmount pending stays absent',true,async()=>{const a=await pending('A');await click('.ant-modal-close');await wait(async()=>!(await snapshot()).visible);await click('#unmount');await drain();await settle(a);await check(s=>assert.equal(s.visible,false));});
 const report={passed:results.filter(x=>x.passed).length,productInvariantFailures:results.filter(x=>x.classification==='product-invariant').length,fixtureErrors:results.filter(x=>x.classification==='fixture-or-control-error').length,results};
 report.validRed=report.productInvariantFailures>0&&report.fixtureErrors===0&&results.filter(x=>x.control).every(x=>x.passed)&&results.every(x=>!x.pageExceptions.length&&x.observations.every(o=>o.state.forbidden===0));
 await writeFile(path.join(artifactRoot,'ssh-request-cases.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));
 assert.equal(results.filter(x=>!x.passed).length,0,'SSH result race cases failed; inspect classifications');return results;
}
