import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { verifyRestorePassword } from './restorePasswordBrowserChecks.mjs';
import { verifyRestoreSuccess } from './restoreSuccessBrowserChecks.mjs';

export async function verifyBackupSettings({ send, evaluate, baseUrl, artifactRoot }) {
  const checks = [];
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  const waitFor = async expression => {
    for (let attempt = 0; attempt < 100; attempt++) {
      if (await evaluate(expression)) return;
      await delay(50);
    }
    throw new Error('Timed out: ' + expression);
  };
  const record = (name, actual) => checks.push({ name, actual });
  const call = (method, ...args) => evaluate('backupFixture.' + method + '(' + args.map(JSON.stringify).join(',') + ')');
  const navigate = async query => {
    await send('Page.navigate', { url: baseUrl + '/?' + new URLSearchParams(query) });
    await waitFor('!!window.backupFixture && !!document.querySelector("[data-testid=open-settings]")');
  };
  const openSettings = async () => {
    await call('open');
    await waitFor('backupFixture.canSave()');
    await delay(350);
  };
  const screenshot = async name => {
    await delay(350);
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    await writeFile(path.join(artifactRoot, name + '.png'), Buffer.from(data, 'base64'));
  };
  const assertFade = async (name, shouldOverflow) => {
    await waitFor('!!document.querySelector("[data-scroll-fade-visible]")');
    await waitFor('document.querySelector("[data-scroll-fade-visible]").dataset.scrollFadeVisible === ' + JSON.stringify(String(shouldOverflow)));
    const measured = await evaluate('(() => {'
      + 'const body=document.querySelector(".ant-modal-body");'
      + 'const fade=body.querySelector("[data-scroll-fade-visible]");'
      + 'const before=body.scrollHeight;fade.style.display="none";const without=body.scrollHeight;fade.style.display="";'
      + 'const footer=document.querySelector(".ant-modal-footer");'
      + 'return {height:body.clientHeight,scrollHeight:before,without,scrollWidth:body.scrollWidth,width:body.clientWidth,'
      + 'anchorHeight:fade.getBoundingClientRect().height,anchorBottom:fade.getBoundingClientRect().bottom,'
      + 'bodyBottom:body.getBoundingClientRect().bottom,pointerEvents:getComputedStyle(fade).pointerEvents,'
      + 'hidden:fade.getAttribute("aria-hidden"),footerBottom:footer?.getBoundingClientRect().bottom,viewport:innerHeight};'
      + '})()');
    assert.equal(measured.scrollHeight, measured.without, name + ': fade must not create scroll height');
    assert.equal(measured.anchorHeight, 0, name + ': overlay anchor has no flow height');
    assert.equal(measured.pointerEvents, 'none');
    assert.equal(measured.hidden, 'true');
    assert.ok(measured.scrollWidth <= measured.width + 1, name + ': no horizontal overflow');
    assert.ok(!measured.footerBottom || measured.footerBottom < measured.viewport, name + ': footer visible');
    if (shouldOverflow) {
      assert.ok(measured.scrollHeight > measured.height);
      assert.ok(Math.abs(measured.anchorBottom - measured.bodyBottom) < 2, name + ': fade follows visible bottom');
    }
    record(name, measured);
  };

  // Run each race in a fresh document. Deferred service promises control order;
  // assertions inspect the real component DOM and real row selection callbacks.
  const verifyRemoteLifecycle = async () => {
    const failures = [];
    let completed = 0;
    const check = (name, actual, expected) => {
      try { assert.deepEqual(actual, expected, name); record(name, actual); }
      catch (error) { failures.push(error); console.error(error.message); }
    };
    const begin = async (nextChannel = 'repository') => {
      await navigate({ theme: 'light', language: 'en-US', remoteLifecycle: '1' });
      await call('openRemote', 'webdav');
      await waitFor('backupFixture.state.remoteLoads.length === 1');
      await call('closeRemote');
      await call('openRemote', nextChannel);
      await waitFor('backupFixture.state.remoteLoads.length === 2');
      assert.deepEqual(await evaluate('backupFixture.state.remoteLoads.map(request => request.channel)'),
        ['webdav', nextChannel], 'both independent service requests must actually start');
      assert.equal((await call('remoteSnapshot')).loading, true, 'new request starts loading');
    };
    const expectedFiles = channel => ['ai-toolbox-backup-20261019-120000.zip', channel + '-only.zip'];
    const assertSelection = async (name, channel) => {
      await call('selectRemote', 'ai-toolbox-backup-20261019-120000.zip');
      const selections = (await call('remoteSnapshot')).selections;
      check(name + ': selected file retains current channel and SHA', selections.map(({ channel, file }) =>
        ({ channel, filename: file.filename, sha: file.sha })), [{ channel,
        filename: 'ai-toolbox-backup-20261019-120000.zip', sha: channel + '-shared-sha' }]);
    };
    for (const older of ['success', 'error']) {
      for (const newer of ['success', 'error', 'pending']) {
        const name = 'remote lifecycle: old ' + older + ' after new ' + newer;
        await begin();
        if (newer !== 'pending') await call('settleRemote', 1, newer);
        const before = await call('remoteSnapshot');
        // Positive control: a current error must still be surfaced, not globally swallowed.
        if (newer === 'error') {
          assert.ok(before.errors.some(error => error.includes('repository-request-1-failed')),
            name + ': current repository error is visible');
        }
        await call('settleRemote', 0, older);
        const after = await call('remoteSnapshot');
        check(name + ': obsolete completion preserves loading', after.loading, newer === 'pending');
        check(name + ': obsolete completion preserves current list', after.files,
          newer === 'success' ? expectedFiles('repository') : []);
        check(name + ': obsolete rejection does not add an error', after.errors, before.errors);
        if (newer === 'pending') {
          await call('settleRemote', 1, 'success');
          check(name + ': current completion renders repository', (await call('remoteSnapshot')).files,
            expectedFiles('repository'));
        }
        if (newer !== 'error') await assertSelection(name, 'repository');
        completed++;
      }
    }
    // Same-channel reopen must invalidate the earlier session too; a channel-only
    // guard is insufficient. Distinct per-request SHA proves which session won.
    await begin('webdav');
    await evaluate('backupFixture.state.remoteLoads[0].files[0].sha="obsolete-session-sha"');
    await call('settleRemote', 1, 'success');
    await call('settleRemote', 0, 'success');
    await assertSelection('same channel close/reopen', 'webdav');
    completed++;

    await navigate({ theme: 'light', language: 'en-US', remoteLifecycle: '1' });
    await call('openRemote', 'webdav');
    await waitFor('backupFixture.state.remoteLoads.length === 1');
    await call('closeRemote');
    await call('settleRemote', 0, 'error');
    check('closed modal ignores obsolete error', (await call('remoteSnapshot')).errors, []);
    await call('openRemote', 'repository');
    await waitFor('backupFixture.state.remoteLoads.length === 2');
    await call('settleRemote', 1, 'success');
    await assertSelection('reopen after closed request rejects', 'repository');
    completed++;
    // A completed previous session is also stale: if the new channel fails,
    // its modal must not offer the previous channel's otherwise valid entries.
    await navigate({ theme: 'light', language: 'en-US', remoteLifecycle: '1' });
    await call('openRemote', 'webdav');
    await waitFor('backupFixture.state.remoteLoads.length === 1');
    await call('settleRemote', 0, 'success');
    assert.deepEqual((await call('remoteSnapshot')).files, expectedFiles('webdav'),
      'previous session really rendered its files');
    await call('closeRemote');
    await call('openRemote', 'repository');
    await waitFor('backupFixture.state.remoteLoads.length === 2');
    await call('settleRemote', 1, 'error');
    const failedReopen = await call('remoteSnapshot');
    check('failed new channel clears previous successful list', failedReopen.files, []);
    check('failed new channel finishes its own loading', failedReopen.loading, false);
    assert.ok(failedReopen.errors.some(error => error.includes('repository-request-1-failed')),
      'failed new channel still reports its current error');
    check('failed new channel has no restore selection', failedReopen.selections, []);
    completed++;
    assert.equal(completed, 9, 'all remote lifecycle scenarios must execute');
    console.log('Executed ' + completed + ' remote lifecycle scenarios; assertion failures: ' + failures.length);
    if (failures.length) throw new AggregateError(failures, 'Remote backup lifecycle regressions: ' + failures.length);
  };
  // Delete races use the real Modal, row button and Popconfirm. Only the remote
  // service promise is deferred; no test implementation of handleDelete exists.
  const verifyRemoteDeleteLifecycle = async () => {
    const failures = [];
    let completed = 0;
    const shared = 'ai-toolbox-backup-20261019-120000.zip';
    const expectedFiles = channel => [shared, channel + '-only.zip'];
    const snapshot = () => call('remoteDeleteSnapshot');
    const check = (name, actual, expected) => {
      try { assert.deepEqual(actual, expected, name); record(name, actual); }
      catch (error) { failures.push(error); console.error(error.message); }
    };
    const begin = async () => {
      await navigate({ theme: 'light', language: 'en-US', remoteLifecycle: '1', remoteDeleteLifecycle: '1' });
      await call('openRemote', 'webdav');
      await waitFor('backupFixture.state.remoteLoads.length === 1');
      await call('settleRemote', 0, 'success');
      assert.deepEqual((await snapshot()).files, expectedFiles('webdav'), 'delete source list really rendered');
      await call('openDeleteConfirmation', shared);
      await waitFor('!![...document.querySelectorAll(".ant-popconfirm")].find(element => element.getClientRects().length && getComputedStyle(element).visibility !== "hidden")');
      const before = await snapshot();
      assert.deepEqual(before.deletes, [], 'opening confirmation does not delete');
      assert.deepEqual(before.selections, [], 'delete button does not select restore');
      assert.equal(await call('visibleModalCount'), 1, 'delete button leaves modal open');
    };
    const confirm = async () => {
      await call('answerDeleteConfirmation', true);
      await waitFor('backupFixture.state.remoteDeletes.length === 1');
      const pending = await snapshot();
      assert.deepEqual(pending.deletes, [{ channel: 'webdav', filename: shared,
        sha: 'webdav-shared-sha', settled: false }], 'confirmation sends exact original file once');
      assert.deepEqual(pending.files, expectedFiles('webdav'), 'pending deletion keeps list');
      assert.deepEqual(pending.selections, [], 'confirmation never restores');
      assert.deepEqual(pending.errors, [], 'pending deletion has no error toast');
      assert.deepEqual(pending.successes, [], 'pending deletion has no success toast');
    };
    const assertRequest = async name => {
      const state = await snapshot();
      check(name + ': one original service delete only', state.deletes, [{ channel: 'webdav',
        filename: shared, sha: 'webdav-shared-sha', settled: true }]);
      check(name + ': no accidental restore selection', state.selections, []);
    };
    // Positive controls reject fixes which suppress every delete result or skip
    // the actual confirmation. Cancel must invoke neither delete nor restore.
    for (const outcome of ['success', 'error', 'cancel']) {
      const name = 'remote delete current ' + outcome;
      await begin();
      if (outcome === 'cancel') {
        await call('answerDeleteConfirmation', false);
        await waitFor('![...document.querySelectorAll(".ant-popconfirm")].some(element => element.getClientRects().length && getComputedStyle(element).visibility !== "hidden")');
        const state = await snapshot();
        check(name + ': no service delete', state.deletes, []);
        check(name + ': no restore selection', state.selections, []);
        check(name + ': list retained', state.files, expectedFiles('webdav'));
        check(name + ': no success toast', state.successes, []);
        check(name + ': no error toast', state.errors, []);
        check(name + ': modal stays open', await call('visibleModalCount'), 1);
      } else {
        await confirm();
        await call('settleRemoteDelete', 0, outcome);
        const state = await snapshot();
        check(name + ': expected list', state.files,
          outcome === 'success' ? ['webdav-only.zip'] : expectedFiles('webdav'));
        check(name + ': success toast count', state.successes.length, outcome === 'success' ? 1 : 0);
        check(name + ': error toast count', state.errors.length, outcome === 'error' ? 1 : 0);
        if (outcome === 'error') check(name + ': current error remains visible',
          state.errors.some(error => error.includes('webdav-delete-0-failed')), true);
        await assertRequest(name);
      }
      completed++;
    }
    for (const nextChannel of ['repository', 'webdav']) {
      for (const outcome of ['success', 'error']) {
        const name = 'remote delete old ' + outcome + ' after reopen ' + nextChannel;
        await begin();
        await confirm();
        await call('closeRemote');
        await waitFor('backupFixture.visibleModalCount() === 0');
        await call('openRemote', nextChannel);
        await waitFor('backupFixture.state.remoteLoads.length === 2');
        // Same filename, distinct revision even for a same-channel reopen.
        const currentSha = nextChannel + '-reopened-sha';
        await evaluate('backupFixture.state.remoteLoads[1].files[0].sha=' + JSON.stringify(currentSha));
        await call('settleRemote', 1, 'success');
        const before = await snapshot();
        assert.deepEqual(before.files, expectedFiles(nextChannel), name + ': current list is loaded');
        await call('settleRemoteDelete', 0, outcome);
        const after = await snapshot();
        check(name + ': current list preserved', after.files, before.files);
        check(name + ': loading preserved', after.loading, before.loading);
        check(name + ': no obsolete success toast', after.successes, before.successes);
        check(name + ': no obsolete error toast', after.errors, before.errors);
        await assertRequest(name);
        // A missing row is already recorded above; keep running all race cases
        // in RED rather than aborting on a secondary DOM lookup failure.
        if (after.files.includes(shared)) {
          await call('selectRemote', shared);
          check(name + ': real row selection uses new SHA', (await snapshot()).selections.map(({ channel, file }) =>
            ({ channel, filename: file.filename, sha: file.sha })),
          [{ channel: nextChannel, filename: shared, sha: currentSha }]);
        }
        completed++;
      }
    }
    for (const outcome of ['success', 'error']) {
      const name = 'remote delete closed late ' + outcome;
      await begin();
      await confirm();
      await call('closeRemote');
      await waitFor('backupFixture.visibleModalCount() === 0');
      await call('settleRemoteDelete', 0, outcome);
      const closed = await snapshot();
      check(name + ': remains closed', await call('visibleModalCount'), 0);
      check(name + ': no success toast', closed.successes, []);
      check(name + ': no error toast', closed.errors, []);
      await assertRequest(name);
      await call('openRemote', 'repository');
      await waitFor('backupFixture.state.remoteLoads.length === 2');
      await call('settleRemote', 1, 'success');
      check(name + ': next session loads intact', (await snapshot()).files, expectedFiles('repository'));
      completed++;
    }
    assert.equal(completed, 9, 'all remote delete lifecycle scenarios must execute');
    console.log('Executed ' + completed + ' remote delete lifecycle scenarios; assertion failures: ' + failures.length);
    if (failures.length) throw new AggregateError(failures, 'Remote backup delete lifecycle regressions: ' + failures.length);
  };
  const group = process.env.BACKUP_SETTINGS_CHECK_GROUP || 'all';
  assert.ok(['all', 'remote-lifecycle', 'remote-delete-lifecycle', 'restore-password', 'restore-success'].includes(group), 'Unknown backup settings check group: ' + group);
  if (group === 'all' || group === 'restore-success') {
    await verifyRestoreSuccess({ send, evaluate, navigate, waitFor, delay, record, screenshot });
  }
  if (group === 'restore-success') return checks;
  if (group === 'all' || group === 'restore-password') {
    await verifyRestorePassword({ send, evaluate, navigate, waitFor, delay, record, screenshot, call });
  }
  if (group === 'restore-password') return checks;
  if (group === 'all' || group === 'remote-delete-lifecycle') await verifyRemoteDeleteLifecycle();
  if (group === 'remote-delete-lifecycle') return checks;
  await verifyRemoteLifecycle();
  if (group === 'remote-lifecycle') return checks;

  for (const scenario of [
    { theme: 'light', language: 'zh-CN', system: 'light' },
    { theme: 'dark', language: 'en-US', system: 'dark' },
    { theme: 'system', language: 'en-US', system: 'light' },
    { theme: 'system', language: 'zh-CN', system: 'dark' },
  ]) {
    const name = scenario.theme + '-' + scenario.system;
    await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: scenario.system }] });
    for (const channel of ['local', 'webdav', 'repository']) {
      await navigate({ ...scenario, channel });
      await openSettings();
      if (channel === 'local') {
        await call('toggle', 'settings.autoBackup.title');
        await call('number', 'settings.autoBackup.interval', '2');
        await call('number', 'settings.autoBackup.maxKeep', '3');
      }
      if (channel === 'repository') {
        assert.equal(await evaluate('document.querySelector("#repository_directory").value'), '', 'root directory remains empty');
        await assertFade(name + ' settings initial overflow', true);
        await screenshot(name + '-settings');
        await evaluate('document.querySelector(".ant-modal-body").scrollTop=100000');
        await waitFor('document.querySelector("[data-scroll-fade-visible]").dataset.scrollFadeVisible === "false"');
        await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 1400, deviceScaleFactor: 1, mobile: false });
        await assertFade(name + ' no overflow after resize', false);
        await send('Emulation.setDeviceMetricsOverride', { width: 800, height: 650, deviceScaleFactor: 1, mobile: false });
        await assertFade(name + ' narrow settings', true);
        await screenshot(name + '-narrow');
        await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
      }
      await call('click', 'common.save');
      await waitFor('backupFixture.state.closed');
      const saved = await evaluate('backupFixture.savedRequests()');
      assert.equal(saved.length, 1);
      const payload = saved[0].args.payload;
      assert.equal(payload.backup_type, channel);
      assert.equal(payload.webdav.url, 'https://dav.example.invalid');
      assert.equal(payload.repository.owner, 'fixture-owner');
      assert.equal(payload.repository.directory, '');
      assert.equal(payload.encryption_password, undefined);
      if (channel === 'local') {
        assert.deepEqual(await call('autoSettings'), { enabled: true, interval: 2, maxKeep: 3 });
        await delay(350);
        await openSettings();
        assert.deepEqual(await call('autoSettings'), { enabled: true, interval: 2, maxKeep: 3 });
        await call('click', 'common.save');
        await waitFor('backupFixture.state.closed');
        const savedAgain = await evaluate('backupFixture.savedRequests().at(-1).args.payload');
        assert.equal(savedAgain.auto_backup_enabled, true);
        assert.equal(savedAgain.auto_backup_interval_days, 2);
        assert.equal(savedAgain.auto_backup_max_keep, 3);
      }
      record(name + ' ' + channel + ' settings round trip', payload.backup_type);
    }

    await navigate({ ...scenario, deferFiles: '1' });
    await call('open', 'remote');
    await waitFor('backupFixture.state.pendingFileLoads.length === 1');
    await assertFade(name + ' remote loading', false);
    await call('resolveFiles');
    await waitFor('document.querySelectorAll(".ant-list-item").length === 24');
    await assertFade(name + ' remote loaded', true);
    await screenshot(name + '-remote');
    await evaluate('document.querySelector(".ant-modal-body").scrollTop=100000');
    await waitFor('document.querySelector("[data-scroll-fade-visible]").dataset.scrollFadeVisible === "false"');
  }

  await navigate({ theme: 'light', channel: 'repository', deferLoad: '1' });
  await call('open');
  await waitFor('backupFixture.state.pendingLoads.length === 1');
  assert.equal(await call('canSave'), false);
  await call('click', 'common.save');
  assert.equal((await call('savedRequests')).length, 0);
  await call('resolveLoad');
  await waitFor('backupFixture.canSave()');
  record('repository cannot be cleared before its stored connection loads', true);

  await call('input', '#repository_owner', '');
  await call('channel', 'local');
  await call('click', 'common.save');
  await waitFor('backupFixture.state.closed');
  assert.equal((await call('savedRequests'))[0].args.payload.repository.owner, 'fixture-owner');
  record('hidden incomplete repository draft preserves loaded connection', true);

  await navigate({ theme: 'light', channel: 'repository' });
  await openSettings();
  await call('input', 'input[autocomplete="new-password"]', 'fixture-github-token');
  await call('platform', 'Gitee');
  assert.equal(await evaluate('document.querySelector("input[autocomplete=new-password]").value'), '');
  await call('click', 'settings.webdav.testConnection');
  const testRequest = await evaluate('backupFixture.state.requests.find(request => request.command==="test_backup_repository_connection")');
  assert.equal(testRequest.args.config.platform, 'gitee');
  assert.equal(testRequest.args.token, null);
  record('platform switch clears draft token before connection test', true);

  await navigate({ theme: 'dark', channel: 'webdav', language: 'en-US' });
  await openSettings();
  await call('toggle', 'settings.backupSettings.encryption.title');
  const passwordLabel = await evaluate('(() => {const label=[...document.querySelectorAll(".ant-form-item-label label")].find(label=>label.textContent==="Encryption password");return label?{width:label.clientWidth,scrollWidth:label.scrollWidth}:null;})()');
  assert.ok(passwordLabel, 'the encryption password has a visible label');
  assert.ok(passwordLabel.scrollWidth <= passwordLabel.width + 1, 'the English password label is not clipped');
  await call('input', 'input[autocomplete="new-password"]', 'fixture-encryption-password');
  await evaluate('backupFixture.state.failSave=true');
  await call('click', 'common.save');
  await waitFor('backupFixture.canSave()');
  assert.equal(await evaluate('backupFixture.state.closed'), false);
  assert.equal(await evaluate('document.querySelector("input[autocomplete=new-password]").value'), 'fixture-encryption-password');
  await call('click', 'common.save');
  await waitFor('backupFixture.state.closed');
  await delay(350);
  await openSettings();
  assert.equal(await evaluate('document.querySelector("input[autocomplete=new-password]").value'), '');
  record('failed save retains password draft; successful close clears it', true);

  await navigate({ theme: 'light', blank: '1', channel: 'local' });
  await openSettings();
  await call('click', 'common.save');
  await waitFor('backupFixture.state.closed');
  record('fresh local settings save without a repository connection', true);

  // Directory entries support regex exclusions: the section only shows for the
  // directory type, invalid patterns block the save inline, and valid patterns
  // round-trip into the saved payload.
  await navigate({ theme: 'light', language: 'zh-CN', channel: 'local' });
  await openSettings();
  await call('click', 'settings.backupSettings.customEntries.add');
  await waitFor('backupFixture.visibleModalCount() === 2');
  assert.equal(
    await call('nestedFormItem', 'settings.backupSettings.customEntries.excludePatterns').then(() => true).catch(() => false),
    false,
    'file entries do not show the exclude section',
  );
  await call('nestedSelectOption', 'settings.backupSettings.customEntries.type', 'settings.backupSettings.customEntries.directory');
  assert.equal(await call('nestedExcludePatternCount'), 0);
  await call('nestedInput', 'settings.backupSettings.customEntries.name', 'memories');
  await call('nestedInput', 'settings.backupSettings.customEntries.sourcePath', '~/.agents/memories');
  await call('nestedClickButton', 'settings.backupSettings.customEntries.addExcludePattern');
  assert.equal(await call('nestedExcludePatternCount'), 1);
  await screenshot('custom-entry-exclude-modal');
  await call('nestedInput', 'settings.backupSettings.customEntries.excludePatterns', '[', 0);
  await call('nestedClickButton', 'common.save');
  assert.equal(await call('visibleModalCount'), 2, 'invalid regex keeps the entry modal open');
  assert.ok(await evaluate('!!document.querySelector(".ant-form-item-explain-error")'), 'invalid regex shows an inline error');
  await call('nestedInput', 'settings.backupSettings.customEntries.excludePatterns', '^local$', 0);
  await call('nestedClickButton', 'settings.backupSettings.customEntries.addExcludePattern');
  await call('nestedInput', 'settings.backupSettings.customEntries.excludePatterns', '(^|/)\\.git$', 1);
  await call('nestedClickButton', 'common.save');
  await waitFor('backupFixture.visibleModalCount() === 1');
  await screenshot('custom-entry-exclude-list');
  assert.ok(
    await evaluate('document.body.textContent.includes("^local$") && document.body.textContent.includes(".git$")'),
    'the entry list shows the configured exclude rules',
  );
  await call('click', 'common.save');
  await waitFor('backupFixture.state.closed');
  const excludePayload = (await call('savedRequests')).at(-1).args.payload.backup_custom_entries;
  assert.deepEqual(excludePayload.map(entry => entry.exclude_patterns), [['^local$', '(^|/)\\.git$']]);
  assert.equal(excludePayload[0].entry_type, 'directory');
  record('directory exclude rules validate inline and round trip', true);

  // Reopening the saved entry must pre-fill the rules and a file-type switch
  // must drop them, so editing never silently loses or keeps stale patterns.
  await openSettings();
  await call('clickLabel', 'settings.backupSettings.customEntries.edit');
  await waitFor('backupFixture.visibleModalCount() === 2');
  assert.equal(await call('nestedInputValue', 'settings.backupSettings.customEntries.excludePatterns', 0), '^local$');
  assert.equal(await call('nestedInputValue', 'settings.backupSettings.customEntries.excludePatterns', 1), '(^|/)\\.git$');
  await call('nestedSelectOption', 'settings.backupSettings.customEntries.type', 'settings.backupSettings.customEntries.file');
  await waitFor('!document.querySelector(".ant-form-item-explain-error")');
  await call('nestedClickButton', 'common.save');
  await waitFor('backupFixture.visibleModalCount() === 1');
  assert.ok(
    !await evaluate('document.body.textContent.includes("^local$")'),
    'switching to the file type drops the exclude rules from the entry',
  );
  record('editing pre-fills exclude rules and a file switch drops them', true);

  await navigate({ theme: 'light' });
  await evaluate('backupFixture.state.files=[]');
  await call('open', 'remote');
  await waitFor('!!document.querySelector(".ant-empty")');
  await assertFade('empty remote list', false);
  await screenshot('empty-remote');
  return checks;
}
