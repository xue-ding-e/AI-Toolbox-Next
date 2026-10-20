import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

const CASE_COUNTS = {
  success: 8,
  cancel: 8,
  'export-pending': 11,
  'picker-rejected': 8,
  'picker-pending': 11,
};
const PICKER_COMMAND = 'plugin:dialog|save';
const EXPORT_COMMAND = 'proxy_gateway_export_request_log_detail';
const PICKER_ARGS = {
  options: {
    title: 'Export detail',
    defaultPath: 'fixture-model-fixture-export-detail-2026-01-01-00-00-01Z.json',
    filters: [{ name: 'JSON', extensions: ['json'] }],
  },
};
const EXPORT_ARGS = {
  traceId: 'fixture-export-detail',
  exportPath: '/synthetic/gateway-request-export.json',
};

export async function verifyGatewayRequestExport({ send, evaluate, baseUrl, artifactRoot, exceptions }) {
  const selection = process.env.GATEWAY_REQUEST_EXPORT_CASE ?? 'all';
  assert.ok(selection === 'all' || Object.hasOwn(CASE_COUNTS, selection), 'Unknown export scenario: ' + selection);
  const scenarios = selection === 'all' ? Object.keys(CASE_COUNTS) : [selection];
  const checks = [];
  const completed = [];
  const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
  const check = (name, actual, expected = true) => {
    assert.deepEqual(actual, expected, name);
    checks.push(name);
    console.log('PASS ' + name);
  };
  const action = expression => evaluate('requestExportFixture.' + expression);
  const commands = command => action('state.requests.filter(request => request.command === ' + JSON.stringify(command) + ')');
  const waitFor = async expression => {
    for (let attempt = 0; attempt < 150; attempt++) {
      if (await evaluate(expression)) return;
      await delay(50);
    }
    throw new Error('Timed out waiting for: ' + expression);
  };
  const settle = () => evaluate('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
  const checkPicker = async name => check(name + ': real save wrapper sends the exact JSON dialog options',
    await commands(PICKER_COMMAND), [{ command: PICKER_COMMAND, args: PICKER_ARGS }]);
  const checkExport = async name => check(name + ': real export wrapper preserves the selected trace and destination',
    await commands(EXPORT_COMMAND), [{ command: EXPORT_COMMAND, args: EXPORT_ARGS }]);

  for (const scenario of scenarios) {
    const runId = randomUUID();
    const startCount = checks.length;
    await send('Page.navigate', { url: baseUrl + '/?scenario=' + scenario + '&runId=' + runId });
    await waitFor('window.requestExportFixture?.state.runId === ' + JSON.stringify(runId)
      + ' && document.querySelectorAll("[data-row-key]").length === 1 && !document.querySelector("[aria-busy=true]")');
    await action('openDetail()');
    await waitFor('[...document.querySelectorAll("[role=dialog] code")].some(element => element.textContent === "fixture-export-detail")');
    try {
      check(scenario + ': setup uses only the real list and detail reads', await action(
        'state.requests.filter(request => request.command !== "proxy_gateway_session_usage_rebuild_progress").map(request => request.command)'),
      ['proxy_gateway_request_logs', 'proxy_gateway_request_log_detail']);
      check(scenario + ': export starts enabled', await action('exportDisabled()'), false);
      await action('clickExport()');
      await waitFor('requestExportFixture.state.requests.some(request => request.command === "plugin:dialog|save")');

      if (scenario === 'success') {
        await waitFor('requestExportFixture.notices() === "Request detail exported" && !requestExportFixture.exportDisabled()');
        await checkPicker(scenario);
        await checkExport(scenario);
        check(scenario + ': completion is shown', await action('notices()'), 'Request detail exported');
        check(scenario + ': button restores after completion', await action('exportDisabled()'), false);
      } else if (scenario === 'cancel') {
        await settle();
        await checkPicker(scenario);
        check(scenario + ': cancelling never invokes export', await commands(EXPORT_COMMAND), []);
        check(scenario + ': cancelling shows neither success nor error', await evaluate(
          '({ notice: requestExportFixture.notices(), error: requestExportFixture.errors() })'), { notice: '', error: '' });
        check(scenario + ': button remains usable after cancel', await action('exportDisabled()'), false);
      } else if (scenario === 'picker-rejected') {
        // Do not catch the component's rejected promise or suppress its global
        // exception. A missing inline error and any unhandled rejection are red.
        await waitFor('requestExportFixture.errors().includes("Synthetic picker rejection")');
        await checkPicker(scenario);
        check(scenario + ': rejected picker never invokes export', await commands(EXPORT_COMMAND), []);
        check(scenario + ': picker failure is exposed by the actual view', await action('errors()'),
          'Failed to load request details: Synthetic picker rejection');
        check(scenario + ': button restores after picker failure', await action('exportDisabled()'), false);
      } else if (scenario === 'picker-pending') {
        await waitFor('requestExportFixture.state.pendingPickers === 1');
        await checkPicker(scenario);
        check(scenario + ': export waits for a selected path', await commands(EXPORT_COMMAND), []);
        // Actual DOM click calls respect disabled controls. Separate evaluations
        // permit React to commit between attempts; no handler is called directly.
        await action('clickExport()');
        await action('clickExport()');
        await settle();
        check(scenario + ': repeated clicks open at most one picker', (await commands(PICKER_COMMAND)).length, 1);
        check(scenario + ': only one picker promise is outstanding', await action('state.pendingPickers'), 1);
        await action('resolvePickers()');
        await waitFor('requestExportFixture.state.pendingPickers === 0 && !requestExportFixture.exportDisabled()');
        await settle();
        check(scenario + ': cancellation after waiting still performs no export', await commands(EXPORT_COMMAND), []);
        check(scenario + ': button restores after deferred cancel', await action('exportDisabled()'), false);
        check(scenario + ': deferred cancel has no completion notice', await action('notices()'), '');
      } else if (scenario === 'export-pending') {
        await waitFor('requestExportFixture.state.pendingExports === 1 && requestExportFixture.exportDisabled()');
        await checkPicker(scenario);
        await checkExport(scenario);
        check(scenario + ': button disables while backend export is pending', await action('exportDisabled()'));
        await action('clickExport()');
        await action('clickExport()');
        await settle();
        check(scenario + ': disabled clicks dispatch no additional picker or export', await evaluate(
          'requestExportFixture.state.requests.filter(request => ["plugin:dialog|save", "proxy_gateway_export_request_log_detail"].includes(request.command)).map(request => request.command)'),
        [PICKER_COMMAND, EXPORT_COMMAND]);
        check(scenario + ': no success is announced before export resolves', await action('notices()'), '');
        await action('resolveExports()');
        await waitFor('requestExportFixture.notices() === "Request detail exported" && !requestExportFixture.exportDisabled()');
        check(scenario + ': resolved export announces success', await action('notices()'), 'Request detail exported');
        check(scenario + ': button restores after deferred export', await action('exportDisabled()'), false);
      }
      check(scenario + ': no IPC falls outside the synthetic allowlist', await action('state.unexpected'), []);
      check(scenario + ': no unhandled browser exception', exceptions.length, 0);
      assert.equal(checks.length - startCount, CASE_COUNTS[scenario], 'Incomplete assertions for ' + scenario);
      completed.push(scenario);
    } finally {
      // Save synthetic evidence even when a tests-first assertion fails. This
      // does not consume, remove, or mark browser exceptions as expected.
      try {
        await writeFile(path.join(artifactRoot, scenario + '.json'), JSON.stringify({
          scenario,
          passed: completed.includes(scenario),
          checks: checks.slice(startCount),
          observed: await evaluate('({ state: requestExportFixture.state, disabled: requestExportFixture.exportDisabled(), error: requestExportFixture.errors(), notice: requestExportFixture.notices() })'),
          browserExceptions: exceptions,
        }, null, 2));
      } catch (error) {
        console.error('Could not record export diagnostics:', error.message);
      }
    }
  }
  assert.deepEqual(completed, scenarios, 'Every selected export scenario must execute');
  const expectedChecks = scenarios.reduce((count, scenario) => count + CASE_COUNTS[scenario], 0);
  assert.ok(expectedChecks > 0);
  assert.equal(checks.length, expectedChecks, 'Every selected export assertion must execute');
  if (selection === 'all') {
    assert.equal(completed.length, 5);
    assert.equal(checks.length, 46);
  }
  console.log(`Passed ${completed.length} request-export scenarios (${checks.length} assertions).`);
  return checks;
}
