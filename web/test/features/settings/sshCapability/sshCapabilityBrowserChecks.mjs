import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import path from 'node:path';
import {createSSHFixtureInteractions, waitForSSHFixtureState} from '../sshRequest/sshRequestBrowserInteractions.mjs';

// Explicit feature expectations are independent of the runtime locale files.
const EXPECTED = {
  "en-US": {
    "authenticationSucceeded": "Authentication succeeded for this test",
    "authenticationTimeout": "SSH authentication timed out.",
    "connectionFailed": "Connection Failed",
    "capability": {
      "supported": "POSIX shell syntax and HOME verified. Required sync tools and write permissions have not been checked.",
      "missingHome": "Authentication succeeded, but HOME is empty. SSH sync prerequisites are not met.",
      "unknown": "SSH sync prerequisites have not been verified. Authentication alone does not confirm sync support."
    },
    "probeReason": {
      "channelOpenFailed": "The probe channel could not be opened.",
      "execFailed": "The probe command could not be started.",
      "channelFailure": "The probe channel failed.",
      "channelClosedEarly": "The probe channel closed before completion.",
      "outputLimitExceeded": "The probe output exceeded the safe limit.",
      "probeTimeout": "The probe timed out.",
      "exitSignal": "The probe ended with an exit signal.",
      "conflictingExitStatus": "The probe returned conflicting exit statuses.",
      "missingExitStatus": "The probe did not return an exit status.",
      "nonzeroExitStatus": "The probe returned a nonzero exit status.",
      "unexpectedOutput": "The probe response did not match the expected protocol."
    },
    "cleanupWarning": {
      "title": "Test connection cleanup warning",
      "disconnectFailed": "Disconnect failed. This does not change the authentication or probe result.",
      "disconnectTimeout": "Disconnect timed out. This does not change the authentication or probe result.",
      "unknown": "Connection cleanup could not be confirmed."
    }
  },
  "zh-CN": {
    "authenticationSucceeded": "本次测试认证成功",
    "authenticationTimeout": "SSH 认证超时。",
    "connectionFailed": "连接失败",
    "capability": {
      "supported": "已核验 POSIX shell 语法和 HOME；尚未检查同步所需工具及写入权限。",
      "missingHome": "认证成功，但 HOME 为空，不满足 SSH 同步前提。",
      "unknown": "尚未核验 SSH 同步前提；仅认证成功不代表支持同步。"
    },
    "probeReason": {
      "channelOpenFailed": "无法打开探测通道。",
      "execFailed": "无法启动探测命令。",
      "channelFailure": "探测通道失败。",
      "channelClosedEarly": "探测完成前通道已关闭。",
      "outputLimitExceeded": "探测输出超过安全上限。",
      "probeTimeout": "探测超时。",
      "exitSignal": "探测因退出信号而结束。",
      "conflictingExitStatus": "探测返回了冲突的退出状态。",
      "missingExitStatus": "探测未返回退出状态。",
      "nonzeroExitStatus": "探测返回了非零退出状态。",
      "unexpectedOutput": "探测响应不符合预期协议。"
    },
    "cleanupWarning": {
      "title": "测试连接清理警告",
      "disconnectFailed": "断开连接失败；不影响本次认证或探测结果。",
      "disconnectTimeout": "断开连接超时；不影响本次认证或探测结果。",
      "unknown": "无法确认连接清理结果。"
    }
  }
};

// Capture only this local synthetic fixture. Diagnostics never replace the
// original assertion failure; record capture errors separately for inspection.
async function captureEvidence({send,evaluate,artifactRoot,stem,includeDOM=false}) {
  const evidence={files:[],diagnosticErrors:[]};
  if(includeDOM) {
    try {
      const filename=stem+'.html';
      await writeFile(path.join(artifactRoot,filename),await evaluate('document.documentElement.outerHTML'));
      evidence.files.push(filename);
    } catch(error) {evidence.diagnosticErrors.push('DOM: '+String(error));}
  }
  try {
    const filename=stem+'.png';
    const shot=await send('Page.captureScreenshot',{format:'png'});
    await writeFile(path.join(artifactRoot,filename),Buffer.from(shot.data,'base64'));
    evidence.files.push(filename);
  } catch(error) {evidence.diagnosticErrors.push('screenshot: '+String(error));}
  return evidence;
}

