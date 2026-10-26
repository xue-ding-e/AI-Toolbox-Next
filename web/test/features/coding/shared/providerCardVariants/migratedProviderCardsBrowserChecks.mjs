import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

/**
 * Real-browser assertions for the four mapping layers migrated on 2026-10-20
 * (Claude Desktop, Gemini CLI, Grok, OmO Native's card view).
 *
 * Why a browser: every claim here is about what is rendered — which layout a
 * mapping layer picked, which facts a cell states, whether a footer spans the
 * card, whether a protected delete is greyed out. Source guards
 * (`verify-provider-card-layout`) only prove the mapping layer is thin; they
 * cannot see that the footer geometry regressed or that the row lost its sixth
 * action.
 */
const SCENARIO_COUNTS = {
  'render-light': 85,
  'menu-dismiss': 3,
  'menu-delete': 3,
  'menu-protected': 4,
  'detail-cancel': 3,
  'detail-confirm': 2,
  'grok-list-refresh': 5,
  'grok-list-token': 5,
  'grok-card-refresh': 5,
  'grok-card-token': 5,
  'render-dark': 12,
};
const MATRIX = ['claude-desktop', 'gemini', 'grok-official', 'grok-custom', 'omo-default', 'omo-other']
  .flatMap(name => ['list', 'card'].map(view => name + '-' + view));
// Full host inventory is broader than the original twelve theme surfaces:
// readonly/selection/builtin cases and the subscription block are intentional
// additional fixtures, not extra instances of the original two-view matrix.
const FIXTURE_IDS = [
  ...MATRIX,
  'openclaw-list', 'openclaw-card', 'openclaw-default-list',
  'hermes-list', 'hermes-card', 'hermes-readonly-list',
  'hermes-selection-list', 'hermes-selection-disabled-list', 'hermes-readonly-selection-list',
  'pi-list', 'pi-card', 'pi-builtin-list',
  'omp-list', 'omp-card', 'omp-subscription-block',
  'dsh-list', 'dsh-card', 'dsh-builtin-list',
];

