import assert from 'node:assert/strict';

// Expectations deliberately live outside the browser/service double. Sharing a
// fixture payload builder here could make a wrong route and its oracle agree.
const localPlainPath = 'C:/Fixture/Selected/ai-toolbox-backup-20261019-120000.zip';
const localEncryptedPath = 'C:/Fixture/Selected/ai-toolbox-backup-20261019-120001.zip.enc';
const plainFilename = 'ai-toolbox-backup-20261019-120000-abcdef01_RouteHost.zip';
const encryptedFilename = 'ai-toolbox-backup-20261019-120001-abcdef02_RouteHost.zip.enc';
const webdavConnection = {
  url: 'https://restore-routes.example.invalid/dav', username: 'fixture-route-user',
  password: 'fixture-webdav-password', remotePath: '/fixture-backups',
};
const confirmationTitle = 'settings.backupSettings.confirmRestore';
const passwordTitle = 'settings.backupSettings.encryption.passwordPromptTitle';
const restoreButton = 'settings.backupSettings.restoreBackup';
const sourceCommands = ['plugin:dialog|open', 'list_webdav_backups', 'list_repository_backups'];

const expectedRestore = (channel, encrypted, skipCliCustomRoots, restorePassword = null, settled = false) => {
  const options = { skipCliCustomRoots, restorePassword };
  if (channel === 'local') {
    return {
      command: 'restore_database',
      args: { zipFilePath: encrypted ? localEncryptedPath : localPlainPath, ...options }, settled,
    };
  }
  if (channel === 'webdav') {
    return {
      command: 'restore_from_webdav',
      args: { ...webdavConnection, filename: encrypted ? encryptedFilename : plainFilename, ...options }, settled,
    };
  }
  return {
    command: 'restore_from_repository',
    args: { filename: encrypted ? encryptedFilename : plainFilename,
      sha: (encrypted ? '2' : '1').repeat(40), ...options }, settled,
  };
};

