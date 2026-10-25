import { mockIPC } from '@tauri-apps/api/mocks';

// Installed before importing any application module. All data is synthetic.
window.__TAURI_OS_PLUGIN_INTERNALS__ = { platform: 'linux' };
const allowedScenarios = new Set(['readback', 'cancel', 'save', 'pending', 'retry', 'remount']);
const params = new URLSearchParams(location.search);
const scenario = params.get('scenario') || 'readback';
if (!allowedScenarios.has(scenario)) throw new Error('Unknown Grok fixture scenario');
const caseToken = params.get('caseToken') || scenario;
const calls = [], writes = [], forbidden = [], listeners = new Map();
let fetchCount = 0, saveCount = 0;
let pendingSave = null;
const clone = value => structuredClone(value);
const baseUrl = 'https://grok-fixture.example.invalid/v1';
const existing = {
  key: 'grok-4.5', model: 'grok-4.5', displayName: 'Fixture hand-edited Grok',
  contextWindow: 123456, reasoningEfforts: ['low', 'high'], reasoningEffort: 'low',
  supportsReasoningEffort: true, supportsBackendSearch: false, baseUrl, apiBackend: 'chat_completions',
};
let providers = [{
  id: 'fixture-grok', name: 'Synthetic Grok Fixture', category: 'custom',
  settingsConfig: JSON.stringify({
    auth: {}, baseUrl, config: '',
    ...(scenario !== 'new' ? { defaultModelKey: 'grok-4.5' } : {}),
    modelCatalog: { models: [existing, { key: 'synthetic-untouched', model: 'synthetic-untouched', displayName: 'Untouched model', contextWindow: 64000 }] },
  }),
  meta: { apiFormat: 'openai_chat' }, isApplied: false, isDisabled: false,
  createdAt: '2026-01-01T00:00:00Z', updatedAt: '2026-01-01T00:00:00Z',
}];
const status = {
  cli_key: 'grok', state: 'direct', dot: 'gray', can_takeover: false,
  can_restore_direct: false, gateway_origin: null, runtime_root: '/fixture/grok',
  managed_targets: [], mode: null, primary_provider_id: null,
  provider_priorities: [], message: null,
};
function deny(label) {
  forbidden.push(label);
  throw new Error('Forbidden Grok fixture operation: ' + label);
}
const policy = document.createElement('meta');
policy.httpEquiv = 'Content-Security-Policy';
policy.content = "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; font-src 'self' data:; object-src 'none'; base-uri 'none'; form-action 'none'";
document.head.append(policy);
document.addEventListener('securitypolicyviolation', event => { forbidden.push('csp:' + event.violatedDirective); });
window.open = () => deny('window.open');
const noArgsCommands = new Set([
  'get_grok_config_file_path', 'get_grok_root_path_info', 'list_grok_providers',
  'list_opencode_favorite_providers', 'has_all_api_hub_extension',
  'get_provider_list_state', 'list_grok_prompt_configs', 'refresh_tray_menu',
]);
mockIPC(async (command, args) => {
  calls.push(command);
  const exactKeys = keys => args && typeof args === 'object' && !Array.isArray(args)
    && JSON.stringify(Object.keys(args).sort()) === JSON.stringify([...keys].sort());
  if (noArgsCommands.has(command) && args !== undefined && !(args && typeof args === 'object' && !Array.isArray(args) && Object.keys(args).length === 0)) return deny(command + ':args');
  if (command === 'plugin:event|listen') {
    if (!exactKeys(['event', 'target', 'handler']) || args.event !== 'gateway-running-changed'
      || args.target?.kind !== 'Any' || Object.keys(args.target).length !== 1 || !Number.isInteger(args.handler)) return deny(command);
    listeners.set(args.handler, args.event);
    return args.handler;
  }
  if (command === 'plugin:event|unlisten') {
    if (!exactKeys(['event', 'eventId']) || listeners.get(args.eventId) !== args.event) return deny(command);
    listeners.delete(args.eventId);
    return null;
  }
  switch (command) {
    case 'get_grok_config_file_path': return '/fixture/grok/config.toml';
    case 'get_grok_root_path_info': return { path: '/fixture/grok', source: 'default' };
    case 'list_grok_providers': return clone(providers);
    case 'list_opencode_favorite_providers':
    case 'list_grok_prompt_configs': return [];
    case 'has_all_api_hub_extension': return false;
    case 'get_provider_list_state': return { sort_modes: {}, last_used: {}, view_modes: { grok: 'list' } };
    case 'proxy_gateway_cli_status':
      if (!exactKeys(['cliKey']) || args.cliKey !== 'grok') return deny(command + ':args');
      return clone(status);
    case 'list_grok_official_accounts':
      if (!exactKeys(['providerId']) || args.providerId !== 'fixture-grok') return deny(command + ':args');
      return [];
    case 'fetch_provider_models': return deny(command);
    case 'update_grok_provider': {
      if (!exactKeys(['provider']) || args.provider?.id !== 'fixture-grok' || args.provider.category !== 'custom'
        || args.provider.isApplied !== false || typeof args.provider.settingsConfig !== 'string') return deny(command + ':args');
      const settings = JSON.parse(args.provider.settingsConfig);
      if (settings.baseUrl !== baseUrl || Object.keys(settings.auth || {}).length !== 0) return deny(command + ':settings');
      // Only settings JSON is exposed in writes; never arbitrary provider credentials.
      writes.push(clone(settings));
      saveCount++;
      if (scenario === 'retry' && saveCount === 1) throw new Error('Synthetic Grok save failure');
      if (scenario === 'pending') await new Promise(resolve => {
        if (pendingSave) return deny('duplicate-pending-save');
        pendingSave = resolve;
      });
      providers = [clone(args.provider)];
      return clone(providers[0]);
    }
    case 'refresh_tray_menu': return null;
    default: return deny(command);
  }
});
export const boundary = {
  releaseSave() {
    if (!pendingSave) throw new Error('No pending synthetic save');
    const resolve = pendingSave; pendingSave = null; resolve();
  },
  snapshot: () => ({ caseToken, scenario, providers: clone(providers), writes: clone(writes),
    forbidden: [...forbidden], fetchCount, saveCount, calls: [...calls], listeners: listeners.size }),
};
