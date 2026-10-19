// All paths, connection details and passwords in this fixture are synthetic.
// Production backupApi/settingsApi/appApi still construct the IPC payloads.
// This module is the only service/platform double; it never calls a backend.
export const fixtureData = {
  localDirectory: 'C:/Fixture/Backups',
  localPlainPath: 'C:/Fixture/Selected/ai-toolbox-backup-20261019-120000.zip',
  localEncryptedPath: 'C:/Fixture/Selected/ai-toolbox-backup-20261019-120001.zip.enc',
  webdav: {
    url: 'https://restore-routes.example.invalid/dav',
    username: 'fixture-route-user',
    password: 'fixture-webdav-password',
    remotePath: '/fixture-backups',
    hostLabel: 'RouteHost',
  },
  repository: {
    platform: 'github', owner: 'fixture-owner', repository: 'fixture-repository',
    branch: 'fixture-backups', directory: 'fixture-directory',
  },
};

const makeFiles = channel => [
  {
    filename: 'ai-toolbox-backup-20261019-120000-abcdef01_RouteHost.zip',
    size: 1024, encrypted: false,
    ...(channel === 'repository' ? { sha: '1'.repeat(40) } : {}),
  },
  {
    filename: 'ai-toolbox-backup-20261019-120001-abcdef02_RouteHost.zip.enc',
    size: 2048, encrypted: true,
    ...(channel === 'repository' ? { sha: '2'.repeat(40) } : {}),
  },
];

const state = {
  requests: [],
  restores: [],
  subscriptions: [],
  unexpected: [],
  localSelection: fixtureData.localPlainPath,
  repository: structuredClone(fixtureData.repository),
  files: { webdav: makeFiles('webdav'), repository: makeFiles('repository') },
};
const pendingRestores = [];
let deferNextLocalSelection = false;
let pendingLocalSelection = null;
const restoreCommands = new Set(['restore_database', 'restore_from_webdav', 'restore_from_repository']);
const allowedEvents = new Set(['update-download-progress', 'auto-backup-completed', 'auto-backup-failed']);
const clone = value => structuredClone(value ?? null);
const repositoryView = () => ({ config: clone(state.repository), has_token: true });
const encryptionStatus = () => ({ enabled: false, has_password: false, password_known: true });

const unexpected = label => {
  state.unexpected.push(label);
  throw new Error('Unexpected settings restore fixture operation: ' + label);
};

export const platform = () => 'windows';
export const locale = async () => 'en-US';
export const listen = async event => {
  if (!allowedEvents.has(event)) return unexpected('event:' + event);
  state.subscriptions.push(event);
  // No operating-system listener is registered and no event is emitted.
  return () => {};
};

export function initialConfiguration(scenario = 'configured') {
  // Incomplete saved settings cannot be created through the validated form.
  // Seed them before mounting, just as the existing fixture seeds local state.
  const webdav = clone(fixtureData.webdav);
  if (scenario === 'configured') return { backupType: 'local', webdav };
  if (scenario === 'webdav-url-missing') {
    webdav.url = '';
    return { backupType: 'webdav', webdav };
  }
  if (scenario === 'repository-owner-missing' || scenario === 'repository-name-missing') {
    state.repository[scenario === 'repository-owner-missing' ? 'owner' : 'repository'] = '';
    return { backupType: 'repository', webdav };
  }
  throw new Error('Unknown settings restore initial configuration: ' + scenario);
}

export function installTauriBoundary() {
  window.__TAURI_INTERNALS__ = {
    invoke: async (command, args) => {
      const request = { command, args: clone(args) };
      state.requests.push(request);
      if (restoreCommands.has(command)) {
        state.restores.push({ ...request, settled: false });
        return new Promise((resolve, reject) => pendingRestores.push({ resolve, reject }));
      }
      if (command === 'get_backup_repository_settings') return repositoryView();
      if (command === 'get_backup_encryption_status') return encryptionStatus();
      if (command === 'list_backup_file_filter_path_options') return [];
      if (command === 'plugin:app|version') return '0.0.0-fixture';
      if (command === 'get_app_data_dir_info') {
        return {
          override: null, effective: 'C:/Fixture/AppData', default: 'C:/Fixture/AppData',
          is_custom: false, next_start: 'C:/Fixture/AppData', restart_required: false,
        };
      }
      if (command === 'plugin:dialog|open') {
        // Only selectBackupFile's real plugin call is allowed. A directory
        // picker or another native interaction must fail rather than succeed.
        if (args?.options?.directory) return unexpected('directory dialog');
        if (deferNextLocalSelection) {
          deferNextLocalSelection = false;
          return new Promise((resolve, reject) => { pendingLocalSelection = { resolve, reject }; });
        }
        return state.localSelection;
      }
      if (command === 'list_webdav_backups') return clone(state.files.webdav);
      if (command === 'list_repository_backups') return clone(state.files.repository);
      if (command === 'save_backup_settings') {
        // In-memory acknowledgement of the real settings form/store payload.
        // No secret is loaded, stored or transmitted; token drafts remain empty.
        if (args.payload.repository_token || args.payload.encryption_password) {
          return unexpected('credential draft save');
        }
        if (args.payload.backup_type === 'repository') state.repository = clone(args.payload.repository);
        return { repository: repositoryView(), encryption: encryptionStatus() };
      }
      // Includes restart_app, deletion, backups, external links and updates.
      // Even if the page catches an error, final assertions detect this record.
      return unexpected(command);
    },
  };
}

export function boundarySnapshot() {
  return clone({
    requests: state.requests, restores: state.restores,
    subscriptions: state.subscriptions, unexpected: state.unexpected,
    localSelectionPending: pendingLocalSelection !== null,
    repository: state.repository,
  });
}

export function holdNextLocalSelection() {
  if (deferNextLocalSelection || pendingLocalSelection) throw new Error('Synthetic picker already held');
  deferNextLocalSelection = true;
}

export function rejectLocalSelection() {
  if (!pendingLocalSelection) throw new Error('No pending synthetic picker');
  const pending = pendingLocalSelection;
  pendingLocalSelection = null;
  pending.reject('fixture native picker unavailable');
}

export function setLocalSelection(path) {
  state.localSelection = path;
}

export function replaceSelectionSources() {
  // Change only future synthetic picker/list responses, never React state or a
  // page handler. A selected file and all password retries must keep their data.
  state.localSelection = 'C:/Fixture/Replacement/replacement.zip.enc';
  state.files = {
    webdav: [{ filename: 'replacement-webdav.zip.enc', size: 4096, encrypted: true }],
    // A later list would return the same name with different content. Retries
    // must stay bound to the SHA returned by the selection's original list.
    repository: [{ filename: 'ai-toolbox-backup-20261019-120001-abcdef02_RouteHost.zip.enc',
      sha: '3'.repeat(40), size: 4096, encrypted: true }],
  };
}

export function settleRestore(index, outcome = 'success') {
  const request = state.restores[index];
  if (!request || request.settled) throw new Error('Missing or settled synthetic restore: ' + index);
  if (!['success', 'passwordRequired', 'passwordWrong', 'failure'].includes(outcome)) {
    throw new Error('Unsupported synthetic restore outcome: ' + outcome);
  }
  request.settled = true;
  if (outcome === 'success') pendingRestores[index].resolve({ warnings: [], willReapplyApplied: true });
  else if (outcome === 'failure') pendingRestores[index].reject('fixture ordinary restore failure');
  else pendingRestores[index].reject(JSON.stringify({ type: outcome }));
}