export async function verifySettingsRestoreRoutes({ send, evaluate, baseUrl }) {
  const checks = [];
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  const call = (method, ...args) => evaluate('settingsRestoreFixture.' + method
    + '(' + args.map(JSON.stringify).join(',') + ')');
  const snapshot = () => call('snapshot');
  const waitFor = async expression => {
    for (let attempt = 0; attempt < 200; attempt++) {
      if (await evaluate(expression)) return;
      await delay(50);
    }
    throw new Error('Timed out waiting for: ' + expression);
  };
  const waitState = expression => waitFor('window.settingsRestoreFixture && (() => {'
    + 'const state = settingsRestoreFixture.snapshot(); return ' + expression + '; })()');
  const closed = () => waitState('state.dialogs === 0');
  const confirm = () => call('clickDialog', confirmationTitle, 'common.confirm');
  const cancelConfirmation = () => call('clickDialog', confirmationTitle, 'common.cancel');
  const passwordConfirm = () => call('clickDialog', passwordTitle, 'common.confirm');

  const assertSafe = state => {
    assert.deepEqual(state.unexpected, [], 'no unexpected native operation, restart or credential draft');
    assert.deepEqual(state.subscriptions.toSorted(), [
      'auto-backup-completed', 'auto-backup-failed', 'update-download-progress',
    ], 'the page mounted with only its known platform event subscriptions');
  };
  const assertSources = (state, channel, openings = 1) => {
    const sources = state.requests.filter(request => sourceCommands.includes(request.command));
    assert.equal(sources.length, openings, 'exactly one selection/list request per opened picker, never on retry');
    const command = channel === 'local' ? 'plugin:dialog|open'
      : channel === 'webdav' ? 'list_webdav_backups' : 'list_repository_backups';
    for (const source of sources) {
      assert.equal(source.command, command, 'only the chosen channel loads a selection');
      if (channel === 'local') {
        assert.equal(source.args.options.multiple, false);
        assert.notEqual(source.args.options.directory, true);
        assert.deepEqual(source.args.options.filters, [{ name: 'Backup Files', extensions: ['zip', 'enc'] }],
          'the production picker accepts both plaintext and encrypted backups');
      } else {
        assert.deepEqual(source.args, channel === 'webdav' ? webdavConnection : null,
          'list request preserves complete WebDAV parameters or repository no-argument contract');
      }
    }
  };
  const record = async (name, channel, openings = 1) => {
    const result = await snapshot();
    assertSafe(result);
    assertSources(result, channel, openings);
    checks.push({ name, actual: result });
  };
  const assertNoRestore = async () => {
    const state = await snapshot();
    assert.deepEqual(state.restores, [], 'selection, option clicks and cancellation make zero restore calls');
    assert.equal(state.successOpen, false);
    assertSafe(state);
  };

  const begin = async (channel, dark = false) => {
    // A new document resets both real stores and synthetic IPC observations.
    // The same mounted page is retained while changing channel through its UI.
    await send('Page.navigate', { url: baseUrl + '/?' + new URLSearchParams({
      theme: dark ? 'dark' : 'light', language: dark ? 'zh-CN' : 'en-US',
    }) });
    await waitState('state.mounted && state.subscriptions.length === 3'
      + ' && state.requests.some(request => request.command === "get_app_data_dir_info")'
      + ' && state.requests.some(request => request.command === "get_backup_repository_settings")');
    assert.equal((await snapshot()).channel, 'local', 'each document starts in the real local store state');
    await call('clickPage', 'common.edit');
    await waitState('state.canSave');
    await call('selectChannel', channel);
    await call('clickDialog', 'settings.backupSettings.title', 'common.save');
    await waitState('state.channel === ' + JSON.stringify(channel) + ' && state.dialogs === 0');
    const state = await snapshot();
    const saves = state.requests.filter(request => request.command === 'save_backup_settings');
    assert.equal(saves.length, 1, 'channel selection goes through the real form and store save once');
    assert.equal(saves[0].args.payload.backup_type, channel);
    assert.deepEqual(saves[0].args.payload.webdav, {
      url: webdavConnection.url, username: webdavConnection.username, password: webdavConnection.password,
      remote_path: webdavConnection.remotePath, host_label: 'RouteHost',
    });
    assert.deepEqual(saves[0].args.payload.repository, {
      platform: 'github', owner: 'fixture-owner', repository: 'fixture-repository',
      branch: 'fixture-backups', directory: 'fixture-directory',
    });
    assert.equal(saves[0].args.payload.repository_token, undefined);
    assert.equal(saves[0].args.payload.encryption_password, undefined);
    assert.deepEqual(state.requests.filter(request => sourceCommands.includes(request.command)), [],
      'editing channel never starts a picker or remote list');
    await assertNoRestore();
  };

  const openSelection = async (channel, encrypted = false) => {
    if (channel === 'local') await call('setLocalSelection', encrypted ? localEncryptedPath : localPlainPath);
    await call('clickPage', restoreButton);
    if (channel !== 'local') {
      await waitState('state.remoteFiles.length === 2');
      assert.deepEqual((await snapshot()).remoteFiles.toSorted(), [plainFilename, encryptedFilename].toSorted());
      await assertNoRestore();
      await call('selectRemote', encrypted ? encryptedFilename : plainFilename);
    }
    await waitState('state.confirmOpen && state.dialogs === 1 && !state.restoreLoading');
    assert.equal((await snapshot()).option, false, 'a page selection creates a fresh unchecked confirmation');
    await assertNoRestore();
  };
  const checkOption = async () => {
    await call('toggleOption');
    await waitState('state.option === true');
    await assertNoRestore();
  };
  const assertPending = async expected => {
    await waitState('state.restores.length === ' + expected.length + ' && state.restoreLoading && state.dialogs === 0');
    assert.deepEqual((await snapshot()).restores, expected,
      'real page and service adapters send exact route/option/password payloads while pending');
  };
  const requirePassword = async (index, outcome) => {
    await call('settleRestore', index, outcome);
    await waitState('state.passwordOpen && state.dialogs === 1 && !state.restoreLoading');
    const state = await snapshot();
    assert.equal(state.password, '', 'each retry prompt has a fresh input');
    assert.equal(state.confirmOpen, false, 'password prompt does not recreate the selection confirmation');
    assert.equal(state.restores.length, index + 1, 'opening a password prompt does not retry');
    assert.equal(state.wrongPassword, outcome === 'passwordWrong'
      ? await call('text', 'settings.backupSettings.encryption.errors.passwordWrong') : null);
  };
  const inputPassword = async value => {
    await call('focusPassword');
    await send('Input.insertText', { text: value });
    await waitState('state.password === ' + JSON.stringify(value));
  };
  const finish = async (index, expected) => {
    await call('settleRestore', index);
    await waitState('state.successOpen && state.dialogs === 1 && !state.restoreLoading');
    const state = await snapshot();
    assert.deepEqual(state.restores, expected);
    assertSafe(state);
    // The real forced-restart success modal remains open. Never click restart;
    // the next scenario navigates to a fresh fixture document instead.
  };

  for (const channel of ['local', 'webdav', 'repository']) {
    await begin(channel);
    if (channel === 'local') await call('setLocalSelection', null);
    await call('clickPage', restoreButton);
    if (channel === 'local') {
      await waitState('state.requests.some(request => request.command === "plugin:dialog|open") && !state.restoreLoading');
    } else {
      await waitState('state.remoteFiles.length === 2');
      await call('closeRemote');
    }
    await closed();
    await assertNoRestore();
    await record(channel + ': cancel file selection invokes zero restore services', channel);

    await begin(channel);
    await openSelection(channel, true);
    await checkOption();
    await cancelConfirmation();
    await closed();
    await assertNoRestore();
    await record(channel + ': cancel checked confirmation invokes zero restore services', channel);

    await begin(channel);
    await openSelection(channel);
    await confirm();
    await assertPending([expectedRestore(channel, false, false)]);
    await finish(0, [expectedRestore(channel, false, false, null, true)]);
    await record(channel + ': plaintext selection routes exact parameters with default false', channel);

    await begin(channel, true);
    await openSelection(channel, true);
    await checkOption();
    // Future picker/list results change before confirmation. The page's
    // original file/connection closure must survive both password retries.
    await call('replaceSelectionSources');
    await confirm();
    await assertPending([expectedRestore(channel, true, true)]);
    await requirePassword(0, 'passwordRequired');
    await inputPassword('fixture-route-wrong');
    await passwordConfirm();
    await assertPending([
      expectedRestore(channel, true, true, null, true),
      expectedRestore(channel, true, true, 'fixture-route-wrong'),
    ]);
    await requirePassword(1, 'passwordWrong');
    await inputPassword('fixture-route-correct');
    await passwordConfirm();
    await assertPending([
      expectedRestore(channel, true, true, null, true),
      expectedRestore(channel, true, true, 'fixture-route-wrong', true),
      expectedRestore(channel, true, true, 'fixture-route-correct'),
    ]);
    await finish(2, [null, 'fixture-route-wrong', 'fixture-route-correct'].map(password =>
      expectedRestore(channel, true, true, password, true)));
    await record(channel + ': checked option keeps selected path/filename/SHA across two password retries', channel);
  }

  assert.equal(checks.length, 12, 'all three channels must execute all four route scenarios');
  console.log('Executed 12 real GeneralSettingsPage restore route scenarios (synthetic IPC only)');
  return checks;
}
