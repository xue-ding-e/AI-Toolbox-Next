import assert from 'node:assert/strict';

// Real CodexPage and AntD keyboard/modal behavior; only Tauri IPC is synthetic.
export async function verifyCodexAuthEntry({ send, evaluate, baseUrl }) {
  const checks = [];
  const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
  const until = async expression => {
    for (let i = 0; i < 200; i++) { if (await evaluate(expression)) return; await wait(50); }
    throw new Error('Timed out: ' + expression);
  };
  const check = async (name, expression, expected = true) => {
    assert.deepEqual(await evaluate(expression), expected, name); checks.push(name); console.log('PASS ' + name);
  };
  const key = async value => {
    await send('Input.dispatchKeyEvent', { type: 'keyDown', key: value, code: value, windowsVirtualKeyCode: value === 'Enter' ? 13 : 27 });
    await send('Input.dispatchKeyEvent', { type: 'keyUp', key: value, code: value, windowsVirtualKeyCode: value === 'Enter' ? 13 : 27 });
  };
  for (const theme of ['light', 'dark']) {
    await send('Page.navigate', { url: `${baseUrl}/?viewMode=card&authExport=1&theme=${theme}` });
    await until('window.codexCardViewFixture?.renderedOrder().length === 4');
    const center = await evaluate('codexCardViewFixture.cardCenter("Official Codex")');
    for (const type of ['mousePressed', 'mouseReleased']) await send('Input.dispatchMouseEvent', { type, ...center, button: 'left', clickCount: 1 });
    await until('document.querySelectorAll("button[aria-label=\\"auth.json\\"]").length === 2');
    await evaluate('document.querySelectorAll("button[aria-label=\\"auth.json\\"]")[1].focus()');
    await key('Enter');
    await until('Boolean(document.querySelector(".ant-modal-content"))');
    await check(theme + ' row entry performs no implicit credential read', 'codexCardViewFixture.state.requests.filter(r => r.command === "export_codex_official_account_auth_json").length', 0);
    await evaluate('[...document.querySelectorAll(".ant-modal-content button")].find(b=>b.textContent.includes("生成并预览")).click()');
    await until('codexCardViewFixture.state.requests.some(r => r.command === "export_codex_official_account_auth_json")');
    await check(theme + ' preview selects second account', 'codexCardViewFixture.state.requests.filter(r => r.command === "export_codex_official_account_auth_json").at(-1).args', { providerId: 'provider-c', accountId: 'account-2', copyToClipboard: false });
    await evaluate('[...document.querySelectorAll(".ant-modal-content button")].find(b=>b.textContent.includes("复制 auth.json")).click()');
    await until('codexCardViewFixture.state.authCopies?.length === 1');
    await check(theme + ' explicit copy uses same account', 'JSON.parse(codexCardViewFixture.state.authCopies[0]).tokens.access_token', 'synthetic-account-2');
    await key('Escape');
    await until('!document.querySelector(".ant-modal-content") || document.querySelector(".ant-modal-wrap").style.display === "none"');
    await evaluate('document.querySelectorAll("button[aria-label=\\"auth.json\\"]")[0].click(); codexCardViewFixture.state.deferAuth = true');
    await until('Boolean([...document.querySelectorAll(".ant-modal-content button")].find(b=>b.textContent.includes("生成并预览")))');
    await evaluate('[...document.querySelectorAll(".ant-modal-content button")].find(b=>b.textContent.includes("生成并预览")).click()');
    await until('Boolean(codexCardViewFixture.state.resolveAuth)');
    await check(theme + ' duplicate operation disabled', '[...document.querySelectorAll(".ant-modal-content button")].find(b=>b.textContent.includes("复制 auth.json")).disabled');
    await key('Escape');
    await evaluate('codexCardViewFixture.state.resolveAuth()');
    await wait(200);
    await check(theme + ' late response after Escape leaves no secret editor', 'document.querySelectorAll(".ant-modal-content .monaco-editor").length', 0);
  }
  return checks;
}
