import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

const PLAIN_INPUT = '#plain-input';
const GUARDED_INPUT = '#guarded-input';

/**
 * Chromium cannot reproduce the WebKit composition abort that issue #409
 * reports, so these checks pin the contract we own instead: while an IME
 * composition is in flight the wrapper never forwards the preedit text to
 * rc-select (which is what rewrites the controlled value and kills the
 * composition on macOS), and it still forwards ordinary typing and the once
 * committed text.
 *
 * The plain antd AutoComplete runs beside it to keep the difference visible —
 * if a future antd stops propagating mid-composition text, the fixture fails
 * loudly instead of silently losing the reason this wrapper exists.
 */
export async function verifyImeSafeAutoComplete({ send, evaluate, baseUrl, artifactRoot }) {
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
  const action = expression => evaluate('window.imeSafeAutoCompleteFixture.' + expression);
  const changes = kind => `window.imeSafeAutoCompleteFixture.state.changes.${kind}`;

  const screenshot = async name => {
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    await writeFile(path.join(artifactRoot, name + '.png'), Buffer.from(data, 'base64'));
  };

  const runId = randomUUID();
  await send('Page.navigate', { url: baseUrl + '/?runId=' + runId });
  await waitFor(
    'window.imeSafeAutoCompleteFixture?.state.runId === ' + JSON.stringify(runId)
    + ' && document.querySelectorAll(".ant-select").length === 2',
  );
  await delay(300);

  // ---------------------------------------------------------------- baseline
  // The behaviour the wrapper is meant to remove: antd's AutoComplete hands the
  // pinyin to onChange while the IME is still composing.
  await action(`focus(${JSON.stringify(PLAIN_INPUT)})`);
  await action(`dispatchComposition(${JSON.stringify(PLAIN_INPUT)}, "compositionstart", "")`);
  await action(`setInputValue(${JSON.stringify(PLAIN_INPUT)}, "ceshi")`);
  check('plain AutoComplete forwards the in-composition text (upstream behaviour)', await evaluate(changes('plain')), ['ceshi']);

  // --------------------------------------------------------- guarded wrapper
  await action(`focus(${JSON.stringify(GUARDED_INPUT)})`);
  await action(`dispatchComposition(${JSON.stringify(GUARDED_INPUT)}, "compositionstart", "")`);
  await action(`setInputValue(${JSON.stringify(GUARDED_INPUT)}, "ceshi")`);
  check('guarded wrapper swallows the in-composition text', await evaluate(changes('guarded')), []);
  check('guarded wrapper keeps the in-composition text in the DOM', await action(`inputValue(${JSON.stringify(GUARDED_INPUT)})`), 'ceshi');

  // The browser commits the candidate: the input already holds the hanzi when
  // compositionend arrives.
  await action(`setInputValue(${JSON.stringify(GUARDED_INPUT)}, "测试")`);
  await action(`dispatchComposition(${JSON.stringify(GUARDED_INPUT)}, "compositionend", "测试")`);
  check('guarded wrapper commits the composed text once', await evaluate(changes('guarded')), ['测试']);
  check('guarded wrapper mirrors the committed text into the controlled value', await evaluate('window.imeSafeAutoCompleteFixture.state.values.guarded'), '测试');

  // Ordinary typing must keep working after the composition settled.
  await action(`setInputValue(${JSON.stringify(GUARDED_INPUT)}, "测试abc")`);
  check('guarded wrapper forwards ordinary typing', await evaluate(changes('guarded')), ['测试', '测试abc']);

  await screenshot('ime-safe-autocomplete');

  return checks;
}
