import assert from 'node:assert/strict';

// Independent expectations: never derive the route/error oracle from the IPC
// double. These checks drive the mounted page, not its callbacks or flow helper.
const localPlainPath = 'C:/Fixture/Selected/ai-toolbox-backup-20261019-120000.zip';
const localEncryptedPath = 'C:/Fixture/Selected/ai-toolbox-backup-20261019-120001.zip.enc';
const plainFilename = 'ai-toolbox-backup-20261019-120000-abcdef01_RouteHost.zip';
const encryptedFilename = 'ai-toolbox-backup-20261019-120001-abcdef02_RouteHost.zip.enc';
const webdavConnection = {
  url: 'https://restore-routes.example.invalid/dav', username: 'fixture-route-user',
  password: 'fixture-webdav-password', remotePath: '/fixture-backups',
};
const restoreButton = 'settings.backupSettings.restoreBackup';
const confirmationTitle = 'settings.backupSettings.confirmRestore';
const sourceCommands = ['plugin:dialog|open', 'list_webdav_backups', 'list_repository_backups'];

const expectedRestore = (channel, encrypted, skipCliCustomRoots, settled = false) => {
  const options = { skipCliCustomRoots, restorePassword: null };
  if (channel === 'local') return {
    command: 'restore_database',
    args: { zipFilePath: encrypted ? localEncryptedPath : localPlainPath, ...options }, settled,
  };
  if (channel === 'webdav') return {
    command: 'restore_from_webdav',
    args: { ...webdavConnection, filename: encrypted ? encryptedFilename : plainFilename, ...options }, settled,
  };
  return {
    command: 'restore_from_repository',
    args: { filename: encrypted ? encryptedFilename : plainFilename,
      sha: (encrypted ? '2' : '1').repeat(40), ...options }, settled,
  };
};