// These are synthetic future IPC responses, not evidence that the old backend
// emitted them or that a real SSH server was tested. Baseline failures only mean
// the new capability UI contract is not yet supported.
export async function verifySSHCapability({send,evaluate,baseUrl,exceptions,artifactRoot}) {
  const results=[];
  let serial=0;
  const snapshot=()=>evaluate('sshRequestFixture.snapshot()');
  const wait=waitForSSHFixtureState;
  const drain=async()=>{await evaluate('sshRequestFixture.drain()');await wait(async()=>!(await snapshot()).animations.length);};
  for(const lang of ['en-US','zh-CN']) {
    const locale=EXPECTED[lang];
    const cases=[
      {name:'authentication failure control',control:true,response:{connected:false,error:'synthetic-auth-failure'},capability:null,error:'synthetic-auth-failure'},
      {name:'authentication timeout localized',response:{connected:false,error:'SSH 认证超时'},capability:null,error:locale.authenticationTimeout},
      {name:'authentication failure with cleanup',response:{connected:false,error:'synthetic-auth-failure',cleanupWarning:'disconnectFailed'},capability:null,error:'synthetic-auth-failure',cleanup:'disconnectFailed'},
      {name:'supported ignores contradictory reason',response:{connected:true,syncCapability:'supported',probeReason:'probeTimeout'},capability:'supported',absentReason:'probeTimeout'},
      {name:'legacy missing capability',response:{connected:true},capability:'unknown'},
      {name:'supported narrow probe',response:{connected:true,syncCapability:'supported'},capability:'supported'},
      {name:'empty HOME',response:{connected:true,syncCapability:'missingHome'},capability:'missingHome'},
      {name:'unknown without reason',response:{connected:true,syncCapability:'unknown'},capability:'unknown'},
      ...Object.keys(locale.probeReason).map(reason=>({name:`unknown ${reason}`,response:{connected:true,syncCapability:'unknown',probeReason:reason},capability:'unknown',reason})),
      ...['disconnectFailed','disconnectTimeout'].map(cleanup=>({name:`supported with ${cleanup}`,response:{connected:true,syncCapability:'supported',cleanupWarning:cleanup},capability:'supported',cleanup})),
      {name:'unknown and cleanup remain separate',response:{connected:true,syncCapability:'unknown',probeReason:'probeTimeout',cleanupWarning:'disconnectTimeout'},capability:'unknown',reason:'probeTimeout',cleanup:'disconnectTimeout'},
      {name:'unrecognized codes never shown',response:{connected:true,syncCapability:'synthetic-secret-capability',probeReason:'synthetic-secret-reason',cleanupWarning:'synthetic-secret-cleanup'},capability:'unknown',cleanup:'unknown'},
    ];
    if(lang==='en-US')cases.push(
      {name:'B warning survives late A supported',response:{connected:true,syncCapability:'unknown',probeReason:'probeTimeout',serverInfo:'B-result'},capability:'unknown',reason:'probeTimeout',stale:{connected:true,syncCapability:'supported',serverInfo:'A-result'}},
      {name:'late A cleanup cannot contaminate B supported',response:{connected:true,syncCapability:'supported',serverInfo:'B-result'},capability:'supported',stale:{connected:true,syncCapability:'unknown',cleanupWarning:'disconnectFailed',serverInfo:'A-result'}},
    );
    for(const spec of cases) {
      const caseId=`capability-${serial++}`;
      const exceptionStart=exceptions.length;
      let state,phase='setup',selectorCategory=null;const observations=[];
      const {chooseB}=createSSHFixtureInteractions({send,evaluate,snapshot,drain,
        onPhase:(step,category)=>{phase='interaction:'+step;selectorCategory=category;}});
      try {
        await send('Page.navigate',{url:`${baseUrl}/?case=${caseId}&lang=${lang}`});
        await wait(async()=>evaluate(`!!window.sshRequestFixture && sshRequestFixture.snapshot().caseId===${JSON.stringify(caseId)} && sshRequestFixture.snapshot().calls.some(c=>c.id==='A')`));
        await drain();
        let before=await snapshot();observations.push({label:'ready',state:before});
        let pending=before.calls.flatMap((c,i)=>c.id==='A'&&!c.settled?[i]:[]);
        const stale=spec.stale?pending:[];
        if(spec.stale) {
          await chooseB();
          before=await snapshot();pending=before.calls.flatMap((c,i)=>c.id==='B'&&!c.settled?[i]:[]);
        }
        assert.ok(pending.length>0);observations.push({label:'before settlement',state:before});
        phase='settlement';
        await evaluate(`sshRequestFixture.settle(${JSON.stringify(pending)},false,${JSON.stringify(spec.response)})`);
        await drain();
        if(spec.stale) {
          await evaluate(`sshRequestFixture.settle(${JSON.stringify(stale)},false,${JSON.stringify(spec.stale)})`);
          await drain();
        }
        state=await snapshot();observations.push({label:'after settlement',state});
        phase='safety';
        assert.equal(state.forbidden,0);
        assert.equal(exceptions.length,exceptionStart,'Page exception');
        assert.equal(state.calls.length,before.calls.length,'settlement must not start another request');
        assert.equal(state.spinners,0);
        phase='expectation';
        if(spec.stale){assert.ok(state.text.includes('B-result'));assert.ok(!state.text.includes('A-result'));}
        const capability=state.alerts.filter(a=>a.id==='ssh-sync-capability');
        const cleanup=state.alerts.filter(a=>a.id==='ssh-test-cleanup-warning');
        if(spec.capability) {
          assert.ok(state.text.includes(locale.authenticationSucceeded));
          assert.equal(capability.length,1);
          assert.equal(capability[0].type,spec.capability==='supported'?'success':'warning');
          assert.ok(capability[0].text.includes(locale.capability[spec.capability]));
          if(spec.reason)assert.ok(capability[0].text.includes(locale.probeReason[spec.reason]));
          if(spec.absentReason)assert.ok(!capability[0].text.includes(locale.probeReason[spec.absentReason]));
        } else {
          assert.equal(capability.length,0);
          assert.ok(state.text.includes(locale.connectionFailed));
          assert.ok(state.text.includes(spec.error));
        }
        assert.equal(cleanup.length,spec.cleanup?1:0);
        if(spec.cleanup) {
          assert.equal(cleanup[0].type,'warning');
          assert.ok(cleanup[0].text.includes(locale.cleanupWarning.title));
          assert.ok(cleanup[0].text.includes(locale.cleanupWarning[spec.cleanup]));
        }
        assert.ok(!state.text.includes('synthetic-secret-'));
        assert.ok(!state.text.includes('settings.ssh.capability.'));
        assert.ok(!state.text.includes('settings.ssh.probeReason.'));
        // Two representative localized views per language, not every enum row.
        const evidence=['unknown and cleanup remain separate','supported with disconnectFailed'].includes(spec.name)
          ? await captureEvidence({send,evaluate,artifactRoot,stem:`${caseId}-${lang}-representative`}) : undefined;
        results.push({name:spec.name,lang,control:!!spec.control,passed:true,state,observations,evidence});
      } catch(error) {
        // Only synthetic case metadata, never DOM, URLs or connection fields.
        console.error(JSON.stringify({suite:'ssh-capability',case:spec.name,lang,phase,selectorCategory,error:String(error)}));
        const pageExceptions=exceptions.slice(exceptionStart);
        const classification=phase==='expectation'&&pageExceptions.length===0&&state?.forbidden===0?'expectation-failed':'fixture-or-control-error';
        const evidence=await captureEvidence({send,evaluate,artifactRoot,stem:`${caseId}-${lang}-failure`,includeDOM:true});
        results.push({name:spec.name,lang,control:!!spec.control,passed:false,phase,selectorCategory,evidence,
          classification,error:String(error),state,observations,pageExceptions});
      }
    }
  }
  await writeFile(path.join(artifactRoot,'ssh-capability-cases.json'),JSON.stringify({syntheticBoundaryOnly:true,results},null,2));
  assert.equal(results.filter(r=>!r.passed).length,0,'Capability expectations failed; not a real-server bug reproduction');
  return results;
}

