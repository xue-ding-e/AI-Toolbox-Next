import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

export async function verifyGatewayPrivacy({ send, evaluate, baseUrl, artifactRoot }) {
  const checks = [];
  const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
  const check = (name, actual, expected = true) => { assert.deepEqual(actual, expected, name); checks.push(name); console.log('PASS ' + name); };
  const waitFor = async expression => {
    for (let attempt = 0; attempt < 150; attempt++) { if (await evaluate(expression)) return; await delay(50); }
    throw new Error('Timed out: ' + expression);
  };
  const open = async (parameters = '') => {
    const runId = randomUUID();
    await send('Page.navigate', { url: baseUrl + '/?' + parameters + '&runId=' + runId });
    await waitFor('window.privacyFixture?.state.runId === ' + JSON.stringify(runId) + ' && !document.querySelector("[aria-labelledby=gateway-privacy-toggle-label]")?.disabled');
    await delay(100);
  };
  const action = expression => evaluate('privacyFixture.' + expression);
  const click = key => action('click(' + JSON.stringify(key) + ')');
  const input = (selector, text) => action('input(' + JSON.stringify(selector) + ',' + JSON.stringify(text) + ')');
  const settleAnimations = () => evaluate('Promise.all(document.getAnimations().filter(animation => animation.effect?.getTiming().iterations !== Infinity).map(animation => animation.finished.catch(() => {})))');
  const selectedOptionFits = () => evaluate('(() => { const selected = document.querySelector(".ant-modal .ant-select-content"); return !!selected && selected.scrollWidth <= selected.clientWidth; })()');
  const screenshotSettings = async name => {
    const clip = await evaluate('(() => { const rect = document.querySelector("main section").getBoundingClientRect(); return {x:rect.x-4,y:rect.y-4,width:rect.width+8,height:rect.height+8,scale:2}; })()');
    const { data } = await send('Page.captureScreenshot', { format: 'png', clip });
    await writeFile(path.join(artifactRoot, name + '.png'), Buffer.from(data, 'base64'));
  };

  await open();
  check('protection starts disabled', await action('state.settings.enabled'), false);
  await click('gateway.privacy.manage');
  await click('gateway.privacy.tabs.allowlist');
  await input('.ant-modal textarea', 'first\n');
  check('editing the allowlist preserves the trailing newline', await evaluate('document.querySelector(".ant-modal textarea").value'), 'first\n');
  await input('.ant-modal textarea', 'first\n second ');
  await click('gateway.privacy.tabs.test');
  await input('.ant-tabs-tabpane-active textarea', 'sensitive test text');
  await action('state.deferPreview = true');
  await click('gateway.privacy.testRun');
  await input('.ant-tabs-tabpane-active textarea', 'new input');
  await action('resolvePreview()');
  check('late preview results cannot overwrite newer input', await evaluate('!document.body.textContent.includes("stale-preview")'));
  await click('gateway.privacy.testRun');
  check('local testing does not save or enable protection', await action('state.settings.enabled'), false);
  check('preview uses only the local preview command', await action('state.requests.filter(request => request.command.includes("update")).length'), 0);
  await click('gateway.privacy.tabs.custom');
  await click('gateway.privacy.addRule');
  await click('gateway.privacy.save');
  check('invalid save keeps the editor and shows an inline error', await evaluate('!!document.querySelector(".ant-modal [role=alert]")'));
  await input('.ant-modal input[placeholder]', 'Local secret');
  await input('.ant-tabs-tabpane-active textarea', 'literal-secret');
  await action('state.settings.enabled = true');
  await click('gateway.privacy.save');
  await waitFor('!document.querySelector(".ant-modal")');
  check('saving rules preserves a concurrent backend toggle', await action('state.settings.enabled'));
  check('rule submission updates only rules', await evaluate('Object.keys(privacyFixture.state.requests.filter(request => request.command.includes("update")).at(-1).args.update)'), ['rules']);
  check('allowed values preserve spaces during the form-save-read round trip', await action('state.settings.rules.allowlist'), ['first', ' second ']);
  await action('state.failSave = true');
  await action('toggle()');
  check('failed toggle keeps the saved preference and shows an error', await evaluate('document.querySelector("[aria-labelledby=gateway-privacy-toggle-label]").getAttribute("aria-checked") === "true" && !!document.querySelector("[role=alert]")'));
  await action('toggle()');
  check('toggle can retry after failure', await action('state.settings.enabled'), false);
  await click('gateway.privacy.manage');
  await click('gateway.privacy.tabs.custom');
  check('reopening the editor loads the saved rule', await evaluate('document.querySelector(".ant-modal input[placeholder]").value'), 'Local secret');
  await click('gateway.privacy.cancel');

  for (const mode of ['light', 'dark', 'system']) {
    await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'dark' }] });
    await open('theme=' + mode + '&language=' + (mode === 'light' ? 'zh-CN' : 'en-US'));
    await screenshotSettings('settings-' + mode);
    await click('gateway.privacy.manage');
    await click('gateway.privacy.tabs.custom');
    await click('gateway.privacy.addRule');
    await action('selectRegex()');
    await input('.ant-modal input[placeholder]', 'LongRuleName'.repeat(16));
    await input('.ant-tabs-tabpane-active textarea', 'LongSecretValue'.repeat(40));
    await settleAnimations();
    check(mode + ' theme shows the full regular-expression option', await selectedOptionFits());
    check(mode + ' theme separates the action row from the footer explanation', await evaluate('(() => { const button = [...document.querySelectorAll(".ant-tabs-tabpane-active button")].at(-1); const hint = document.querySelector(".ant-modal-body > p:last-child"); return hint.getBoundingClientRect().top - button.getBoundingClientRect().bottom >= 12; })()'));
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    await writeFile(path.join(artifactRoot, mode + '.png'), Buffer.from(data, 'base64'));
    check(mode + ' theme keeps long form values inside the viewport', await evaluate('document.documentElement.scrollWidth <= innerWidth && document.querySelector(".ant-modal").scrollWidth <= document.querySelector(".ant-modal").clientWidth'));
  }
  await send('Emulation.setDeviceMetricsOverride', { width: 600, height: 850, deviceScaleFactor: 1, mobile: false });
  await delay(200);
  check('narrow viewport shows the full regular-expression option', await selectedOptionFits());
  check('narrow viewport does not overflow', await evaluate('document.documentElement.scrollWidth <= innerWidth && document.querySelector(".ant-modal").scrollWidth <= document.querySelector(".ant-modal").clientWidth && document.querySelector(".ant-modal-wrap").scrollWidth <= document.querySelector(".ant-modal-wrap").clientWidth'));
  const { data } = await send('Page.captureScreenshot', { format: 'png' });
  await writeFile(path.join(artifactRoot, 'narrow.png'), Buffer.from(data, 'base64'));
  return checks;
}
