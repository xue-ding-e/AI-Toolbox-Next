import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
export async function verifyGrokFetchedModels({ send, evaluate, baseUrl, exceptions, artifactRoot }) {
  const results=[]; let phase='setup', observations=[];
  const snapshot=()=>evaluate('grokFetchFixture.snapshot()');
  const observe=async label=>{const state=await snapshot();observations.push({label,state});return state;};
  const wait=async fn=>{for(let n=0;n<250;n++){if(await fn())return;await new Promise(r=>setTimeout(r,20));}throw Error('Fixture UI deadline');};
  async function click(selector,textKey,params){
    phase='interaction';
    const text=textKey===undefined?undefined:await evaluate(`grokFetchFixture.translate(${JSON.stringify(textKey)},${JSON.stringify(params??{})})`);
    const point=await evaluate(`(()=>{const es=[...document.querySelectorAll(${JSON.stringify(selector)})].filter(e=>{const r=e.getBoundingClientRect();return r.width>0&&r.height>0&&getComputedStyle(e).visibility!=='hidden'&&(${JSON.stringify(text??null)}===null||e.textContent.trim()===${JSON.stringify(text??null)});});if(es.length!==1)throw Error('Expected one visible target: '+es.length);const e=es[0],r=e.getBoundingClientRect(),x=r.x+r.width/2,y=r.y+r.height/2;if(e.disabled||x<0||y<0||x>=innerWidth||y>=innerHeight||!e.contains(document.elementFromPoint(x,y)))throw Error('Target unavailable or occluded');return{x,y,tag:e.tagName};})()`);
    observations.push({label:'click-before',selector,text,point});
    await send('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,x:point.x,y:point.y});
    await send('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,x:point.x,y:point.y});
  }
  async function stableModal(){let previous='',stable=0;await wait(async()=>{
    const g=await evaluate(`(()=>{const es=[...document.querySelectorAll('.ant-modal')].filter(e=>{const r=e.getBoundingClientRect();return r.width>0&&r.height>0&&getComputedStyle(e).visibility!=='hidden';});if(es.length!==1)return null;const e=es[0],r=e.getBoundingClientRect();if(e.getAnimations({subtree:true}).some(a=>a.playState==='running'||a.pending)||Number(getComputedStyle(e).opacity)!==1)return null;return{x:r.x,y:r.y,w:r.width,h:r.height};})()`);
    const key=JSON.stringify(g);stable=g&&key===previous?stable+1:0;previous=key;return stable>=2;
  });}
  async function openFetch(){await click('button','common.model.fetchModels');await stableModal();const opened=await observe('opened');assert.equal(opened.fetchCount,0);assert.equal(opened.writes.length,0);await click('.ant-modal button','opencode.fetchModels.fetch');await wait(async()=> (await snapshot()).fetchCount===1);await wait(()=>evaluate(`!!document.querySelector('.ant-modal input[name="grok-4.7"]')`));await stableModal();}
  async function run(name,scenario,fn){phase='setup';observations=[];const start=exceptions.length;const token=`${results.length}-${Date.now()}`;try{
    await send('Page.navigate',{url:baseUrl+'/?scenario='+scenario+'&caseToken='+token});
    await wait(()=>evaluate(`!!window.grokFetchFixture&&grokFetchFixture.isReady()&&grokFetchFixture.snapshot().caseToken===${JSON.stringify(token)}`));
    const initial=await observe('ready');assert.equal(initial.forbidden.length,0);assert.equal(initial.writes.length,0);
    await openFetch();await fn();phase='safety';assert.equal((await snapshot()).forbidden.length,0);assert.equal(exceptions.length,start);
    results.push({name,passed:true,observations,pageExceptions:[]});
  }catch(error){let state;try{state=await snapshot();}catch{}const diagnostics=[];try{await writeFile(path.join(artifactRoot,`case-${results.length}-failure.html`),await evaluate('document.documentElement.outerHTML'));const shot=await send('Page.captureScreenshot',{format:'png'});await writeFile(path.join(artifactRoot,`case-${results.length}-failure.png`),Buffer.from(shot.data,'base64'));}catch(e){diagnostics.push(String(e));}results.push({name,passed:false,phase,error:String(error),assertion:{code:error.code,actualIsUndefined:error.actual===undefined,expected:error.expected},observations,state,pageExceptions:exceptions.slice(start),diagnostics});}}
  async function submit(removeCount=0){
    assert.equal((await snapshot()).writes.length,0);
    await click('.ant-modal input[name="grok-4.7"]');
    await wait(()=>evaluate(`document.querySelector('.ant-modal input[name="grok-4.7"]').checked===true`));
    await observe('selected-before-save');assert.equal((await snapshot()).writes.length,0);
    await click('.ant-modal button','opencode.fetchModels.applyChanges',{addCount:1,removeCount});
    await wait(async()=> (await snapshot()).writes.length===1);
  }
  for(const scenario of ['new','existing','remove-default']) await run(scenario+' catalog merge',scenario,async()=>{
    if(scenario==='existing') assert.equal(await evaluate(`document.querySelector('.ant-modal input[name="grok-4.5"]').disabled`),true);
    if(scenario==='remove-default') await click('.ant-modal label','opencode.fetchModels.removeMissing',{count:1});
    await submit(scenario==='remove-default'?1:0);
    await wait(async()=> (await snapshot()).dialogs.length===0);
    const state=await observe('saved');assert.equal(state.forbidden.length,0);assert.equal(state.saveCount,1);
    const saved=state.writes[0],models=saved.modelCatalog.models;
    assert.deepEqual(JSON.parse(state.providers[0].settingsConfig),saved);
    if(scenario==='existing'){
      const prior=models.find(x=>x.key==='grok-4.5');
      assert.equal(prior.contextWindow,123456);assert.deepEqual(prior.reasoningEfforts,['low','high']);
      assert.equal(prior.reasoningEffort,'low');assert.equal(prior.displayName,'Fixture hand-edited Grok');assert.equal(saved.defaultModelKey,'grok-4.5');
    }else{assert.equal(saved.defaultModelKey,'grok-4.7');assert.equal(models.length,1);}
    const model=models.find(x=>x.key==='grok-4.7');assert(model);assert.equal(model.maxCompletionTokens,undefined);
    phase='invariant:preset-context';assert.equal(model.contextWindow,500000,'Fetched known model lost preset context');phase='interaction';
    assert.deepEqual(model.reasoningEfforts,['low','medium','high','xhigh']);
    assert.deepEqual(model.modalities,{input:['text','image'],output:['text']});
    assert.equal(model.displayName,'Grok 4.7');assert.equal(model.reasoningEffort,'high');
    assert.equal(model.supportsReasoningEffort,true);assert.equal(model.attachment,true);assert.equal(model.supportsImage,true);
    assert.equal(model.baseUrl,'https://grok-fixture.example.invalid/v1');assert.equal(model.apiBackend,'chat_completions');
  });
  await run('save failure does not claim stored catalog','save-failure',async()=>{
    const before=(await snapshot()).providers;
    await submit();
    const errorText=await evaluate(`grokFetchFixture.translate('common.error')`);
    await wait(async()=> (await snapshot()).alerts.some(x=>x.includes(errorText)));
    const state=await observe('save-failure');phase='invariant';
    assert.equal(state.saveCount,1);assert.deepEqual(state.providers,before);
    assert.equal(state.dialogs.length,1);assert(state.alerts.some(x=>x.includes(errorText)));
  });
  const fixtureErrors=results.filter(x=>!x.passed&&!(x.phase==='invariant:preset-context'&&x.assertion?.code==='ERR_ASSERTION'&&x.assertion.actualIsUndefined&&x.assertion.expected===500000&&x.state?.forbidden.length===0&&x.pageExceptions.length===0));
  const validRed=results.length===4&&results.slice(0,3).every(x=>!x.passed&&x.phase==='invariant:preset-context'&&x.assertion?.code==='ERR_ASSERTION'&&x.assertion.actualIsUndefined&&x.assertion.expected===500000&&x.state?.forbidden.length===0&&x.pageExceptions.length===0)&&results[3].passed&&fixtureErrors.length===0&&exceptions.length===0;
  const report={passed:results.filter(x=>x.passed).length,failed:results.filter(x=>!x.passed).length,fixtureErrors:fixtureErrors.length,validRed,results,globalPageExceptions:exceptions};
  await writeFile(path.join(artifactRoot,'grok-fetched-cases.json'),JSON.stringify(report,null,2));
  assert(results.length>0,'No cases is not validation');assert.equal(report.failed,0);assert.equal(exceptions.length,0);return results;
}
