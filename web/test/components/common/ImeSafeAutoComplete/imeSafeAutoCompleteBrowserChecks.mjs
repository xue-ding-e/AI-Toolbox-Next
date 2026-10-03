import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

const PLAIN_INPUT = '#plain-input';
const GUARDED_INPUT = '#guarded-input';
const PLAIN_SMALL_INPUT = '#plain-small-input';
const GUARDED_SMALL_INPUT = '#guarded-small-input';
const PLAIN_FIELD = '#plain-field-input';
const GUARDED_FIELD = '#guarded-field-input';
// A URL the IME composes together with the space a Chinese IME appends when it
// commits English in Chinese mode. The fixture's write-back sanitizer trims it,
// so the value it writes back differs from what is being composed.
const COMPOSING_FIELD_VALUE = 'https://a.example ';
const SANITIZED_FIELD_VALUE = 'https://a.example';

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
    + ' && document.querySelectorAll(".ant-select").length === 4'
    + ' && document.querySelectorAll("form").length === 2',
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

  // The custom child Input is the control now: antd's `-customize` root hands
  // the chrome (border, inline padding) to it instead of drawing it itself. What
  // must not change is the box the user sees, so compare the rendered metrics of
  // both controls — height, font size, and where the text starts.
  const controlMetrics = selector => `(() => {
    const input = document.querySelector(${JSON.stringify(selector)});
    const root = input.closest('.ant-select');
    const inlinePad = element => parseFloat(getComputedStyle(element).paddingInlineStart) || 0;
    return {
      height: Math.round(root.getBoundingClientRect().height),
      fontSize: getComputedStyle(input).fontSize,
      textInset: Math.round(inlinePad(root) + inlinePad(input)),
    };
  })()`;
  check(
    'guarded control renders the same box and text inset as the plain one',
    await evaluate(controlMetrics(GUARDED_INPUT)),
    await evaluate(controlMetrics(PLAIN_INPUT)),
  );
  // antd hides the Select's own placeholder node for a customized input and
  // expects the child to carry it, so it has to still be there.
  check(
    'guarded input keeps the placeholder',
    await evaluate(`document.querySelector(${JSON.stringify(GUARDED_INPUT)}).getAttribute('placeholder')`),
    'fixture placeholder',
  );

  // `size` must reach the custom input too: antd ignores it on AutoComplete once
  // the input is customized, so a dense call site (the Kimi model table) would
  // silently grow back to the default control height.
  const controlHeight = selector => `Math.round(document.querySelector(${JSON.stringify(selector)}).closest('.ant-select').getBoundingClientRect().height)`;
  const smallGuardedHeight = await evaluate(controlHeight(GUARDED_SMALL_INPUT));
  check('guarded control honours size="small"', smallGuardedHeight, await evaluate(controlHeight(PLAIN_SMALL_INPUT)));
  check('size="small" still renders smaller than the default control', smallGuardedHeight < await evaluate(controlHeight(GUARDED_INPUT)), true);

  // ---------------------------------------- form field with a write-back loop
  // The Codex base-url/api-key shape: `onValuesChange` sanitizes the typed text
  // into state and an effect writes it straight back with `setFieldsValue`. Both
  // scenarios below carry identical form wiring, so the only difference is the
  // input component. Chromium cannot abort the composition the way WebKit does,
  // but it does show the rewrite the abort comes from: on the plain field the
  // write-back reaches the DOM while the IME is still composing, on the guarded
  // one it waits for the commit.
  await action(`focus(${JSON.stringify(PLAIN_FIELD)})`);
  await action(`dispatchComposition(${JSON.stringify(PLAIN_FIELD)}, "compositionstart", "")`);
  await action(`setInputValue(${JSON.stringify(PLAIN_FIELD)}, ${JSON.stringify(COMPOSING_FIELD_VALUE)})`);
  await delay(200);
  check(
    'plain form field: the per-keystroke write-back rewrites the composing value',
    await action(`inputValue(${JSON.stringify(PLAIN_FIELD)})`),
    SANITIZED_FIELD_VALUE,
  );
  check(
    'plain form field: the form got the in-composition text',
    await evaluate(changes('plainField')),
    [COMPOSING_FIELD_VALUE],
  );

  await action(`focus(${JSON.stringify(GUARDED_FIELD)})`);
  await action(`dispatchComposition(${JSON.stringify(GUARDED_FIELD)}, "compositionstart", "")`);
  await action(`setInputValue(${JSON.stringify(GUARDED_FIELD)}, ${JSON.stringify(COMPOSING_FIELD_VALUE)})`);
  await delay(200);
  check(
    'guarded form field: the composing value survives the write-back',
    await action(`inputValue(${JSON.stringify(GUARDED_FIELD)})`),
    COMPOSING_FIELD_VALUE,
  );
  check('guarded form field: nothing reached the form mid-composition', await evaluate(changes('guardedField')), []);

  // The IME commits: only now may the field (and with it the write-back) move.
  await action(`setInputValue(${JSON.stringify(GUARDED_FIELD)}, ${JSON.stringify(SANITIZED_FIELD_VALUE)})`);
  await action(`dispatchComposition(${JSON.stringify(GUARDED_FIELD)}, "compositionend", ${JSON.stringify(SANITIZED_FIELD_VALUE)})`);
  await delay(200);
  check(
    'guarded form field: commits once and settles on the sanitized value',
    await action(`inputValue(${JSON.stringify(GUARDED_FIELD)})`),
    SANITIZED_FIELD_VALUE,
  );
  check(
    'guarded form field: the form got the committed text once',
    await evaluate(changes('guardedField')),
    [SANITIZED_FIELD_VALUE],
  );

  await screenshot('ime-safe-autocomplete');

  return checks;
}
