import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runBrowserFixture } from './lib/browser-fixture.mjs';

const fixtureDirectory = fileURLToPath(new URL('../web/test/components/common/AuthConfigModal/fixtures', import.meta.url));
await runBrowserFixture({
  fixtureDirectory,
  fixtureFilename: 'AuthConfigGuardFixture.jsx',
  artifactPrefix: 'auth-config-guard-',
  fixtureAliases: [{
    find: 'monaco-editor/esm/vs/language/json/monaco.contribution',
    replacement: fileURLToPath(new URL('../web/test/components/common/JsonEditor/fixtures/JsonLanguageContributionStub.js', import.meta.url)),
  }],
  async verify({ send, evaluate, baseUrl, artifactRoot }) {
    const checks = [];
    const waitFor = async expression => {
      for (let attempt = 0; attempt < 200; attempt++) {
        if (await evaluate(expression)) return;
        await new Promise(resolve => setTimeout(resolve, 50));
      }
      throw new Error('Timed out waiting for: ' + expression);
    };
    const check = (name, actual, expected = true) => {
      assert.deepEqual(actual, expected, name);
      checks.push(name);
      console.log('PASS ' + name);
    };
    const saveVisible = 'Boolean(document.querySelector(".ant-modal-footer .ant-btn-primary"))';
    for (const theme of ['light', 'dark']) {
      await send('Emulation.setDeviceMetricsOverride', { width: theme === 'dark' ? 700 : 1280, height: 900, deviceScaleFactor: 1, mobile: false });
      await send('Page.navigate', { url: `${baseUrl}/?theme=${theme}` });
      await waitFor('window.authGuard && authGuard.editorReadOnly() === true');
      check(`${theme}: unknown mode hides Save`, await evaluate(saveVisible), false);
      await evaluate('authGuard.begin("failed"); authGuard.reject("failed")');
      await waitFor('authGuard.state.settled.includes("failed")');
      check(`${theme}: failed mode keeps Monaco read-only`, await evaluate('authGuard.editorReadOnly()'));
      check(`${theme}: failed mode never offers Save`, await evaluate(saveVisible), false);
      await evaluate('authGuard.begin("v1"); authGuard.resolve("v1", false)');
      await waitFor(`${saveVisible} && authGuard.editorReadOnly() === false`);
      check(`${theme}: confirmed V1 unlocks the real editor`, await evaluate('authGuard.editorReadOnly()'), false);
      await evaluate('authGuard.begin("old")');
      await waitFor('authGuard.editorReadOnly() === true');
      check(`${theme}: pending probe hides Save immediately`, await evaluate(saveVisible), false);
      await evaluate('authGuard.begin("v2"); authGuard.resolve("v2", true)');
      await waitFor('authGuard.state.settled.includes("v2") && authGuard.editorReadOnly() === true');
      await evaluate('authGuard.resolve("old", false)');
      await waitFor('authGuard.state.settled.includes("old")');
      check(`${theme}: late V1 cannot restore Save`, await evaluate(saveVisible), false);
      check(`${theme}: late V1 cannot unlock Monaco`, await evaluate('authGuard.editorReadOnly()'));
      const { data } = await send('Page.captureScreenshot', { format: 'png' });
      await writeFile(path.join(artifactRoot, `${theme}-v2-read-only.png`), Buffer.from(data, 'base64'));
      await evaluate('authGuard.begin("retry"); authGuard.resolve("retry", false)');
      await waitFor(`${saveVisible} && authGuard.editorReadOnly() === false`);
      await evaluate('authGuard.edit(JSON.stringify({synthetic:{key:"edited"}}))');
      await waitFor('document.querySelector(".monaco-editor").textContent.includes("edited")');
      await evaluate('document.querySelector(".ant-modal-footer .ant-btn-primary").click()');
      await waitFor('authGuard.state.saved === 1 && authGuard.state.closed === 1 && authGuard.state.refreshed === 1');
      check(`${theme}: actual modal submits edited and opened values`, await evaluate('[authGuard.state.submitted, authGuard.state.original]'), [
        { synthetic: { key: 'edited' } }, { synthetic: { key: 'original' } },
      ]);
      await evaluate('authGuard.open()');
      await waitFor(saveVisible);
      await evaluate('document.querySelector(".ant-modal-close").click()');
      await waitFor('authGuard.state.closed === 2');
      check(`${theme}: cancel and reopen never cause another write`, await evaluate('authGuard.state.saved'), 1);
    }
    return checks;
  },
});