export async function verifySettingsRestoreErrors({ send, evaluate, baseUrl }) {
  const checks = [];
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  const call = (method, ...args) => evaluate('settingsRestoreFixture.' + method
    + '(' + args.map(JSON.stringify).join(',') + ')');
  const snapshot = () => call('snapshot');
  const waitState = async expression => {
    for (let attempt = 0; attempt < 200; attempt++) {
      if (await evaluate('window.settingsRestoreFixture && (() => {'
        + 'const state = settingsRestoreFixture.snapshot(); return ' + expression + '; })()')) return;
      await delay(50);
    }
    throw new Error('Timed out waiting for settings restore error state: ' + expression);
  };
  const assertSafe = state => {
    assert.deepEqual(state.unexpected, [], 'no unexpected IPC, restart, deletion or credential draft');
    assert.deepEqual(state.subscriptions.toSorted(), [
      'auto-backup-completed', 'auto-backup-failed', 'update-download-progress',
    ]);
  };
  const assertRecovered = state => {
    assert.equal(state.restoreLoading, false, 'the real restore button released loading');
    assert.equal(state.canRestore, true, 'the real restore button is available for a user retry');
    assert.equal(state.localSelectionPending, false);
    assert.equal(state.dialogs, 0, 'failure leaves no blocking dialog');
    assert.equal(state.confirmOpen, false);
    assert.equal(state.passwordOpen, false, 'ordinary errors must not become password prompts');
    assert.equal(state.successOpen, false, 'ordinary errors must not show restore success');
    assertSafe(state);
  };
  const assertSources = (state, channel, count) => {
    const sources = state.requests.filter(request => sourceCommands.includes(request.command));
    assert.equal(sources.length, count, 'only explicit page retries reopen the selection source');
    const command = channel === 'local' ? 'plugin:dialog|open'
      : channel === 'webdav' ? 'list_webdav_backups' : 'list_repository_backups';
    for (const source of sources) {
      assert.equal(source.command, command, 'retries remain in the chosen channel');
      if (channel === 'local') {
        assert.equal(source.args.options.multiple, false);
        assert.notEqual(source.args.options.directory, true);
        assert.deepEqual(source.args.options.filters, [{ name: 'Backup Files', extensions: ['zip', 'enc'] }]);
      } else {
        assert.deepEqual(source.args, channel === 'webdav' ? webdavConnection : {});
      }
    }
  };
  const begin = async (channel, configuration = 'configured') => {
    await send('Page.navigate', { url: baseUrl + '/?' + new URLSearchParams({
      language: 'en-US', theme: 'light', configuration,
    }) });
    await waitState('state.mounted && state.subscriptions.length === 3'
      + ' && state.requests.some(request => request.command === "get_app_data_dir_info")'
      + ' && state.requests.some(request => request.command === "get_backup_repository_settings")');
    // The IPC promises are immediate; allow their resulting React render to
    // commit before testing an incomplete saved repository configuration.
    await evaluate('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve(true))))');
    if (configuration === 'configured') {
      assert.equal((await snapshot()).channel, 'local');
      await call('clickPage', 'common.edit');
      await waitState('state.canSave');
      await call('selectChannel', channel);
      await call('clickDialog', 'settings.backupSettings.title', 'common.save');
      await waitState('state.channel === ' + JSON.stringify(channel) + ' && state.dialogs === 0');
      const saves = (await snapshot()).requests.filter(request => request.command === 'save_backup_settings');
      assert.equal(saves.length, 1, 'configured channel changes use the real form and settings store');
      assert.equal(saves[0].args.payload.backup_type, channel);
    }
    const state = await snapshot();
    assert.equal(state.channel, channel);
    assert.deepEqual(state.restores, []);
    assertSources(state, channel, 0);
    assertRecovered(state);
  };
  const openSelection = async (channel, encrypted = false) => {
    if (channel === 'local') await call('setLocalSelection', encrypted ? localEncryptedPath : localPlainPath);
    await call('clickPage', restoreButton);
    if (channel !== 'local') {
      await waitState('state.remoteFiles.length === 2');
      await call('selectRemote', encrypted ? encryptedFilename : plainFilename);
    }
    await waitState('state.confirmOpen && state.dialogs === 1 && !state.restoreLoading');
    assert.equal((await snapshot()).option, false, 'a new page selection resets the restore option');
  };
  const confirm = () => call('clickDialog', confirmationTitle, 'common.confirm');
  const assertPending = async expected => {
    await waitState('state.restores.length === ' + expected.length + ' && state.restoreLoading && state.dialogs === 0');
    const state = await snapshot();
    assert.equal(state.canRestore, false);
    assert.deepEqual(state.restores, expected, 'the real service wrappers preserve route, selection and options');
    assertSafe(state);
  };
  const waitForError = async expectedMessage => {
    await waitState('!state.restoreLoading && state.canRestore && state.dialogs === 0'
      + ' && state.errors.includes(' + JSON.stringify(expectedMessage) + ')');
    const state = await snapshot();
    assert.deepEqual(state.errors, [expectedMessage], 'the real visible error message contains the expected detail');
    assertRecovered(state);
    return state;
  };

  for (const [channel, configuration, warningKey] of [
    ['webdav', 'webdav-url-missing', 'settings.backupSettings.noWebDAVConfigured'],
    ['repository', 'repository-owner-missing', 'settings.backupSettings.noRepositoryConfigured'],
    ['repository', 'repository-name-missing', 'settings.backupSettings.noRepositoryConfigured'],
  ]) {
    await begin(channel, configuration);
    const initial = await snapshot();
    if (channel === 'repository') {
      assert.equal(initial.repository.owner, configuration === 'repository-owner-missing' ? '' : 'fixture-owner');
      assert.equal(initial.repository.repository, configuration === 'repository-name-missing' ? '' : 'fixture-repository');
    }
    const warning = await call('text', warningKey);
    // Repeated user clicks must remain harmless while the configuration is missing.
    for (let click = 0; click < 2; click++) {
      await call('clickPage', restoreButton);
      await waitState('state.warnings.includes(' + JSON.stringify(warning) + ') && state.canRestore');
      const state = await snapshot();
      assertRecovered(state);
      assert.deepEqual(state.requests, initial.requests, 'unconfigured remote clicks invoke no additional IPC');
      assertSources(state, channel, 0);
      assert.deepEqual(state.restores, [], 'unconfigured remotes invoke no restore service');
      assert.deepEqual(state.errors, [], 'missing configuration is a warning, not a restore error');
    }
    checks.push({ name: configuration + ': warning with zero picker, list or restore calls', actual: await snapshot() });
  }

  await begin('local');
  await call('holdNextLocalSelection');
  await call('clickPage', restoreButton);
  await waitState('state.localSelectionPending && state.restoreLoading');
  const pickerPending = await snapshot();
  assert.equal(pickerPending.canRestore, false);
  assert.equal(pickerPending.dialogs, 0);
  assert.deepEqual(pickerPending.restores, []);
  assertSources(pickerPending, 'local', 1);
  await call('rejectLocalSelection');
  const pickerFailed = await waitForError(await call('text', 'settings.backupSettings.restoreFailed'));
  assert.deepEqual(pickerFailed.restores, [], 'picker rejection never reaches a restore service');
  assertSources(pickerFailed, 'local', 1);
  await openSelection('local');
  const pickerRetried = await snapshot();
  assertSources(pickerRetried, 'local', 2);
  assert.deepEqual(pickerRetried.restores, [], 'retry opens a fresh confirmation before any restore');
  await call('clickDialog', confirmationTitle, 'common.cancel');
  await waitState('state.dialogs === 0');
  const pickerCancelled = await snapshot();
  assertRecovered(pickerCancelled);
  assert.deepEqual(pickerCancelled.restores, []);
  assertSources(pickerCancelled, 'local', 2);
  checks.push({ name: 'local: rejected picker releases loading and permits a fresh selection',
    pending: pickerPending, failed: pickerFailed, actual: pickerCancelled });

  for (const channel of ['local', 'webdav', 'repository']) {
    await begin(channel);
    await openSelection(channel, true);
    await call('toggleOption');
    await waitState('state.option === true');
    await confirm();
    await assertPending([expectedRestore(channel, true, true)]);
    await call('settleRestore', 0, 'failure');
    const expectedMessage = await call('text', 'settings.backupSettings.restoreFailed') + ': fixture ordinary restore failure';
    const failed = await waitForError(expectedMessage);
    assert.deepEqual(failed.restores, [expectedRestore(channel, true, true, true)]);
    assertSources(failed, channel, 1);

    // Retry through the page button and select another file. No flow callback
    // is called directly, and neither the failed file nor its checked option
    // may leak into this independent, explicitly confirmed attempt.
    await openSelection(channel);
    const retryConfirmation = await snapshot();
    assert.deepEqual(retryConfirmation.restores, failed.restores, 'failure never triggers an automatic restore retry');
    assertSources(retryConfirmation, channel, 2);
    await confirm();
    await assertPending([
      expectedRestore(channel, true, true, true), expectedRestore(channel, false, false),
    ]);
    await call('settleRestore', 1);
    await waitState('state.successOpen && state.dialogs === 1 && !state.restoreLoading');
    const recovered = await snapshot();
    assert.deepEqual(recovered.restores, [
      expectedRestore(channel, true, true, true), expectedRestore(channel, false, false, true),
    ]);
    assert.equal(recovered.passwordOpen, false);
    assert.equal(recovered.confirmOpen, false);
    assertSources(recovered, channel, 2);
    assertSafe(recovered);
    // Keep the real success modal open. Restart is forbidden by the IPC double;
    // the next scenario uses a fresh document, never the stale post-restore UI.
    checks.push({ name: channel + ': ordinary restore error is visible and a fresh page retry succeeds',
      failed, actual: recovered });
  }

  assert.equal(checks.length, 7, 'three configuration guards, one picker failure and three restore retry scenarios');
  console.log('Executed 7 real GeneralSettingsPage restore error recovery scenarios (synthetic IPC only)');
  return checks;
}
