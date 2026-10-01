import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

const EDITORS = ['json', 'jsonc', 'toml', 'markdown'];

export async function verifyJsonEditorBlur({ send, evaluate, baseUrl, artifactRoot }) {
  const checks = [];
  const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
  const check = (name, actual, expected = true) => {
    assert.deepEqual(actual, expected, name); checks.push(name); console.log('PASS ' + name);
  };
  const waitFor = async expression => {
    for (let attempt = 0; attempt < 150; attempt++) {
      if (await evaluate(expression)) return;
      await delay(50);
    }
    throw new Error('Timed out: ' + expression);
  };
  const action = expression => evaluate('jsonEditorBlurFixture.' + expression);
  const click = async selector => {
    const point = await evaluate(`(() => {
      const element = document.querySelector(${JSON.stringify(selector)});
      if (!element) return null;
      const rect = element.getBoundingClientRect();
      return {
        x: Math.round(rect.left + Math.min(rect.width / 2, 40)),
        y: Math.round(rect.top + Math.min(rect.height / 2, 12)),
      };
    })()`);
    if (!point) throw new Error('Missing element: ' + selector);
    for (const type of ['mousePressed', 'mouseReleased']) {
      await send('Input.dispatchMouseEvent', { type, x: point.x, y: point.y, button: 'left', clickCount: 1 });
    }
  };
  const blurCount = editor => `jsonEditorBlurFixture.state.blurs.${editor}.length`;
  // Clicking into the editor and then outside of it is exactly the gesture the
  // panels document as "click outside to save". Headless Chromium does not
  // always hand focus to the editor on a synthetic click, so fall back to
  // focusing/blurring the editor's own focus target directly.
  const blurOnce = async editor => {
    await click(`#fixture-${editor} .monaco-editor .view-lines`);
    if (!await evaluate(`jsonEditorBlurFixture.editorIsFocused(${JSON.stringify(editor)})`)) {
      await action(`focusEditor(${JSON.stringify(editor)})`);
    }
    await waitFor(`jsonEditorBlurFixture.editorIsFocused(${JSON.stringify(editor)})`);
    await click(`#fixture-outside-${editor}`);
    if (await evaluate(`jsonEditorBlurFixture.editorIsFocused(${JSON.stringify(editor)})`)) {
      await action(`blurEditor(${JSON.stringify(editor)})`);
    }
  };
  const screenshot = async name => {
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    await writeFile(path.join(artifactRoot, name + '.png'), Buffer.from(data, 'base64'));
  };

  const runId = randomUUID();
  await send('Page.navigate', { url: baseUrl + '/?runId=' + runId });
  await waitFor(
    'window.jsonEditorBlurFixture?.state.runId === ' + JSON.stringify(runId)
    + ' && document.querySelectorAll(".monaco-editor .view-lines").length === ' + EDITORS.length,
  );
  await delay(400);

  check(
    'every Monaco editor renders in the fixture',
    await evaluate('document.querySelectorAll(".monaco-editor .view-lines").length'),
    EDITORS.length,
  );

  for (const editor of EDITORS) {
    await blurOnce(editor);
    await waitFor(`${blurCount(editor)} === 1`);
    check(`${editor}: the first blur calls the callback of the mounted editor`, await action(`state.blurs.${editor}[0].revision`), 1);

    // The regression from issue #406: a callback captured in `editorDidMount`
    // would stay frozen while the consumer keeps rendering new ones (an MCP
    // server added on another page, a tray switch, an import).
    await action(`revise(${JSON.stringify(editor)}, 4)`);
    await waitFor(`jsonEditorBlurFixture.state.revisions.${editor} === 4`);
    await blurOnce(editor);
    await waitFor(`${blurCount(editor)} === 2`);
    check(`${editor}: a blur after the consumer re-rendered uses the current callback`, await action(`state.blurs.${editor}[1].revision`), 4);
  }

  const jsonPayload = await action('state.blurs.json[0].args');
  check('json: blur hands over the parsed editor content', jsonPayload[0], { permission: { external_directory: { '*': 'deny' } } });
  const jsoncPayload = await action('state.blurs.jsonc[0].args');
  check('jsonc: blur hands over the raw text and a valid parse', [jsoncPayload[0], jsoncPayload[1]], ['{\n  "permission": true\n}', true]);
  const tomlPayload = await action('state.blurs.toml[0].args');
  check('toml: blur hands over the editor text', tomlPayload[0], '[env]\nKEEP = "old"\n');
  const markdownPayload = await action('state.blurs.markdown[0].args');
  check('markdown: blur hands over the editor text', markdownPayload[0], '# Title\n');

  await screenshot('blur-revisions');

  return checks;
}