export async function verifyMigratedProviderCards({ send, evaluate, baseUrl, artifactRoot, exceptions }) {
  const checks = [];
  const completed = [];
  const lightBackgrounds = {};
  let scenarioStart = 0;
  const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
  const check = (name, actual, expected = true) => {
    assert.deepEqual(actual, expected, name);
    checks.push(name);
    console.log('PASS ' + name);
  };
  const waitFor = async (expression, label) => {
    for (let attempt = 0; attempt < 200; attempt++) {
      if (await evaluate(expression)) return;
      await delay(50);
    }
    throw new Error('Timed out waiting for: ' + (label || expression));
  };
  const fixture = expression => evaluate('migratedProviderCardsFixture.' + expression);
  const screenshot = async name => {
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    await writeFile(path.join(artifactRoot, name + '.png'), Buffer.from(data, 'base64'));
  };

  const finishScenario = async name => {
    assert.equal(checks.length - scenarioStart, SCENARIO_COUNTS[name], 'Incomplete assertions: ' + name);
    assert.deepEqual(await fixture('state.unexpectedIPC'), [], 'Mapping fixture must not call a backend');
    assert.equal(exceptions.length, 0, 'Unhandled browser exceptions remain failures');
    completed.push(name);
    scenarioStart = checks.length;
  };
  const mouse = async (type, x, y) => send('Input.dispatchMouseEvent', {
    type, x, y, ...(type === 'mouseMoved' ? {} : { button: 'left', clickCount: 1 }),
  });
  const visibleMenus = '[...document.querySelectorAll(".ant-dropdown")].filter(node => node.getClientRects().length && getComputedStyle(node).display !== "none" && !node.classList.contains("ant-dropdown-hidden")).length';
  const dismissMenu = async () => {
    await mouse('mouseMoved', 2, 2);
    await mouse('mousePressed', 2, 2);
    await mouse('mouseReleased', 2, 2);
    await waitFor(visibleMenus + ' === 0');
  };
  const stableMenuTarget = async (name, revealed) => {
    let previous;
    let stableSamples = 0;
    for (let attempt = 0; attempt < 200; attempt++) {
      const target = await fixture('menuTarget(' + JSON.stringify(name) + ')');
      // Catalog expansion can displace the button after the initial scroll.
      // Reposition only before hover/click; waiting alone cannot make an
      // offscreen target hit-testable. Keep the original total sample budget.
      if (!revealed && !target.inViewport) {
        await fixture('scrollMenuTarget(' + JSON.stringify(name) + ')');
        previous = undefined;
        stableSamples = 0;
        await delay(50);
        continue;
      }
      const stationary = previous && ['x', 'y', 'width', 'height']
        .every(key => Math.abs(target[key] - previous[key]) < 0.25);
      stableSamples = stationary && target.inViewport && target.hit
        && target.width > 0 && target.height > 0 && (!revealed || target.opacity === 1)
        ? stableSamples + 1 : 0;
      if (stableSamples >= 5) return target;
      previous = target;
      await delay(50);
    }
    throw new Error('Menu trigger never settled or was not hit-testable: '
      + JSON.stringify({ name, revealed, target: previous }));
  };
  const openDeleteMenu = async name => {
    // Expanding the list catalog above the compact cards can still be moving
    // them after its text appears. Never click coordinates cached before that
    // motion (or the compact card's own hover lift) has settled.
    await waitFor(visibleMenus + ' === 0', 'no stale dropdown before opening ' + name);
    assert.equal(await evaluate('Boolean(document.querySelector("[data-provider-detail-panel]"))'), false);
    await fixture('scrollMenuTarget(' + JSON.stringify(name) + ')');
    const resting = await stableMenuTarget(name, false);
    await mouse('mouseMoved', resting.x, resting.y);
    const revealed = await stableMenuTarget(name, true);
    await mouse('mouseMoved', revealed.x, revealed.y);
    // Exactly one real pointer click; no synthetic trigger click or style override.
    await mouse('mousePressed', revealed.x, revealed.y);
    await mouse('mouseReleased', revealed.x, revealed.y);
    try {
      await waitFor(visibleMenus + ' === 1', 'one visible dropdown for ' + name);
      return await fixture('deleteMenuState()');
    } catch (error) {
      console.error('Menu pointer diagnostics:', JSON.stringify({
        name, resting, revealed, current: await fixture('menuTarget(' + JSON.stringify(name) + ')'),
        visibleMenus: await evaluate(visibleMenus),
        detailOpened: await evaluate('Boolean(document.querySelector("[data-provider-detail-panel]"))'),
      }));
      throw error;
    }
  };
  const checkSurfaces = async themeMode => {
    const mounted = await evaluate('[...document.querySelectorAll("[data-fixture]")].map(node => node.dataset.fixture).sort()');
    assert.deepEqual(mounted, [...FIXTURE_IDS].sort(), 'Complete migrated-provider fixture inventory');
    for (const name of MATRIX) {
      const observed = await fixture('surfaceStyle(' + JSON.stringify(name) + ')');
      const rgba = observed.background.match(/^rgba?\(([^)]+)\)$/)?.[1].split(/[,\s]+/).filter(Boolean).map(Number);
      check(themeMode + ': ' + name + ' paints its actual opaque token surface', {
        surface: observed.actualSurface && observed.hasBox,
        token: observed.background === observed.expectedBackground,
        opaque: Boolean(rgba && rgba.length >= 3 && (rgba.length === 3 || rgba[3] === 1)),
        theme: Boolean(rgba && rgba.slice(0, 3).every(value => themeMode === 'dark' ? value < 128 : value > 128)),
        changed: themeMode === 'light' || observed.background !== lightBackgrounds[name],
      }, { surface: true, token: true, opaque: true, theme: true, changed: true });
      if (themeMode === 'light') lightBackgrounds[name] = observed.background;
    }
  };

  const openFixture = async (query) => {
    const runId = randomUUID();
    await send('Page.navigate', { url: `${baseUrl}/?${query}&runId=${encodeURIComponent(runId)}` });
    await waitFor(
      'window.migratedProviderCardsFixture?.state.runId === ' + JSON.stringify(runId)
        + " && Boolean(document.querySelector('[data-fixture=\"grok-custom-list\"]'))",
      `fixture ${query}`,
    );
    await delay(300);
  };

  /* ---------------------------------------------------------------- Claude Desktop */

  await openFixture('theme=light');

  // Role bindings keep their `Role: model` shape, with the value monospaced.
  const desktopText = await fixture('text("claude-desktop-list")');
  check(
    'the Claude Desktop card states each configured role as `Role: model`',
    /Sonnet:\s*claude-sonnet-4-5/.test(desktopText) && /Opus:\s*claude-opus-4-8/.test(desktopText),
  );

  // `tierAlias` is a real tag, not secondary text.
  const tierTag = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="claude-desktop-list"]');
      const tags = [...root.querySelectorAll('.ant-tag')];
      return tags.some(tag => tag.textContent.includes('sonnet'));
    })()
  `);
  check('a role tier alias renders as a tag', tierTag);

  // Notes live in the footer, below the name row and spanning the card's body.
  const notesBelowHeader = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="claude-desktop-list"]');
      const note = [...root.querySelectorAll('*')].find(node =>
        node.children.length === 0 && node.textContent.trim() === '团队共享额度');
      if (!note) return false;
      const name = [...root.querySelectorAll('*')].find(node =>
        node.children.length === 0 && node.textContent.includes('Desktop Relay'));
      return note.getBoundingClientRect().top > name.getBoundingClientRect().bottom;
    })()
  `);
  check('notes sit below the name row, not beside it', notesBelowHeader);

  // A disabled drag leaves no handle at all (the grip used to stay and do nothing).
  check('a drag-disabled card renders no handle', await fixture('hasHandle("claude-desktop-list")'), false);

  // The grid cell states the role bindings on its second line.
  const desktopSubtitle = await fixture('subtitle("claude-desktop-card")');
  check(
    'the Claude Desktop cell states its role bindings',
    Boolean(desktopSubtitle && /Sonnet:\s*claude-sonnet-4-5/.test(desktopSubtitle)),
  );

  /* ---------------------------------------------------------------- Gemini CLI */

  const geminiText = await fixture('text("gemini-list")');
  check(
    'the Gemini official card is titled with the shared heading and its count',
    /官方账号\s*\(2\)/.test(geminiText),
  );
  check(
    'the Gemini official card explains switching on its second line',
    geminiText.includes('替换当前登录'),
  );

  // The account block's rows must reach the card's content edge; the footer
  // used to sit in the header's left column and stopped ~160px short.
  //
  // The list card's fold starts closed (it always has), so open it first.
  check('the Gemini account list can be opened from its title', await fixture('expandAccounts("gemini-list")'));
  await delay(200);
  const rowEdges = await fixture('edges("gemini-list", "[style*=\\"border-bottom\\"]")');
  const cardContentEdges = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="gemini-list"]');
      const card = root.querySelector('.ant-card');
      const body = card.querySelector('.ant-card-body');
      const style = getComputedStyle(body);
      const rect = body.getBoundingClientRect();
      return {
        right: Math.round(rect.right - parseFloat(style.paddingRight)),
        left: Math.round(rect.left + parseFloat(style.paddingLeft)),
      };
    })()
  `);
  check(
    'the account rows span the card content width',
    Boolean(rowEdges) && Math.abs(rowEdges.right - cardContentEdges.right) <= 2,
  );

  // The virtual row allows save and the existing read actions, but cannot
  // switch to itself or delete the live login. Assert the full button set.
  const virtualRowActions = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="gemini-list"]');
      const row = [...root.querySelectorAll('[style*="border-bottom"]')]
        .find(node => node.textContent.includes('local@example.test'));
      return row ? [...row.querySelectorAll('button')].map(button => button.textContent.trim()) : null;
    })()
  `);
  check(
    'the virtual row permits refresh/details/save but no switch or delete',
    virtualRowActions,
    ['刷新', '详情', '保存当前登录'],
  );

  // The applied stored account offers no switch either (it is the current one).
  const appliedRowText = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="gemini-list"]');
      const row = [...root.querySelectorAll('[style*="border-bottom"]')]
        .find(node => node.textContent.includes('work@example.test'));
      return row ? row.textContent.replace(/\\s+/g, ' ').trim() : null;
    })()
  `);
  check(
    'the applied account is marked default instead of offering a switch',
    Boolean(appliedRowText && appliedRowText.includes('默认') && !appliedRowText.includes('切换')),
  );

  // The cell states the login in use.
  const geminiSubtitle = await fixture('subtitle("gemini-card")');
  check(
    'the Gemini official cell states the current login',
    Boolean(geminiSubtitle && geminiSubtitle.includes('当前登录') && geminiSubtitle.includes('work@example.test')),
  );

  /* ---------------------------------------------------------------- Grok */

  const grokOfficialText = await fixture('text("grok-official-list")');
  check(
    'the Grok official card is titled with the shared heading',
    /官方账号\s*\(2\)/.test(grokOfficialText),
  );
  // The fold starts closed on the list card; the sixth action lives on a row.
  check('the Grok account list can be opened from its title', await fixture('expandAccounts("grok-official-list")'));
  await delay(200);
  const grokExpandedText = await fixture('text("grok-official-list")');
  check(
    'Grok keeps its sixth account action through the shared extra-action slot',
    grokExpandedText.includes('刷新 Token'),
  );

  // The refresh-token action is absent from the custom provider (no accounts).
  check(
    'a custom Grok provider renders no account block',
    !(await fixture('text("grok-custom-list")')).includes('刷新 Token'),
  );

  // The model toolbar keeps the documented order:
  // 批量删除 → 模型测试 → 获取模型 → 添加模型.
  //
  // Searched from the catalog's own title, not from the top of the card: the
  // header's inline connectivity action carries the same words (模型测试), and
  // an unscoped `indexOf` would score that one as the first toolbar item.
  const toolbarOrder = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="grok-custom-list"]');
      const text = root.textContent.replace(/\s+/g, ' ');
      const from = text.indexOf('模型列表');
      if (from < 0) return [-1, -1, -1, -1];
      return ['批量删除', '模型测试', '获取模型', '添加模型']
        .map(label => text.indexOf(label, from));
    })()
  `);
  check(
    'the Grok model toolbar keeps its documented order',
    toolbarOrder.every((index, position) => index >= 0 && (position === 0 || index > toolbarOrder[position - 1])),
  );

  // The custom card's second line states the endpoint, model and masked key.
  //
  // No protocol tag here: `apiFormatLabel` comes from `openAiApiFormatFromBaseUrl`
  // / the config's own `api` key, and this fixture's bare endpoint resolves to
  // none. The tag's presence is covered by the list card in the app; what this
  // check guards is that the free-form entries survived the migration.
  const grokCustomText = await fixture('text("grok-custom-list")');
  check(
    'the custom Grok card states endpoint, model and masked key',
    grokCustomText.includes('https://grok.example.test/v1')
      && grokCustomText.includes('grok-4 (high)')
      && grokCustomText.includes('API Key: sk-grok-...6789')
      && !grokCustomText.includes('sk-grok-fixture-key-0123456789'),
  );

  /* ---------------------------------------------------------------- OmO Native */

  const omoDefaultSubtitle = await fixture('subtitle("omo-default-card")');
  check(
    'the OmO cell states the npm package',
    Boolean(omoDefaultSubtitle && omoDefaultSubtitle.startsWith('@ai-sdk/')),
  );
  check(
    'the provider that owns the default states it',
    Boolean(omoDefaultSubtitle && omoDefaultSubtitle.includes('当前主模型')),
  );

  const omoOtherSubtitle = await fixture('subtitle("omo-other-card")');
  check(
    'a non-default OmO cell states only the package',
    Boolean(omoOtherSubtitle && omoOtherSubtitle.startsWith('@ai-sdk/') && !omoOtherSubtitle.includes('当前主模型')),
  );

  const defaultDelete = await openDeleteMenu('omo-default-card');
  check(
    'the default provider’s menu greys out delete and explains why',
    Boolean(defaultDelete && defaultDelete.found && defaultDelete.disabled),
  );

  await dismissMenu();
  const otherDelete = await openDeleteMenu('omo-other-card');
  check(
    'an ordinary provider’s menu keeps delete live',
    Boolean(otherDelete && otherDelete.found && !otherDelete.disabled),
  );

  await dismissMenu();

  // Complete the formerly one-sided Grok/OmO mapping matrix.
  check('Grok official compact card states the current login',
    (await fixture('subtitle("grok-official-card")')).includes('grok@example.test'));
  check('Grok custom compact card states its primary model',
    (await fixture('subtitle("grok-custom-card")')).includes('grok-4 (high)'));
  await fixture('expandModels("omo-default-list")');
  await fixture('expandModels("omo-other-list")');
  await waitFor('migratedProviderCardsFixture.text("omo-default-list").includes("Default Model")'
    + ' && migratedProviderCardsFixture.text("omo-other-list").includes("Other Model")');
  check('OmO default list keeps provider identity and its model catalog',
    /omo-default/.test(await fixture('text("omo-default-list")'))
      && /Default Model/.test(await fixture('text("omo-default-list")')));
  check('OmO ordinary list keeps provider identity and its model catalog',
    /omo-other/.test(await fixture('text("omo-other-list")'))
      && /Other Model/.test(await fixture('text("omo-other-list")')));
  await checkSurfaces('light');
  /* ---------------------------------------------------------------- OpenClaw */

  const openClawText = await fixture('text("openclaw-list")');
  check(
    'the OpenClaw card states its id, npm package and endpoint',
    openClawText.includes('relay-main')
      && openClawText.includes('@ai-sdk/openai-compatible')
      && openClawText.includes('https://relay.openclaw.example.test/v1'),
  );
  check(
    'the OpenClaw card offers no provider-level 应用',
    !openClawText.includes('应用'),
  );

  // The catalog toolbar is the only place the probes live now: the bespoke card
  // had them in a header `extraActions` slot the shared styles do not have.
  const openClawToolbar = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="openclaw-list"]');
      const buttons = [...root.querySelectorAll('button')].map(node => node.textContent.replace(/\\s+/g, ' ').trim());
      return {
        batch: buttons.some(text => text.includes('批量删除')),
        test: buttons.some(text => text.includes('模型测试')),
        fetch: buttons.some(text => text.includes('获取模型')),
        add: buttons.some(text => text.includes('添加模型')),
      };
    })()
  `);
  check(
    'the OpenClaw model toolbar carries batch delete, test, fetch and add',
    openClawToolbar.batch && openClawToolbar.test && openClawToolbar.fetch && openClawToolbar.add,
  );

  check('the OpenClaw cell states the npm package', await fixture('subtitle("openclaw-card")') === '@ai-sdk/openai-compatible');

  // The OpenCode-style list card has no "more" menu — delete is an icon button
  // in the header, which the default provider's guard must grey out.
  const openClawDefaultDelete = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="openclaw-default-list"]');
      const deleteButton = [...root.querySelectorAll('button')]
        .find(node => node.querySelector('.anticon-delete'));
      if (!deleteButton) return { found: false };
      // A disabled antd button drops pointer events, so the tooltip has to hang
      // off a wrapper element — the button's direct parent must be that span.
      return {
        found: true,
        disabled: deleteButton.disabled === true,
        hasTooltipHost: deleteButton.parentElement?.tagName === 'SPAN',
      };
    })()
  `);
  check(
    'the default OpenClaw provider greys out delete',
    Boolean(openClawDefaultDelete.found && openClawDefaultDelete.disabled),
  );
  check(
    'the disabled OpenClaw delete keeps a hover target for its reason',
    Boolean(openClawDefaultDelete.hasTooltipHost),
  );

  /* ---------------------------------------------------------------- Hermes */

  const hermesText = await fixture('text("hermes-list")');
  check(
    'the Hermes card states its endpoint and API mode',
    hermesText.includes('https://api.anthropic.com') && hermesText.includes('anthropic'),
  );
  check(
    'the Hermes card offers no provider-level 应用',
    !hermesText.includes('应用'),
  );
  // No new "default" badge on the card: the fact was already stated on the
  // catalog row (`当前主模型`), and a migration does not add state furniture.
  // Scoped to the header line, which is where a badge would have gone.
  const hermesNameLine = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="hermes-list"]');
      const name = [...root.querySelectorAll('*')]
        .find(node => node.children.length === 0 && node.textContent.includes('Anthropic Direct'));
      return name?.parentElement?.textContent.replace(/\\s+/g, ' ').trim() ?? '';
    })()
  `);
  check(
    'the Hermes card does not re-state the default as a name-line badge',
    !hermesNameLine.includes('当前主模型'),
  );

  // The catalog rows live behind the card's fold; read them after opening it.
  check('the Hermes card opens its model catalog', await fixture('expandModels("hermes-list")'));
  const hermesRows = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="hermes-list"]');
      const rows = [...root.querySelectorAll('[data-model-row]')].map(node => ({
        text: node.textContent.replace(/\\s+/g, ' ').trim(),
        grip: Boolean(node.querySelector('[class*="grip"]')),
        setPrimary: [...node.querySelectorAll('button')]
          .some(button => button.textContent.includes('设为主模型')),
      }));
      return rows;
    })()
  `);
  check(
    'the Hermes model row that owns the default says so',
    Boolean(hermesRows.find(row => row.text.includes('claude-opus-4-8'))?.text.includes('当前主模型')),
  );
  check(
    'another Hermes model row offers 设为主模型',
    Boolean(hermesRows.find(row => row.text.includes('claude-sonnet-4-5'))?.setPrimary),
  );
  check(
    'Hermes model rows carry no drag grip (the CLI has no model reorder)',
    hermesRows.every(row => !row.grip),
  );
  // The provider card itself does drag: `custom_providers` is a YAML sequence,
  // and `reorder_hermes_providers` rewrites it. A model grip would drag nothing;
  // the card grip is the one that persists.
  check(
    'the Hermes provider card carries a drag grip',
    await evaluate(`Boolean(document.querySelector('[data-fixture="hermes-list"] [data-provider-card-handle]'))`),
  );

  const hermesReadOnly = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="hermes-readonly-list"]');
      const text = root.textContent.replace(/\\s+/g, ' ').trim();
      // The catalog toolbar always carries 批量删除 / 添加模型; what the read-only
      // state must remove is the *provider* actions (header icon buttons).
      const headerIcons = [...root.querySelectorAll('button')]
        .map(node => node.querySelector('.anticon')?.className ?? '')
        .filter(name => name);
      return {
        tagged: text.includes('只读'),
        edit: headerIcons.some(name => name.includes('anticon-edit')),
        copy: headerIcons.some(name => name.includes('anticon-copy')),
        del: headerIcons.some(name => name.includes('anticon-delete')),
      };
    })()
  `);
  check(
    'a read-only Hermes provider is tagged 只读',
    hermesReadOnly.tagged,
  );
  check(
    'a read-only Hermes provider offers no edit, copy or delete',
    !hermesReadOnly.edit && !hermesReadOnly.copy && !hermesReadOnly.del,
  );
  const hermesReadOnlyText = await fixture('text("hermes-readonly-list")');
  check(
    'a read-only Hermes provider does not pretend its key is its endpoint',
    !hermesReadOnlyText.includes('(只读)'),
  );
  // `reorder_hermes_providers` rewrites the custom list only, so a grip on a
  // `providers:` dict row would move a card and persist nothing.
  check(
    'a read-only Hermes provider carries no drag grip',
    await evaluate(`!document.querySelector('[data-fixture="hermes-readonly-list"] [data-provider-card-handle]')`),
  );

  /* ---------------------------------------------- selection mode (checkbox) */

  /** Reads the left-slot control of a fixture block. */
  const readSelectionSlot = name => evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="${name}"]');
      const input = root?.querySelector('input[type="checkbox"]');
      return {
        hasCheckbox: Boolean(input),
        disabled: input ? input.disabled : null,
        hasHandle: Boolean(root?.querySelector('[data-provider-card-handle]')),
      };
    })()
  `);

  const eligibleSlot = await readSelectionSlot("hermes-selection-list");
  check('an eligible row in selection mode shows a live checkbox',
    eligibleSlot.hasCheckbox && eligibleSlot.disabled === false);
  check('the checkbox replaces the drag handle in selection mode', !eligibleSlot.hasHandle);

  const blockedSlot = await readSelectionSlot("hermes-selection-disabled-list");
  check('an ineligible row still shows a checkbox instead of a blank slot',
    blockedSlot.hasCheckbox);
  check('the ineligible row’s checkbox is disabled', blockedSlot.disabled === true);
  check('the ineligible row shows no drag handle either', !blockedSlot.hasHandle);

  // Hover the disabled checkbox: its reason must be reachable, which a bare
  // disabled <input> would swallow.
  // These fixture blocks stack past the fold, and a hover on an off-screen
  // element never reaches the tooltip: scroll it in first, then measure.
  await evaluate(`document.querySelector('[data-fixture="hermes-selection-disabled-list"]')
    ?.scrollIntoView({ block: 'center' })`);
  await delay(300);
  const blockedCheckboxCenter = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="hermes-selection-disabled-list"]');
      const box = root?.querySelector('input[type="checkbox"]')?.parentElement;
      if (!box) return null;
      const rect = box.getBoundingClientRect();
      if (rect.width === 0 || rect.top < 0 || rect.bottom > innerHeight) return null;
      return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
    })()
  `);
  assert.ok(blockedCheckboxCenter, 'the disabled checkbox must be on screen');
  await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...blockedCheckboxCenter });
  await delay(600);
  check(
    'hovering the disabled checkbox explains why the row cannot be picked',
    await evaluate(`document.querySelector('.ant-tooltip')?.textContent?.trim() ?? null`),
    '该渠道已设为默认，不可多选',
  );

  const readOnlySlot = await readSelectionSlot("hermes-readonly-selection-list");
  check('a read-only row in selection mode is disabled too, not blank',
    readOnlySlot.hasCheckbox && readOnlySlot.disabled === true);

  const hermesCardSubtitle = await fixture('subtitle("hermes-card")');
  check(
    'the Hermes cell states the SDK family and the default model',
    Boolean(hermesCardSubtitle
      && hermesCardSubtitle.startsWith('@ai-sdk/anthropic')
      && hermesCardSubtitle.includes('当前主模型')),
  );

  /* ------------------------------------------------------- Pi / Oh My Pi / dsh */

  // Pi's old card fell back through the credential mask and the source labels
  // before printing a "Base URL". The shared card has a real endpoint line, and
  // the mask lives in the detail sidebar only.
  await fixture('expandModels("pi-list")');
  const piList = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="pi-list"]');
      const text = root.textContent.replace(/\\s+/g, ' ').trim();
      return {
        text,
        grip: Boolean(root.querySelector('[data-provider-card-handle]')),
      };
    })()
  `);
  check(
    'the Pi card states its endpoint and protocol',
    piList.text.includes('https://openrouter.ai/api/v1') && piList.text.includes('@ai-sdk/openai-compatible'),
  );
  check('the Pi card offers no provider-level apply', !piList.text.includes('应用'));
  check('the Pi credential is not printed as an endpoint', !piList.text.includes('sk-or-v1-fixture-key-1234'));
  check('the Pi provider card carries a drag grip', piList.grip);
  const piRows = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="pi-list"]');
      return [...root.querySelectorAll('[data-model-row]')]
        .map(row => row.textContent.replace(/\\s+/g, ' ').trim());
    })()
  `);
  check(
    'the Pi default model row is marked as the current one',
    piRows.some(text => text.includes('claude-opus-4-8') && text.includes('当前主模型')),
  );
  // The row reads `contextWindow` / `maxTokens` (Pi's stored names); reading
  // the form field names instead silently dropped this suffix from every row.
  check(
    'the Pi model row states its context window',
    piRows.some(text => text.includes('claude-opus-4-8') && text.includes('200K')),
  );

  // The old card printed the credential mask / source names *in place of* the
  // endpoint when there was none. The shared card names the sources as tags,
  // but must not turn any of them into a link.
  const piBuiltin = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="pi-builtin-list"]');
      return {
        tags: [...root.querySelectorAll('.ant-tag')].map(node => node.textContent.trim()),
        links: [...root.querySelectorAll('a[href^="http"]')].map(node => node.getAttribute('href')),
      };
    })()
  `);
  check(
    'a built-in Pi key names its sources as tags',
    piBuiltin.tags.includes('内置') && piBuiltin.tags.includes('settings.json'),
  );
  check(
    'a built-in Pi key renders without a fabricated endpoint',
    piBuiltin.links.length === 0,
  );

  await fixture('expandModels("omp-list")');
  const ompList = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="omp-list"]');
      return {
        text: root.textContent.replace(/\\s+/g, ' ').trim(),
        grip: Boolean(root.querySelector('[data-provider-card-handle]')),
      };
    })()
  `);
  check(
    'the Oh My Pi card states its endpoint and protocol',
    ompList.text.includes('https://openrouter.ai/api/v1') && ompList.text.includes('@ai-sdk/openai-compatible'),
  );
  check('the Oh My Pi card carries a drag grip', ompList.grip);

  // The subscription block is a list member of its own: it must reach the real
  // page, not a stub, for this check to mean anything.
  const ompSubscriptionText = await fixture('text("omp-subscription-block")');
  check(
    'the Oh My Pi subscription block states its OAuth status',
    ompSubscriptionText.includes('凭据已存储'),
  );
  // The list card folds its account rows away (Codex's call, user-requested for
  // OMP on 2026-10-22). `OfficialAccountsSection` draws a caret only when the
  // host passes `onToggleCollapsed`, so the presence of that button *is* the
  // collapse affordance — the rows alone would render either way.
  const ompAccountTitleButton = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="omp-subscription-block"]');
      const button = [...root.querySelectorAll('button')]
        .find(candidate => candidate.textContent.includes('账号列表'));
      return Boolean(button);
    })()
  `);
  check('the Oh My Pi account list is collapsible', ompAccountTitleButton);
  check(
    'clicking the Oh My Pi account list title expands it',
    await fixture('expandAccounts("omp-subscription-block")'),
  );

  const dshList = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="dsh-list"]');
      return {
        text: root.textContent.replace(/\\s+/g, ' ').trim(),
        grip: Boolean(root.querySelector('[data-provider-card-handle]')),
      };
    })()
  `);
  check(
    'the dsh card states its endpoint and SDK family',
    dshList.text.includes('https://api.deepseek.com/v1')
      && dshList.text.includes('@ai-sdk/openai-compatible'),
  );
  check('the dsh card carries a drag grip', dshList.grip);
  check(
    'the dsh card names the credential ref the key is stored under',
    dshList.text.includes('DEEPSEEK_CUSTOM_API_KEY'),
  );

  const dshBuiltin = await evaluate(`
    (() => {
      const root = document.querySelector('[data-fixture="dsh-builtin-list"]');
      const text = root.textContent.replace(/\\s+/g, ' ').trim();
      const headerIcons = [...root.querySelectorAll('button')]
        .map(node => node.querySelector('.anticon')?.className ?? '')
        .filter(name => name);
      // The model toolbar's "批量删除" also carries a delete icon, so a plain
      // icon search would report a provider-level delete that is not there.
      // The provider-level row is icon-only; the model toolbar's buttons all
      // carry a label.
      const iconOnlyDelete = [...root.querySelectorAll('button')]
        .some(node => node.querySelector('.anticon-delete') && !node.textContent.trim());
      return {
        text,
        grip: Boolean(root.querySelector('[data-provider-card-handle]')),
        edit: headerIcons.some(name => name.includes('anticon-edit')),
        del: iconOnlyDelete,
      };
    })()
  `);
  check(
    'a catalog-backed dsh route is tagged as the bundled catalog',
    dshBuiltin.text.includes('内置 · 适配器默认模型'),
  );
  check(
    'a catalog-backed dsh route keeps editing but offers no delete and no grip',
    dshBuiltin.edit && !dshBuiltin.del && !dshBuiltin.grip,
  );
  check(
    'a catalog-backed dsh route still lets the user declare its own models',
    dshBuiltin.text.includes('添加模型'),
  );

  // The grid cell only carries "more"; edit / copy / share / delete live in the
  // detail sidebar. A card that forgets `detail.secondaryActions` silently loses
  // every one of them in card mode while the list layout still shows them.
  for (const [block, label] of [
    ['pi-card', 'Pi'],
    ['omp-card', 'Oh My Pi'],
    ['dsh-card', 'dsh'],
  ]) {
    check(
      `the ${label} detail sidebar keeps the quieter actions`,
      await fixture(`detailFooterActions("${block}")`),
      ['编辑', '复制', '分享', '删除'],
    );
  }

  await finishScenario('render-light');
  await screenshot('migrated-provider-cards-light');

  /* ---------------------------------------------------------------- menu behavior */

  await fixture('resetCallbacks()');
  check('ordinary menu opens with a live delete entry', (await openDeleteMenu('omo-other-card')).disabled, false);
  await dismissMenu();
  check('dismissing the menu actually closes it', await evaluate(visibleMenus), 0);
  check('dismissing a menu invokes no delete callback', await fixture('state.callbacks'), []);
  await finishScenario('menu-dismiss');

  check('ordinary delete stays actionable on reopening', (await openDeleteMenu('omo-other-card')).disabled, false);
  await fixture('clickMenuDelete()');
  await waitFor('migratedProviderCardsFixture.state.callbacks.length === 1');
  const deletedOther = [{ action: 'delete', fixture: 'omo-other-card', providerId: 'omo-other' }];
  check('menu delete dispatches the exact ordinary provider once', await fixture('state.callbacks'), deletedOther);
  check('menu dispatch does not invent a detail confirmation dialog', await evaluate('Boolean(document.querySelector(".ant-modal-confirm"))'), false);
  await finishScenario('menu-delete');
  await dismissMenu();

  await fixture('resetCallbacks()');
  const protectedDelete = await openDeleteMenu('omo-default-card');
  check('protected delete is disabled', protectedDelete.disabled);
  check('protected delete exposes the precise reason', protectedDelete.title, '默认渠道不能删除');
  await fixture('clickMenuDelete()');
  await delay(100);
  check('clicking protected delete never dispatches a callback', await fixture('state.callbacks'), []);
  check('protected delete opens no confirmation', await evaluate('Boolean(document.querySelector(".ant-modal-confirm"))'), false);
  await finishScenario('menu-protected');
  await dismissMenu();

  /* ---------------------------------------------------------------- real detail confirmation */

  await fixture('openDetail("omo-other-card")');
  await waitFor('Boolean(document.querySelector("[data-provider-detail-panel]"))');
  await fixture('clickDetailDelete()');
  await waitFor('Boolean(document.querySelector(".ant-modal-confirm"))');
  check('detail delete opens the real provider confirmation', await evaluate(
    'document.querySelector(".ant-modal-confirm").textContent.includes("OmO Other")'));
  check('opening confirmation does not delete', await fixture('state.callbacks'), []);
  await fixture('answerConfirm(false)');
  await waitFor('!document.querySelector(".ant-modal-confirm")');
  check('cancelling the real confirmation invokes no delete', await fixture('state.callbacks'), []);
  await finishScenario('detail-cancel');

  await fixture('clickDetailDelete()');
  await waitFor('Boolean(document.querySelector(".ant-modal-confirm"))');
  check('reopened confirmation still waits for consent', await fixture('state.callbacks'), []);
  await fixture('answerConfirm(true)');
  await waitFor('!document.querySelector(".ant-modal-confirm")');
  check('confirmed detail delete invokes the exact callback once', await fixture('state.callbacks'), deletedOther);
  await finishScenario('detail-confirm');
  await fixture('closeDetail()');
  await waitFor('!document.querySelector("[data-provider-detail-panel]")');

  /* ---------------------------------------------------------------- real Grok callback/loading mapping */

  const idle = [
    { accountId: 'grok-account-oauth', refresh: false, token: false },
    { accountId: 'grok-account-other', refresh: false, token: false },
  ];
  for (const viewMode of ['list', 'card']) {
    if (viewMode === 'card') {
      await fixture('openDetail("grok-official-card")');
      await waitFor('Boolean(document.querySelector("[data-provider-detail-panel]"))');
    }
    for (const action of ['refresh', 'token']) {
      const scenario = 'grok-' + viewMode + '-' + action;
      const accountId = action === 'refresh' ? 'grok-account-oauth' : 'grok-account-other';
      const invocation = [{ action, viewMode, providerId: 'grok-official', accountId }];
      const click = 'clickRefresh(' + [viewMode, accountId, action].map(value => JSON.stringify(value)).join(',') + ')';
      const loading = 'loadingState(' + JSON.stringify(viewMode) + ')';
      await fixture('resetCallbacks()');
      check(scenario + ': both account rows start idle', await fixture(loading), idle);
      await fixture(click);
      await waitFor('migratedProviderCardsFixture.' + loading + '.some(row => row.' + action + ')');
      check(scenario + ': callback preserves provider and account identity', await fixture('state.callbacks'), invocation);
      check(scenario + ': only the correct account and action show loading', await fixture(loading),
        idle.map(row => row.accountId === accountId ? { ...row, [action]: true } : row));
      await fixture(click);
      await fixture(click);
      check(scenario + ': loading clicks do not duplicate the callback', await fixture('state.callbacks'), invocation);
      await fixture('finishRefresh()');
      await waitFor('migratedProviderCardsFixture.' + loading + '.every(row => !row.refresh && !row.token)');
      check(scenario + ': clearing the supplied loading prop restores both rows', await fixture(loading), idle);
      await finishScenario(scenario);
    }
    if (viewMode === 'card') {
      await fixture('closeDetail()');
      await waitFor('!document.querySelector("[data-provider-detail-panel]")');
    }
  }

  /* ---------------------------------------------------------------- dark actual surfaces */

  await openFixture('theme=dark');
  await checkSurfaces('dark');
  await finishScenario('render-dark');
  await screenshot('migrated-provider-cards-dark');

  assert.deepEqual(completed, Object.keys(SCENARIO_COUNTS), 'Every migrated-card scenario must execute');
  assert.equal(completed.length, 11, 'Fixed scenario count prevents zero-test success');
  assert.equal(checks.length, 132, 'Fixed assertion count prevents partial success');
  await writeFile(path.join(artifactRoot, 'scenarios.json'), JSON.stringify({
    completed, assertionCounts: SCENARIO_COUNTS, checks, browserExceptions: exceptions,
  }, null, 2));
  console.log(`Passed ${completed.length} migrated-provider-card scenarios (${checks.length} assertions).`);
  const routingChecks = await verifyClaudeDesktopRouting({ send, evaluate, baseUrl, artifactRoot, exceptions });
  assert.equal(checks.length + routingChecks.length, 160, 'Complete migration and Desktop routing coverage');
  return [...checks, ...routingChecks];
}

const DESKTOP_ROUTING_COUNTS = {
  'desktop-identity-list': 4,
  'desktop-identity-card': 4,
  'desktop-takeover-list': 3,
  'desktop-takeover-card': 3,
  'desktop-mapped-list': 3,
  'desktop-mapped-card': 3,
  'desktop-stopped-list': 4,
  'desktop-stopped-card': 4,
};

/**
 * Independently runnable against the same real fixture for strict RED/GREEN.
 * Every case records its phase and observations, even after another fails.
 * Only safe-remap exposing a live, hit-testable direct action instead of proxy
 * is the expected defect; setup/IPC/browser/pointer failures are never RED.
 */
export async function verifyClaudeDesktopRouting({ send, evaluate, baseUrl, artifactRoot, exceptions }) {
  const checks = [];
  const cases = [];
  const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
  const fixture = expression => evaluate('migratedProviderCardsFixture.' + expression);
  const check = (name, actual, expected = true) => {
    assert.deepEqual(actual, expected, name);
    checks.push(name);
    console.log('PASS ' + name);
  };
  const waitFor = async (expression, label) => {
    for (let attempt = 0; attempt < 200; attempt++) {
      if (await evaluate(expression)) return;
      await delay(50);
    }
    throw new Error('Timed out waiting for: ' + label);
  };
  const mouse = async (type, x, y) => send('Input.dispatchMouseEvent', {
    type, x, y, ...(type === 'mouseMoved' ? {} : { button: 'left', clickCount: 1 }),
  });
  const assertClean = async observations => {
    observations.unexpectedIPC = await fixture('state.unexpectedIPC');
    observations.browserExceptions = [...exceptions];
    assert.deepEqual(observations.unexpectedIPC, [], 'Mapping fixture must not call a backend');
    assert.deepEqual(observations.browserExceptions, [], 'Browser exceptions remain failures');
  };
  const stableTarget = async (action, observations) => {
    const argument = JSON.stringify(action);
    let previous;
    let stableSamples = 0;
    let hovered = false;
    for (let attempt = 0; attempt < 200; attempt++) {
      const target = await fixture('desktopGatewayTarget(' + argument + ')');
      observations.pointer = { action, ...target };
      const stationary = previous && ['x', 'y', 'width', 'height']
        .every(key => Math.abs(target[key] - previous[key]) < 0.25);
      stableSamples = stationary && target.visible && target.inViewport && target.hit
        ? stableSamples + 1 : 0;
      if (stableSamples >= 5) {
        if (hovered) return target;
        await mouse('mouseMoved', target.x, target.y);
        hovered = true;
        stableSamples = 0;
      }
      previous = target;
      await delay(50);
    }
    throw new Error('Desktop action never settled or was not hit-testable: ' + JSON.stringify(observations.pointer));
  };
  const captureDiagnostics = async (report, failure) => {
    const basename = failure ? 'case-' + report.name + '-failure' : 'desktop-routing-' + report.name;
    try {
      const { data } = await send('Page.captureScreenshot', { format: 'png' });
      await writeFile(path.join(artifactRoot, basename + '.png'), Buffer.from(data, 'base64'));
      report.screenshot = basename + '.png';
    } catch (error) {
      report.diagnosticError = { screenshot: error instanceof Error ? error.message : String(error) };
    }
    if (failure) {
      try {
        await writeFile(path.join(artifactRoot, basename + '.html'), await evaluate('document.documentElement.outerHTML'));
        report.html = basename + '.html';
      } catch (error) {
        report.diagnosticError = { ...report.diagnosticError, html: error instanceof Error ? error.message : String(error) };
      }
    }
  };
  const legacyRemapActions = { direct: [{ visible: true, disabled: false }], proxy: [], restore: [] };
  for (const scenarioCase of ['identity', 'takeover', 'mapped', 'stopped']) {
    for (const viewMode of ['list', 'card']) {
      const name = 'desktop-' + scenarioCase + '-' + viewMode;
      const report = { name, role: scenarioCase === 'mapped' ? 'regression' : 'control', phase: 'navigation', observations: {} };
      const observations = report.observations;
      const assertionStart = checks.length;
      let expectedDefect = false;
      let fixtureMounted = false;
      try {
        const runId = randomUUID();
        await send('Page.navigate', { url: `${baseUrl}/?theme=light&desktopGateway=${scenarioCase}&view=${viewMode}&runId=${encodeURIComponent(runId)}` });
        await waitFor('window.migratedProviderCardsFixture?.state.runId === ' + JSON.stringify(runId)
          + ' && Boolean(document.querySelector("[data-fixture=desktop-gateway]"))', name + ': fresh fixture');
        fixtureMounted = true;
        report.phase = 'mount';
        const providerId = 'desktop-gateway-' + scenarioCase + '-' + viewMode;
        const surface = viewMode === 'list' ? '.ant-card' : '[data-provider-card-clickable]';
        observations.surfaces = await evaluate('document.querySelectorAll(' + JSON.stringify(
          '[data-fixture="desktop-gateway"] [data-provider-id="' + providerId + '"] ' + surface,
        ) + ').length');
        assert.equal(observations.surfaces, 1, name + ': actual shared view must mount');
        await delay(300);
        await assertClean(observations);
        observations.actions = await fixture('desktopGatewayActions()');
        observations.callbacks = await fixture('state.callbacks');
        const action = scenarioCase === 'identity' ? 'direct' : scenarioCase === 'takeover' ? 'restore' : 'proxy';
        const expected = { direct: [], proxy: [], restore: [] };
        expected[action] = [{ visible: true, disabled: scenarioCase === 'stopped' }];
        observations.expectedActions = expected;

        // Establish that the erroneous direct button is really reachable before
        // identifying this particular mismatch as a baseline regression.
        if (scenarioCase === 'mapped' && JSON.stringify(observations.actions) === JSON.stringify(legacyRemapActions)) {
          report.phase = 'baseline-direct-pointer';
          await stableTarget('direct', observations);
          await assertClean(observations);
          observations.actions = await fixture('desktopGatewayActions()');
          observations.callbacks = await fixture('state.callbacks');
          assert.deepEqual(observations.callbacks, [], 'Inspection must not dispatch callbacks');
          expectedDefect = JSON.stringify(observations.actions) === JSON.stringify(legacyRemapActions);
        }
        report.phase = 'action-availability';
        check(name + ': only the correct real action is visible with the correct enabled state', observations.actions, expected);
        report.phase = 'pointer';
        const target = await stableTarget(action, observations);
        observations.actions = await fixture('desktopGatewayActions()');
        assert.deepEqual(observations.actions, expected, 'Action state must remain correct after hover');
        check(name + ': real action is visible and hit-testable after hover settles', target.visible && target.inViewport && target.hit);

        // The real mapping layer owns gateway service calls. Availability only
        // for enabled proxy/restore; no IPC allowlist or substitute component.
        // Stopped uses a legacy non-Claude model, not safe-remap while stopped.
        report.phase = 'callback';
        if (scenarioCase === 'identity' || scenarioCase === 'stopped') {
          await mouse('mouseMoved', target.x, target.y);
          await mouse('mousePressed', target.x, target.y);
          await mouse('mouseReleased', target.x, target.y);
          if (scenarioCase === 'identity') {
            await waitFor('migratedProviderCardsFixture.state.callbacks.length === 1', name + ': direct callback');
          } else {
            await delay(100);
          }
          observations.callbacks = await fixture('state.callbacks');
          check(name + ': pointer click dispatches only the permitted direct callback', observations.callbacks,
            scenarioCase === 'identity' ? [{ action: 'direct', viewMode, providerId }] : []);
          observations.detailOpened = await evaluate('Boolean(document.querySelector("[data-provider-detail-panel]"))');
          check(name + ': action click does not open the compact detail panel', observations.detailOpened, false);
        } else {
          observations.callbacks = await fixture('state.callbacks');
          check(name + ': inspecting gateway availability invokes no callback', observations.callbacks, []);
        }
        report.phase = 'final-guards';
        await assertClean(observations);
        assert.equal(checks.length - assertionStart, DESKTOP_ROUTING_COUNTS[name], 'Incomplete assertions: ' + name);
        report.phase = 'complete';
        report.status = 'passed';
        if (scenarioCase === 'mapped' || scenarioCase === 'takeover') await captureDiagnostics(report, false);
      } catch (error) {
        report.status = 'failed';
        report.failureKind = expectedDefect && report.phase === 'action-availability'
          ? 'missing-proxy-for-safe-remap' : 'unexpected-failure';
        report.error = error instanceof Error ? error.message : String(error);
        if (fixtureMounted) {
          try {
            await assertClean(observations);
          } catch (guardError) {
            report.failureKind = 'unexpected-failure';
            report.guardError = guardError instanceof Error ? guardError.message : String(guardError);
          }
        }
        await captureDiagnostics(report, true);
        console.error('FAIL ' + name + ' [' + report.phase + '/' + report.failureKind + ']: ' + report.error);
      }
      report.passedAssertions = checks.length - assertionStart;
      cases.push(report);
    }
  }
  const failures = cases.filter(item => item.status === 'failed');
  const summary = {
    attempted: cases.length,
    passed: cases.length - failures.length,
    failed: failures.length,
    passedControls: cases.filter(item => item.role === 'control' && item.status === 'passed').length,
    failedControls: failures.filter(item => item.role === 'control').length,
    expectedDefectFailures: failures.filter(item => item.failureKind === 'missing-proxy-for-safe-remap').length,
    unexpectedFailures: failures.filter(item => item.failureKind !== 'missing-proxy-for-safe-remap').length,
  };
  const validRed = summary.attempted === 8 && summary.passedControls === 6
    && summary.failedControls === 0 && summary.unexpectedFailures === 0
    && checks.length === 22 && exceptions.length === 0
    && JSON.stringify(failures.map(item => item.name)) === JSON.stringify(['desktop-mapped-list', 'desktop-mapped-card']);
  await writeFile(path.join(artifactRoot, 'desktop-routing-cases.json'), JSON.stringify({
    validRed, summary,
    coverageBoundary: {
      enabledProxyAndRestore: 'visibility, enabled state and hit-test only; not clicked',
      identity: 'standard pointer click reaches supplied direct callback',
      stopped: 'legacy non-Claude model with can_takeover=false; standard disabled click',
      notCovered: ['safe-remap-stopped', 'Tauri IPC', 'tray interaction', 'native app', 'gateway engage', 'gateway failure recovery'],
    },
    passed: cases.filter(item => item.status === 'passed').map(item => item.name),
    failed: failures.map(item => item.name),
    fixtureErrors: failures.filter(item => item.failureKind !== 'missing-proxy-for-safe-remap'),
    assertionCounts: DESKTOP_ROUTING_COUNTS, cases, checks, browserExceptions: exceptions,
  }, null, 2));
  assert.deepEqual(cases.map(item => item.name), Object.keys(DESKTOP_ROUTING_COUNTS), 'Every Desktop routing case must execute');
  assert.equal(cases.length, 8, 'Fixed Desktop routing scenario count prevents zero-test success');
  assert.equal(failures.length, 0, 'Desktop routing failures: ' + JSON.stringify(summary));
  assert.equal(exceptions.length, 0, 'Unhandled browser exceptions remain failures');
  assert.equal(checks.length, 28, 'Fixed Desktop routing assertion count prevents partial success');
  console.log(`Passed ${cases.length} Desktop routing scenarios (${checks.length} assertions).`);
  return checks;
}
