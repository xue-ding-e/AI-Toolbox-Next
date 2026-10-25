import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

// Real page/form events only. The fixture controls synthetic API completion, not handlers.
export async function verifyGrokEditor({ send, evaluate, baseUrl, exceptions, artifactRoot }) {
  const results = [];
  let observations = [], phase = 'setup';
  const snap = () => evaluate('grokEditorFixture.snapshot()');
  const observe = async label => { const state = await snap(); observations.push({ label, state }); return state; };
  async function wait(fn) {
    const deadline = Date.now() + 6000;
    while (Date.now() < deadline) { if (await fn()) return; await new Promise(resolve => setTimeout(resolve, 25)); }
    throw new Error('Fixture readiness deadline');
  }
  const visibleDefinition = `e => { const r=e.getBoundingClientRect(); return r.width>0 && r.height>0 && getComputedStyle(e).visibility!=='hidden'; }`;
  async function click(selector) {
    phase = 'interaction';
    // Wait for motion and geometry to settle; never force-click or retry the click.
    let previous = '', stable = 0, point;
    await wait(async () => {
      const current = await evaluate(`(() => {
        const nodes=[...document.querySelectorAll(${JSON.stringify(selector)})].filter(${visibleDefinition});
        if(nodes.length!==1) return null;
        const e=nodes[0],r=e.getBoundingClientRect(),x=r.x+r.width/2,y=r.y+r.height/2;
        // A loading spinner is decorative infinite motion, not a layout barrier.
        if(e.getAnimations({subtree:true}).some(a=>(a.playState==='running'||a.pending)&&a.effect?.getTiming().iterations!==Infinity)) return null;
        if(x<0||y<0||x>=innerWidth||y>=innerHeight||!e.contains(document.elementFromPoint(x,y))) return null;
        return {x,y,width:r.width,height:r.height};
      })()`);
      const key=JSON.stringify(current); stable=current && key===previous ? stable+1 : 0; previous=key; point=current;
      return stable>=2;
    });
    observations.push({ label: 'click', selector, point });
    await send('Input.dispatchMouseEvent', { type: 'mousePressed', button: 'left', clickCount: 1, x: point.x, y: point.y });
    await send('Input.dispatchMouseEvent', { type: 'mouseReleased', button: 'left', clickCount: 1, x: point.x, y: point.y });
  }
  const modal = '.ant-modal';
  const save = '.ant-modal-footer .ant-btn-primary';
  const cancel = '.ant-modal-footer button:not(.ant-btn-primary)';
  async function openEditor() {
    const expanded=await evaluate(`document.querySelector('[data-provider-id="fixture-grok"] .ant-collapse-header')?.getAttribute('aria-expanded')`);
    if(expanded==='false') await click('[data-provider-id="fixture-grok"] .ant-collapse-expand-icon');
    // ModelItem exposes data-model-row and AntD EditOutlined exposes aria-label=edit.
    await click('[data-provider-id="fixture-grok"] [data-model-row]:has([title*="(grok-4.5"]) button:has([aria-label="edit"])');
    await wait(async () => (await snap()).dialogs.length===1);
    await wait(() => evaluate(`!!document.querySelector('.ant-modal input#displayName')`));
  }
  async function fields() {
    return evaluate(`(() => { const m=[...document.querySelectorAll('${modal}')].filter(${visibleDefinition}); if(m.length!==1) throw Error('Expected one editor'); const e=m[0]; return { name:e.querySelector('input#displayName')?.value, context:e.querySelector('input#contextWindow')?.value, efforts:[...e.querySelectorAll('#reasoningEfforts')].map(x=>x.closest('.ant-select')?.textContent), effort:e.querySelector('#reasoningEffort')?.closest('.ant-select')?.textContent }; })()`);
  }
  async function rename(value) {
    await click('.ant-modal input#displayName');
    await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'a', code: 'KeyA', modifiers: 2, windowsVirtualKeyCode: 65 });
    await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'a', code: 'KeyA', modifiers: 2, windowsVirtualKeyCode: 65 });
    await send('Input.insertText', { text: value });
    assert.equal((await fields()).name, value);
  }
  async function closed() { await wait(async () => (await snap()).dialogs.length===0); }
  async function assertStored(before, name, count=1) {
    const state=await observe('stored');
    const prior=JSON.parse(before[0].settingsConfig), after=JSON.parse(state.providers[0].settingsConfig);
    const expected=structuredClone(prior); expected.modelCatalog.models.find(x=>x.key==='grok-4.5').displayName=name;
    // upsert intentionally moves the edited row to the end; order is not a catalog contract.
    for(const settings of [after,expected]) settings.modelCatalog.models.sort((a,b)=>a.key.localeCompare(b.key));
    assert.deepEqual(after,expected, 'Requested label changes while catalog fields remain intact');
    assert.equal(state.saveCount,count);
  }
  for (const scenario of ['readback','cancel','save','pending','retry','remount']) {
    phase='setup'; observations=[]; const exceptionStart=exceptions.length;
    try {
      const token=`editor-${scenario}-${Date.now()}`;
      await send('Page.navigate',{url:baseUrl+'/?scenario='+scenario+'&caseToken='+token});
      await wait(() => evaluate(`!!window.grokEditorFixture && grokEditorFixture.isReady() && grokEditorFixture.snapshot().caseToken===${JSON.stringify(token)}`));
      const initial=await observe('ready'), before=initial.providers;
      assert.equal(initial.forbidden.length,0); assert.deepEqual(initial.routing.unexpected,[]); assert.equal(initial.saveCount,0);
      await openEditor(); const original=await fields(); phase='invariant';
      assert.equal(original.name,'Fixture hand-edited Grok'); assert.equal(Number(original.context),123456);
      assert(original.efforts.join(' ').includes('low') && original.efforts.join(' ').includes('high'));
      assert(original.effort.includes('low')); assert.equal((await snap()).saveCount,0);
      if(scenario==='readback') { await click(cancel); await closed(); }
      else if(scenario==='cancel') {
        await rename('Discard this draft'); await click(cancel); await closed(); await openEditor();
        phase='invariant'; assert.deepEqual(await fields(),original); assert.deepEqual((await snap()).providers,before); assert.equal((await snap()).saveCount,0);
        await click(cancel); await closed();
      } else {
        const name='Saved synthetic label'; await rename(name); await click(save);
        if(scenario==='pending') {
          await wait(async () => (await snap()).saveCount===1);
          assert.equal(await evaluate(`document.querySelector('${save}')?.classList.contains('ant-btn-loading')`),true);
          await click(save); // A second ordinary pointer click, without bypassing loading/disabled behavior.
          await new Promise(resolve=>setTimeout(resolve,200)); phase='invariant'; assert.equal((await snap()).saveCount,1);
          await evaluate('grokEditorFixture.releaseSave()');
        }
        if(scenario==='retry') {
          const errorText=await evaluate(`grokEditorFixture.translate('common.error')`);
          await wait(async () => (await snap()).alerts.some(x=>x.includes(errorText)));
          phase='invariant'; const failed=await observe('rejected'); assert.equal(failed.saveCount,1); assert.deepEqual(failed.providers,before); assert.equal(failed.dialogs.length,1);
          await click(save); await closed();
          assert.equal((await snap()).saveCount,2);
          await assertStored(before,name,2);
        } else { await closed(); await assertStored(before,name); }
        if(scenario==='remount') { await evaluate('grokEditorFixture.remount()'); await wait(()=>evaluate('grokEditorFixture.isReady()')); }
        await openEditor(); phase='invariant'; assert.equal((await fields()).name,name); await click(cancel); await closed();
      }
      phase='safety'; const final=await observe('final');
      assert.deepEqual(final.routing.unexpected,[], 'Editor cases must not use the excluded router boundary');
      const expectedWrites=scenario==='retry'?2:((scenario==='readback'||scenario==='cancel')?0:1);
      assert.equal(final.saveCount,expectedWrites); assert.equal(final.writes.length,expectedWrites);
      if(expectedWrites===0) assert.deepEqual(final.providers,before);
      else await assertStored(before,'Saved synthetic label',expectedWrites); assert.equal(final.forbidden.length,0); assert.equal(final.calls.includes('fetch_provider_models'),false); assert.equal(exceptions.length,exceptionStart);
      results.push({scenario,passed:true,observations});
    } catch(error) {
      const diagnostics=[];
      try { await writeFile(path.join(artifactRoot,`editor-${scenario}-failure.html`),await evaluate('document.documentElement.outerHTML')); const shot=await send('Page.captureScreenshot',{format:'png'}); await writeFile(path.join(artifactRoot,`editor-${scenario}-failure.png`),Buffer.from(shot.data,'base64')); } catch(e) { diagnostics.push(String(e)); }
      results.push({scenario,passed:false,phase,error:String(error),observations,pageExceptions:exceptions.slice(exceptionStart),diagnostics});
    }
  }
  const report={results,passed:results.filter(x=>x.passed).length,failed:results.filter(x=>!x.passed).length,pageExceptions:exceptions};
  await writeFile(path.join(artifactRoot,'grok-editor-cases.json'),JSON.stringify(report,null,2));
  assert.equal(results.length,6); assert.equal(report.failed,0); assert.equal(exceptions.length,0);
  return results;
}