// Baseline mode is deliberately narrow: use the old Modal with the new boundary
// fixture and demonstrate only absent new UI, after proving old auth still works.
// It does not claim that any old backend actually produced these new fields.
export async function verifySSHCapabilityBaseline({send,evaluate,baseUrl,exceptions,artifactRoot}) {
  const results=[];
  const snapshot=()=>evaluate('sshRequestFixture.snapshot()');
  const wait=async predicate=>{
    for(let i=0;i<200;i++){if(await predicate())return;await new Promise(r=>setTimeout(r,20));}
    throw Error('Baseline UI state deadline');
  };
  const drain=async()=>{await evaluate('sshRequestFixture.drain()');await wait(async()=>!(await snapshot()).animations.length);};
  for(const capability of ['unknown','missingHome']) {
    const caseId=`baseline-${capability}`;
    const exceptionStart=exceptions.length;
    const observations=[];
    let state,phase='setup';
    try {
      await send('Page.navigate',{url:`${baseUrl}/?case=${caseId}&lang=en-US`});
      await wait(()=>evaluate(`!!window.sshRequestFixture && sshRequestFixture.snapshot().caseId===${JSON.stringify(caseId)} && sshRequestFixture.snapshot().calls.some(c=>c.id==='A')`));
      await drain();
      const before=await snapshot();observations.push({label:'ready',state:before});
      const pending=before.calls.flatMap((c,i)=>c.id==='A'&&!c.settled?[i]:[]);
      assert.ok(pending.length>0);
      phase='settlement';
      await evaluate(`sshRequestFixture.settle(${JSON.stringify(pending)},false,${JSON.stringify({connected:true,syncCapability:capability,serverInfo:caseId+'-result'})})`);
      await drain();state=await snapshot();observations.push({label:'settled',state});
      phase='existing-auth-control';
      assert.equal(exceptions.length,exceptionStart,'Page exception');
      assert.equal(state.forbidden,0);
      assert.equal(state.spinners,0);
      assert.equal(state.calls.length,before.calls.length,'Unexpected request inventory change');
      assert.ok(state.calls.every((c,i)=>c.id===before.calls[i].id&&c.settled===true));
      assert.ok(state.visible);
      assert.ok(state.text.includes(caseId+'-result'),'Current request identity');
      // Exact pre-feature en-US settings.ssh.connected label. Do not require the
      // new authenticationSucceeded key, which would mask the capability check.
      assert.equal(await evaluate(`(()=>{const tags=[...document.querySelectorAll('.ant-modal .ant-tag')].filter(e=>{const r=e.getBoundingClientRect();return r.width>0&&r.height>0&&getComputedStyle(e).visibility!=='hidden';});return tags.filter(e=>e.textContent==='Connected'&&e.classList.contains('ant-tag-success')).length;})()`),1);
      assert.equal(state.alerts.filter(a=>a.id==='ssh-test-cleanup-warning').length,0);
      phase='new-capability-presence';
      const actual=state.alerts.filter(a=>a.id==='ssh-sync-capability').length;
      assert.equal(actual,0,'Baseline must show the exact absence, not a different UI failure');
      const evidence=await captureEvidence({send,evaluate,artifactRoot,stem:`${caseId}-old-ui`});
      results.push({name:caseId,passed:true,classification:'missing-new-capability-ui',evidence,
        newFeatureExpectation:{expected:1,actual:0},existingAuthControlsPassed:true,syntheticBoundaryOnly:true,observations});
    } catch(error) {
      console.error(JSON.stringify({suite:'ssh-capability-baseline',case:caseId,phase,selectorCategory:null,error:String(error)}));
      const evidence=await captureEvidence({send,evaluate,artifactRoot,stem:`${caseId}-failure`,includeDOM:true});
      results.push({name:caseId,passed:false,evidence,classification:'baseline-control-failed',phase,error:String(error),state,observations,pageExceptions:exceptions.slice(exceptionStart)});
    }
  }
  await writeFile(path.join(artifactRoot,'ssh-capability-baseline-cases.json'),JSON.stringify({syntheticBoundaryOnly:true,results},null,2));
  assert.equal(results.filter(r=>!r.passed).length,0,'Baseline controls failed; do not attribute this to missing capability UI');
  return results;
}
