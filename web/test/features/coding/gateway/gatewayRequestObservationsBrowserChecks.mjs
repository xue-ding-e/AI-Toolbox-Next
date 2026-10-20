import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { HTML_LIKE_SUMMARY } from './fixtures/gatewayRequestObservationsData.ts';

const SCENARIOS = ['state-matrix', 'filtered', 'rewrites', 'legacy', 'empty', 'safe-summary'];
const LOCALES = {
  'zh-CN': { native: '原生工具', rewrites: '网关改写', declared: '已声明', forwarded: '已转发', used: '已调用' },
  'en-US': { native: 'Native tools', rewrites: 'Gateway rewrites', declared: 'declared', forwarded: 'forwarded', used: 'used' },
};

export async function verifyGatewayRequestObservations({ send, evaluate, baseUrl, artifactRoot }) {
  const checks = [];
  const completed = [];
  const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
  const check = (name, actual, expected = true) => {
    assert.deepEqual(actual, expected, name);
    checks.push(name);
    console.log('PASS ' + name);
  };
  const waitFor = async expression => {
    for (let attempt = 0; attempt < 150; attempt++) {
      if (await evaluate(expression)) return;
      await delay(50);
    }
    throw new Error('Timed out waiting for: ' + expression);
  };
  const action = expression => evaluate('requestObservationsFixture.' + expression);
  const section = label => action('section(' + JSON.stringify(label) + ')');
  const click = key => action('click(' + JSON.stringify(key) + ')');
  const rows = (...texts) => texts.map(text => ({ text, title: null }));

  for (const [language, labels] of Object.entries(LOCALES)) {
    const runId = randomUUID();
    await send('Page.navigate', { url: baseUrl + '/?language=' + language + '&runId=' + runId });
    await waitFor('window.requestObservationsFixture?.state.runId === ' + JSON.stringify(runId)
      + ' && document.querySelectorAll("[data-row-key]").length === 6 && !document.querySelector("[aria-busy=true]")');
    check(language + ': list loads only summaries before selection', await action(
      'state.requests.filter(request => request.command !== "proxy_gateway_session_usage_rebuild_progress").map(request => request.command)'), ['proxy_gateway_request_logs']);
    check(language + ': list does not expose observation sections', await evaluate(
      '!document.body.textContent.includes(' + JSON.stringify(labels.native) + ') && !document.body.textContent.includes(' + JSON.stringify(labels.rewrites) + ')'));

    for (const scenario of SCENARIOS) {
      const name = language + '/' + scenario;
      const startCount = checks.length;
      const traceId = 'fixture-observations-' + scenario;
      const requestCount = await action('state.requests.length');
      await action('open(' + JSON.stringify(scenario) + ')');
      await waitFor('[...document.querySelectorAll("[role=dialog] code")].some(element => element.textContent === ' + JSON.stringify(traceId) + ')');
      check(name + ': selection fetches exactly the selected detail through the real API',
        await action('state.requests.slice(' + requestCount + ').filter(request => request.command !== "proxy_gateway_session_usage_rebuild_progress")'),
        [{ command: 'proxy_gateway_request_log_detail', args: { traceId } }]);

      let expectedNative = [];
      let expectedRewrites = [];
      if (scenario === 'state-matrix') {
        // These are independent expected DOM strings, not a copy of the
        // component's boolean filtering or the fixture DTO mapping.
        expectedNative = rows(
          `web_search · ${labels.declared}`,
          `file_search · ${labels.forwarded}`,
          `image_generation · ${labels.used}`,
          `mcp · ${labels.declared} · ${labels.forwarded}`,
          `code_interpreter · ${labels.declared} · ${labels.forwarded} · ${labels.used}`,
        );
      } else if (scenario === 'filtered') {
        expectedNative = rows(`web_search · ${labels.declared}`);
        expectedRewrites = [{ text: 'native_tool_filtered · web_search', title: 'web_search' }];
      } else if (scenario === 'rewrites') {
        expectedRewrites = [
          { text: 'lossy_conversion · native tools normalized', title: 'native tools normalized' },
          { text: 'prompt_cache_key', title: null },
          { text: 'service_tier', title: null },
          { text: 'billing_cch_stripped', title: '' },
        ];
      } else if (scenario === 'safe-summary') {
        expectedRewrites = [{ text: 'lossy_conversion · ' + HTML_LIKE_SUMMARY, title: HTML_LIKE_SUMMARY }];
      }
      check(name + ': localized native statuses match only recorded flags', await section(labels.native),
        { count: expectedNative.length ? 1 : 0, rows: expectedNative });
      check(name + ': localized rewrites preserve summaries and absent detail', await section(labels.rewrites),
        { count: expectedRewrites.length ? 1 : 0, rows: expectedRewrites });
      check(name + ': no raw translation key is rendered', await evaluate(
        '!document.querySelector("[role=dialog]").textContent.includes("gateway.page.requests.observations.")'));

      if (scenario === 'legacy' || scenario === 'empty') {
        await click('gateway.page.requests.detailTabs.body');
        await waitFor('!!document.querySelector("[role=dialog] pre")');
        check(name + ': body uses the stored content or the real empty-state copy', await evaluate(
          'document.querySelector("[role=dialog] pre").textContent'), scenario === 'legacy'
          ? '{"tools":[{"type":"web_search"}],"fixture":"legacy-body"}'
          : language === 'zh-CN' ? '未保存该明细。可在网关设置中开启对应记录项。'
            : 'This detail was not stored. Enable the matching log item in gateway settings.');
        await click('gateway.page.requests.detailTabs.record');
        await waitFor('[...document.querySelectorAll("[role=dialog] code")].some(element => element.textContent === ' + JSON.stringify(traceId) + ')');
        check(name + ': returning from body does not infer native tools', await section(labels.native), { count: 0, rows: [] });
        check(name + ': returning from body does not infer rewrites', await section(labels.rewrites), { count: 0, rows: [] });
      }
      if (scenario === 'safe-summary') {
        check(name + ': HTML-like summary creates no injected element', await evaluate(
          'document.querySelectorAll("[data-observation-injection]").length'), 0);
        check(name + ': HTML-like summary executes no script or event handler', await evaluate('window.__observationExecuted'), false);
      }
      check(name + ': no unapproved IPC or inline load error', await evaluate(
        'requestObservationsFixture.state.unexpected.length === 0 && !document.querySelector("[role=alert]")'));
      await click('common.close');
      await waitFor('!document.querySelector("[role=dialog]")');
      check(name + ': detail closes before the next selection', await evaluate('!document.querySelector("[role=dialog]")'));
      assert.ok(checks.length - startCount >= 6, 'Scenario did not execute its mandatory checks: ' + name);
      completed.push(name);
    }
  }

  // A skipped loop or deleted scenario must never become a zero-test green run.
  assert.equal(completed.length, 12, 'Expected all 12 localized observation scenarios');
  assert.deepEqual(completed, Object.keys(LOCALES).flatMap(language => SCENARIOS.map(scenario => language + '/' + scenario)));
  assert.equal(checks.length, 92, 'Expected every observation assertion to execute');
  await writeFile(path.join(artifactRoot, 'scenarios.json'), JSON.stringify({
    scope: 'Real GatewayRequestsView DOM with synthetic IPC; not a desktop or full-page acceptance run',
    scenarios: completed,
    assertions: checks.length,
  }, null, 2));
  console.log(`Passed ${completed.length} localized request-observation scenarios (${checks.length} assertions).`);
  return checks;
}
