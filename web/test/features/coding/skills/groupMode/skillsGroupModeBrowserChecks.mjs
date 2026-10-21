import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import path from 'node:path';
export async function verifySkillsGroupMode({send,evaluate,baseUrl,exceptions,artifactRoot}){
 const results=[];let phase='setup',observations=[];
 const snapshot=()=>evaluate('skillsGroupFixture.snapshot()');
 const observe=async label=>{const state=await snapshot();observations.push({label,state});return state;};
 const wait=async fn=>{for(let n=0;n<200;n++){if(await fn())return;await new Promise(r=>setTimeout(r,20));}throw Error('Fixture UI deadline');};
 const drain=()=>evaluate('skillsGroupFixture.drain()');
 async function click(selector,text){phase='interaction';const point=await evaluate(`(()=>{const es=[...document.querySelectorAll(${JSON.stringify(selector)})].filter(e=>{const r=e.getBoundingClientRect();return r.width>0&&r.height>0&&getComputedStyle(e).visibility!=='hidden'&&(${JSON.stringify(text??null)}===null||e.textContent===${JSON.stringify(text??null)});});if(es.length!==1)throw Error('Expected one visible target '+es.length);const e=es[0],r=e.getBoundingClientRect(),x=r.x+r.width/2,y=r.y+r.height/2;if(x<0||y<0||x>=innerWidth||y>=innerHeight||!e.contains(document.elementFromPoint(x,y)))throw Error('Target occluded');return{x,y,rect:{x:r.x,y:r.y,width:r.width,height:r.height},tag:e.tagName,role:e.getAttribute('role'),checked:e.getAttribute('aria-checked'),hit:true,popoverClass:e.closest('.ant-popover')?.className??null};})()`);observations.push({label:'click-before',selector,text,point});await send('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,x:point.x,y:point.y});await send('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,x:point.x,y:point.y});await drain();await observe('click-after');}
 const modes='[aria-label="skills.groupControls.groupingLabel"] [role="radio"]';
 async function openOptions(){
  await click('button[title^="skills.toolbar.options"]');
  let previous='',stable=0;
  await wait(async()=>{
   const geometry=await evaluate(`(()=>{const groups=[...document.querySelectorAll('[aria-label="skills.groupControls.groupingLabel"]')].filter(e=>{const r=e.getBoundingClientRect();return r.width>0&&r.height>0&&getComputedStyle(e).visibility!=='hidden';});if(groups.length!==1)return null;const group=groups[0],popup=group.closest('.ant-popover');if(!popup||/ant-zoom-big-(appear|enter|leave)/.test(popup.className))return null;const style=getComputedStyle(popup);if(style.visibility==='hidden'||Number(style.opacity)!==1||popup.getAnimations({subtree:true}).some(a=>a.playState==='running'||a.pending))return null;const radios=[...group.querySelectorAll('[role="radio"]')];if(radios.length!==2)return null;const rects=[];for(const e of radios){const r=e.getBoundingClientRect(),x=r.x+r.width/2,y=r.y+r.height/2;if(r.width<=0||r.height<=0||getComputedStyle(e).visibility==='hidden'||x<0||y<0||x>=innerWidth||y>=innerHeight||!e.contains(document.elementFromPoint(x,y)))return null;rects.push({x:r.x,y:r.y,width:r.width,height:r.height,checked:e.getAttribute('aria-checked'),hit:true});}return{popupClass:popup.className,opacity:style.opacity,rects};})()`);
   const key=JSON.stringify(geometry);stable=geometry&&key===previous?stable+1:0;previous=key;
   if(stable>=2){observations.push({label:'popover-stable',geometry});return true;}return false;
  });
  await drain();
 }
 async function choose(mode){await click(modes,'skills.groupBy'+(mode==='source'?'Source':'Custom'));}
 async function check(fn){const state=await observe('invariant');phase='invariant';fn(state);phase='interaction';}
 async function remount(){const writes=(await snapshot()).writes.length;await click('#remount');await openOptions();assert.equal((await snapshot()).writes.length,writes,'Remount must not write preferences');}
 async function run(name,scenario,fn){phase='setup';observations=[];const start=exceptions.length;try{
  await send('Page.navigate',{url:baseUrl+'/?scenario='+scenario});
  await wait(()=>evaluate(`!!window.skillsGroupFixture&&skillsGroupFixture.isReady()&&skillsGroupFixture.snapshot().scenario===${JSON.stringify(scenario)}&&skillsGroupFixture.snapshot().readCount===1`));await drain();await openOptions();
  const ready=await observe('ready');assert.equal(ready.forbidden,0);assert.equal(ready.writes.length,0,'Mount must not persist derived mode');
  await fn();phase='safety';assert.equal((await snapshot()).forbidden,0);assert.equal(exceptions.length,start);
  results.push({name,passed:true,observations,pageExceptions:[]});
 }catch(error){let state;try{state=await snapshot();}catch{}const diagnosticErrors=[];
  try{await writeFile(path.join(artifactRoot,`case-${results.length}-failure.html`),await evaluate('document.documentElement.outerHTML'));const shot=await send('Page.captureScreenshot',{format:'png'});await writeFile(path.join(artifactRoot,`case-${results.length}-failure.png`),Buffer.from(shot.data,'base64'));}catch(e){diagnosticErrors.push(String(e));}
  results.push({name,passed:false,phase,error:String(error),observations,state,pageExceptions:exceptions.slice(start),diagnosticErrors});
 }}
 await run('auto with no groups uses source without writing','auto-empty',async()=>{await check(s=>{assert.deepEqual(s.mode,['skills.groupBySource']);assert.deepEqual(s.groupHeaders,['synthetic']);});});
 await run('auto with owned group uses custom without writing','auto-group',async()=>{await check(s=>{assert.deepEqual(s.mode,['skills.groupByCustom']);assert.deepEqual(s.groupHeaders,['Synthetic Group']);});});
 await run('explicit source overrides existing custom group','explicit-source',async()=>{await check(s=>{assert.deepEqual(s.mode,['skills.groupBySource']);assert.deepEqual(s.groupHeaders,['synthetic']);});});
 await run('explicit custom survives zero groups','explicit-custom',async()=>{await check(s=>{assert.deepEqual(s.mode,['skills.groupByCustom']);assert.deepEqual(s.groupHeaders,['skills.groupUngrouped']);});});
 await run('successful choice persists across real page remount','auto-empty',async()=>{await choose('custom');await check(s=>{assert.equal(s.stored,'custom');assert.deepEqual(s.writes,['custom']);});await remount();await check(s=>{assert.deepEqual(s.mode,['skills.groupByCustom']);assert.deepEqual(s.groupHeaders,['skills.groupUngrouped']);});});
 await run('late initial read does not overwrite user choice','delayed-read',async()=>{await choose('custom');await evaluate('skillsGroupFixture.resolveInitial()');await check(s=>{assert.deepEqual(s.mode,['skills.groupByCustom']);assert.deepEqual(s.groupHeaders,['skills.groupUngrouped']);assert.equal(s.stored,'custom');});});
 await run('failed save characterization preserves old storage','save-failure',async()=>{await choose('custom');await check(s=>{assert.deepEqual(s.mode,['skills.groupByCustom']);assert.deepEqual(s.groupHeaders,['skills.groupUngrouped']);assert.equal(s.stored,'source');assert.deepEqual(s.writes,['custom']);assert.deepEqual(s.alerts,[],'Current upstream shows no visible persistence error; characterization, not desired UX');});await remount();await check(s=>{assert.deepEqual(s.mode,['skills.groupBySource']);assert.deepEqual(s.groupHeaders,['synthetic']);});});
 await run('successive normal writes retain final intent','auto-empty',async()=>{await choose('custom');await choose('source');await check(s=>{assert.deepEqual(s.writes,['custom','source']);assert.equal(s.stored,'source');});await remount();await check(s=>{assert.deepEqual(s.mode,['skills.groupBySource']);assert.deepEqual(s.groupHeaders,['synthetic']);});});
 await run('unmount smoke: delayed read does not resurrect view','unmount-read',async()=>{await click('#unmount');await evaluate('skillsGroupFixture.resolveInitial()');await check(s=>{assert.equal(s.mounted,false);assert.equal(s.writes.length,0);});});
 const report={passed:results.filter(x=>x.passed).length,failed:results.filter(x=>!x.passed).length,results};await writeFile(path.join(artifactRoot,'skills-group-cases.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));assert.equal(report.failed,0,'Inspect phase and observations; setup failures are not product defects');return results;
}
