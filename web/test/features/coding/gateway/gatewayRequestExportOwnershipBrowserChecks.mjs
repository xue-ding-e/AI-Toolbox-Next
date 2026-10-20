import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

const SCENARIOS = ['picker-switch', 'export-close-reopen', 'export-unmount'];
const PICKER_COMMAND = 'plugin:dialog|save';
const EXPORT_COMMAND = 'proxy_gateway_export_request_log_detail';
const TRACE_A = 'fixture-owner-a';
const TRACE_B = 'fixture-owner-b';
const pickerCall = traceId => ({
  command: PICKER_COMMAND,
  args: { options: {
    title: 'Export detail',
    defaultPath: 'fixture-model-' + traceId + '-2026-01-01-00-00-01Z.json',
    filters: [{ name: 'JSON', extensions: ['json'] }],
  } },
});
const exportCall = (traceId, destination) => ({
  command: EXPORT_COMMAND,
  args: { traceId, exportPath: '/synthetic/' + destination + '-detail.json' },
});

export async function verifyGatewayRequestExportOwnership({ send, evaluate, baseUrl, artifactRoot, exceptions }) {
  const selection = process.env.GATEWAY_REQUEST_EXPORT_OWNERSHIP_CASE ?? 'all';
  assert.ok(selection === 'all' || SCENARIOS.includes(selection), 'Unknown export ownership scenario: ' + selection);
  const scenarios = selection === 'all' ? SCENARIOS : [selection];
  const checks = [];
  const completed = [];
  const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
  const check = (name, actual, expected = true) => {
    assert.deepEqual(actual, expected, name);
    checks.push(name);
    console.log('PASS ' + name);
  };
  const action = expression => evaluate('requestExportOwnershipFixture.' + expression);
  const commands = command => action('state.requests.filter(request => request.command === ' + JSON.stringify(command) + ')');
  const waitFor = async expression => {
    for (let attempt = 0; attempt < 150; attempt++) {
      if (await evaluate(expression)) return;
      await delay(50);
    }
    throw new Error('Timed out waiting for: ' + expression);
  };
  const settle = () => evaluate('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
  const pending = () => action('state.pendingExports');
  const ui = () => evaluate('({ error: requestExportOwnershipFixture.errors(), notice: requestExportOwnershipFixture.notices() })');
  const openDetail = async traceId => {
    await action('openDetail(' + JSON.stringify(traceId) + ')');
    await waitFor('requestExportOwnershipFixture.selectedTrace() === ' + JSON.stringify(traceId));
  };

  for (const scenario of scenarios) {
    const runId = randomUUID();
    const startCount = checks.length;
    const pickerSwitch = scenario === 'picker-switch';
    const currentTrace = scenario === 'export-close-reopen' ? TRACE_A : TRACE_B;
    const oldExportIndex = pickerSwitch ? 1 : 0;
    const newExportIndex = pickerSwitch ? 0 : 1;
    const exportCalls = pickerSwitch
      ? [exportCall(currentTrace, 'new'), exportCall(TRACE_A, 'old')]
      : [exportCall(TRACE_A, 'old'), exportCall(currentTrace, 'new')];
    await send('Page.navigate', { url: baseUrl + '/?scenario=' + scenario + '&runId=' + runId });
    await waitFor('window.requestExportOwnershipFixture?.state.runId === ' + JSON.stringify(runId)
      + ' && document.querySelectorAll("[data-row-key]").length === 2 && !document.querySelector("[aria-busy=true]")');
    await openDetail(TRACE_A);
    try {
      check(scenario + ': setup uses only real list and detail reads', await action(
        'state.requests.filter(request => request.command !== "proxy_gateway_session_usage_rebuild_progress").map(request => request.command)'),
      ['proxy_gateway_request_logs', 'proxy_gateway_request_log_detail']);
      check(scenario + ': initial export is enabled', await action('exportDisabled()'), false);
      await action('clickExport()');
      await waitFor('requestExportOwnershipFixture.state.pendingPickers === 1 && requestExportOwnershipFixture.exportDisabled()');
      check(scenario + ': original picker options retain trace A', await commands(PICKER_COMMAND), [pickerCall(TRACE_A)]);
      check(scenario + ': original picker owns busy state', await action('exportDisabled()'));
      if (!pickerSwitch) {
        await action('resolvePicker(0, "old")');
        await waitFor('requestExportOwnershipFixture.state.pendingExports === 1');
      }
      check(scenario + ': original export uses only the selected original destination', await commands(EXPORT_COMMAND),
        pickerSwitch ? [] : [exportCall(TRACE_A, 'old')]);

      if (scenario === 'export-close-reopen') {
        await action('closeDetail()');
        await waitFor('!document.querySelector("[role=dialog]")');
      } else if (scenario === 'export-unmount') {
        await action('unmountView()');
        await waitFor('!document.querySelector("[role=dialog]") && !document.querySelector("[data-row-key]")');
        await action('mountView()');
        await waitFor('document.querySelectorAll("[data-row-key]").length === 2 && !document.querySelector("[aria-busy=true]")');
      }
      await openDetail(currentTrace);
      check(scenario + ': the replacement detail is rendered', await action('selectedTrace()'), currentTrace);
      check(scenario + ': detached work does not disable the replacement detail', await action('exportDisabled()'), false);
      await action('clickExport()');
      await waitFor('requestExportOwnershipFixture.exportDisabled() && requestExportOwnershipFixture.state.requests.filter(request => request.command === "plugin:dialog|save").length === 2');
      check(scenario + ': both picker calls retain their own detail filename', await commands(PICKER_COMMAND),
        [pickerCall(TRACE_A), pickerCall(currentTrace)]);
      check(scenario + ': replacement picker owns its busy state', await action('exportDisabled()'));
      await action('resolvePicker(1, "new")');
      await waitFor('requestExportOwnershipFixture.state.pendingExports === ' + (pickerSwitch ? 1 : 2));
      check(scenario + ': replacement export retains its chosen trace and path', await commands(EXPORT_COMMAND),
        pickerSwitch ? [exportCall(currentTrace, 'new')] : exportCalls);
      check(scenario + ': replacement stays busy during backend export', await action('exportDisabled()'));

      if (pickerSwitch) {
        await action('resolvePicker(0, "old")');
        await waitFor('requestExportOwnershipFixture.state.pendingExports === 2');
      }
      check(scenario + ': detached export continues with its original trace and actual picker path',
        await commands(EXPORT_COMMAND), exportCalls);
      check(scenario + ': both authorized exports remain pending', await pending(), 2);
      await action('settleExport(' + oldExportIndex + ', ' + JSON.stringify(pickerSwitch ? 'success' : 'reject') + ')');
      await settle();
      check(scenario + ': detached completion does not publish success or error into replacement UI', await ui(), { error: '', notice: '' });
      check(scenario + ': detached finally does not release the replacement busy state', await action('exportDisabled()'));
      check(scenario + ': replacement export remains pending', await pending(), 1);
      await action('clickExport()');
      await action('clickExport()');
      await settle();
      check(scenario + ': detached finally cannot admit another picker', await commands(PICKER_COMMAND),
        [pickerCall(TRACE_A), pickerCall(currentTrace)]);
      check(scenario + ': detached finally cannot duplicate export', await commands(EXPORT_COMMAND), exportCalls);

      await action('settleExport(' + newExportIndex + ', "success")');
      await waitFor('requestExportOwnershipFixture.notices() === "Request detail exported" && !requestExportOwnershipFixture.exportDisabled()');
      check(scenario + ': replacement completion preserves the original success message', await ui(),
        { error: '', notice: 'Request detail exported' });
      check(scenario + ': replacement completion restores its button', await action('exportDisabled()'), false);
      check(scenario + ': all selected operations settle', await evaluate(
        '[requestExportOwnershipFixture.state.pendingPickers, requestExportOwnershipFixture.state.pendingExports]'), [0, 0]);
      check(scenario + ': strict IPC allowlist has no unexpected calls', await action('state.unexpected'), []);
      check(scenario + ': no unhandled browser exception', exceptions.length, 0);
      assert.equal(checks.length - startCount, 23, 'Incomplete ownership assertions for ' + scenario);
      completed.push(scenario);
    } finally {
      try {
        await writeFile(path.join(artifactRoot, scenario + '.json'), JSON.stringify({
          scenario,
          passed: completed.includes(scenario),
          checks: checks.slice(startCount),
          observed: await evaluate('({ state: requestExportOwnershipFixture.state, selected: requestExportOwnershipFixture.selectedTrace(), error: requestExportOwnershipFixture.errors(), notice: requestExportOwnershipFixture.notices() })'),
          browserExceptions: exceptions,
        }, null, 2));
      } catch (error) {
        console.error('Could not record export ownership diagnostics:', error.message);
      }
    }
  }
  assert.deepEqual(completed, scenarios, 'Every selected ownership scenario must execute');
  assert.ok(completed.length > 0);
  assert.equal(checks.length, scenarios.length * 23, 'Every selected ownership assertion must execute');
  if (selection === 'all') {
    assert.equal(completed.length, 3);
    assert.equal(checks.length, 69);
  }
  console.log(`Passed ${completed.length} request-export ownership scenarios (${checks.length} assertions).`);
  return checks;
}
