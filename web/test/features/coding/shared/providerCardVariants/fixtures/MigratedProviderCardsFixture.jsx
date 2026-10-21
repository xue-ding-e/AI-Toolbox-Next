/**
 * Mounts the four migrated provider mapping layers directly, with no page and
 * synthetic props/callbacks. Import-time platform shims are retained, while
 * every backend invocation fails closed.
 *
 * Why not the real pages: the four pages each call dozens of Tauri commands on
 * mount, and the claims this fixture guards belong to the mapping layers — the
 * layout they pick, the facts they state, and the guards they honour. A page
 * fixture would break the day a page gains an unrelated command.
 *
 * What it guards, per CLI:
 *
 * - Claude Desktop: role bindings render as `Role: code`, a `tierAlias` becomes
 *   a real tag, notes live in the footer *below* the header columns, and a
 *   disabled drag renders no handle.
 * - Gemini CLI: the official card's name row is `🔗 官方账号 (n)`, its second
 *   line is the switching explanation, the account block's rows span the card's
 *   content width, and a virtual row offers save rather than switch/delete
 *   (the existing refresh/details read actions are retained).
 * - Grok: the official row's sixth action (refresh token) is rendered through
 *   the shared section's extra-action slot, and the model toolbar keeps the
 *   documented order.
 * - OmO Native: the grid cell states the npm package (plus the default model
 *   for the provider that owns it), and the default provider's "more" menu
 *   greys out delete.
 */
import './ProviderCardSpacingStubs.js';
import React from 'react';
import { createRoot } from 'react-dom/client';
import { MemoryRouter } from 'react-router-dom';
import { ConfigProvider, theme } from 'antd';
import i18n from '@/i18n';
import { useAppStore } from '@/stores/appStore';
import { updateGatewayProviderProfiles } from '@/features/coding/shared/gateway/providerProfiles';
import ClaudeDesktopProviderCard from '@/features/coding/claudedesktop/components/ClaudeDesktopProviderCard';
import GeminiCliProviderCard from '@/features/coding/geminicli/components/GeminiCliProviderCard';
import GrokProviderCard from '@/features/coding/grok/components/GrokProviderCard';
import OmoNativeProviderCard from '@/features/coding/omo_native/components/OmoNativeProviderCard';
import OpenClawProviderCard from '@/features/coding/openclaw/components/OpenClawProviderCard';
import HermesProviderCard from '@/features/coding/hermes/components/HermesProviderCard';
import gatewayProfiles from '../../../../../../../tauri/resources/gateway_provider_profiles.json';
import '@/App.css';

const parameters = new URLSearchParams(location.search);
const desktopGatewayCase = parameters.get('desktopGateway');
const language = parameters.get('language') || 'zh-CN';
const themeMode = parameters.get('theme') || 'light';
const resolvedTheme = themeMode === 'system'
  ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
  : themeMode;
await i18n.changeLanguage(language);
useAppStore.setState({ language });
document.documentElement.dataset.theme = resolvedTheme;
updateGatewayProviderProfiles(gatewayProfiles);

const noop = () => {};
const state = { runId: parameters.get('runId'), callbacks: [], unexpectedIPC: [], antdContainer: null };
window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    state.unexpectedIPC.push({ command, args });
    throw new Error('Unexpected migrated-card fixture IPC: ' + command);
  },
};
const recordDelete = (fixture, providerId) => state.callbacks.push({ action: 'delete', fixture, providerId });

/* ------------------------------------------------------------------ Claude Desktop */

const claudeDesktopProvider = {
  id: 'desktop-a',
  name: 'Desktop Relay',
  category: 'custom',
  settingsConfig: JSON.stringify({
    env: {
      ANTHROPIC_BASE_URL: 'https://desktop.example.test',
      ANTHROPIC_AUTH_TOKEN: 'sk-ant-desktop-fixture-key',
    },
  }),
  meta: {
    claudeDesktopModelRoutes: {
      'claude-sonnet-5': { model: 'claude-sonnet-4-5', tierAlias: 'sonnet' },
      'claude-opus-5': { model: 'claude-opus-4-8' },
    },
  },
  notes: '团队共享额度',
  isApplied: true,
  isDisabled: false,
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};

const claudeDesktopProps = {
  provider: claudeDesktopProvider,
  isApplied: true,
  onEdit: noop,
  onDelete: noop,
  onCopy: noop,
  onShare: noop,
  onTest: noop,
  onSelect: noop,
  onToggleDisabled: noop,
  dragDisabled: true,
  gatewayStatus: null,
  gatewayTakeoverActive: false,
};

// Separate navigation keeps the existing migration matrix and its geometry
// untouched. These are input props, not replicas of the card's proxy decision.
function ClaudeDesktopGatewayFixture() {
  const viewMode = parameters.get('view');
  if (!['identity', 'takeover', 'mapped', 'stopped'].includes(desktopGatewayCase)
    || !['list', 'card'].includes(viewMode)) {
    throw new Error('Invalid Claude Desktop gateway fixture scenario');
  }
  const isApplied = desktopGatewayCase === 'takeover';
  const provider = {
    ...claudeDesktopProvider,
    id: 'desktop-gateway-' + desktopGatewayCase + '-' + viewMode,
    name: 'Synthetic Desktop Gateway',
    settingsConfig: JSON.stringify({ env: {
      ANTHROPIC_BASE_URL: 'https://desktop-gateway.example.test',
      ANTHROPIC_AUTH_TOKEN: 'synthetic-not-a-real-credential',
    } }),
    meta: {
      apiFormat: 'anthropic',
      claudeDesktopModelRoutes: {
        // Stopped is a legacy non-Claude control, not safe-remap coverage.
        'claude-sonnet-5': { model: desktopGatewayCase === 'identity' ? 'claude-sonnet-5'
          : desktopGatewayCase === 'stopped' ? 'deepseek-chat' : 'claude-opus-5' },
      },
    },
    notes: '',
    isApplied,
  };
  const gatewayStatus = {
    cli_key: 'claude_desktop',
    state: isApplied ? 'takeover_applied' : desktopGatewayCase === 'stopped' ? 'gateway_stopped' : 'direct',
    dot: isApplied ? 'green' : 'gray',
    can_takeover: !isApplied && desktopGatewayCase !== 'stopped',
    can_restore_direct: isApplied,
    gateway_origin: null,
    runtime_root: null,
    managed_targets: [],
    mode: isApplied ? 'single' : null,
    primary_provider_id: isApplied ? provider.id : null,
    provider_priorities: [],
    message: null,
  };
  return (
    <div data-fixture="desktop-gateway" style={{ padding: 16 }}>
      <ClaudeDesktopProviderCard {...claudeDesktopProps} provider={provider} viewMode={viewMode}
        isApplied={isApplied} gatewayStatus={gatewayStatus} gatewayTakeoverActive={isApplied}
        onSelect={selected => state.callbacks.push({ action: 'direct', viewMode, providerId: selected.id })}
      />
    </div>
  );
}

/* ------------------------------------------------------------------ Gemini CLI */

const geminiOfficialProvider = {
  id: 'gemini-official',
  name: 'Gemini Official',
  category: 'official',
  settingsConfig: JSON.stringify({ env: {} }),
  isApplied: true,
  isDisabled: false,
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};

const geminiOfficialAccounts = [
  {
    id: '__local__',
    providerId: 'gemini-official',
    name: 'Local login',
    kind: 'local',
    email: 'local@example.test',
    isApplied: false,
    isVirtual: true,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
  },
  {
    id: 'account-oauth',
    providerId: 'gemini-official',
    name: 'Work login',
    kind: 'oauth',
    email: 'work@example.test',
    projectId: 'proj-123',
    planType: 'Pro',
    limit5hText: '87%',
    isApplied: true,
    isVirtual: false,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
  },
];

const geminiProps = {
  provider: geminiOfficialProvider,
  isApplied: true,
  onEdit: noop,
  onDelete: noop,
  onCopy: noop,
  onShare: noop,
  onSelect: noop,
  onToggleDisabled: noop,
  officialAccounts: geminiOfficialAccounts,
  onOfficialAccountLogin: noop,
  onOfficialLocalAccountSave: noop,
  onOfficialAccountApply: noop,
  onOfficialAccountDelete: noop,
  onOfficialAccountRefresh: noop,
  onOfficialAccountViewDetails: noop,
  gatewayStatus: null,
  gatewayTakeoverActive: false,
};

/* ------------------------------------------------------------------ Grok */

const grokOfficialProvider = {
  id: 'grok-official',
  name: 'Grok Official',
  category: 'official',
  settingsConfig: JSON.stringify({ auth: {}, config: {}, modelCatalog: { models: [] } }),
  isApplied: false,
  isDisabled: false,
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};

const grokOfficialAccounts = [
  {
    id: 'grok-account-oauth',
    providerId: 'grok-official',
    name: 'Grok login',
    kind: 'oauth',
    email: 'grok@example.test',
    planType: 'SuperGrok',
    limitWeeklyText: '89%',
    isApplied: true,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
  },
];

// A second synthetic account catches loading accidentally applied to every row.
grokOfficialAccounts.push({
  ...grokOfficialAccounts[0],
  id: 'grok-account-other',
  name: 'Other Grok login',
  email: 'other-grok@example.test',
  isApplied: false,
});

const grokCustomProvider = {
  id: 'grok-custom',
  name: 'Grok Relay',
  category: 'custom',
  settingsConfig: JSON.stringify({
    auth: { API_KEY: 'sk-grok-fixture-key-0123456789' },
    // Channel-level endpoint: `extractGrokSettingsBaseUrl` reads this before
    // falling back to the model catalog / the `config` TOML text.
    baseUrl: 'https://grok.example.test/v1',
    config: { model: 'grok-4' },
    defaultModelKey: 'relay-model',
    modelCatalog: {
      models: [
        { key: 'relay-model', model: 'grok-4', displayName: 'Grok 4', reasoningEffort: 'high' },
        { key: 'relay-mini', model: 'grok-4-mini', displayName: 'Grok 4 Mini' },
      ],
    },
  }),
  isApplied: false,
  isDisabled: false,
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};

const grokCommonProps = {
  onEdit: noop,
  onDelete: noop,
  onCopy: noop,
  onShare: noop,
  onTest: noop,
  onSelect: noop,
  onToggleDisabled: noop,
  onAddModel: noop,
  onEditModel: noop,
  onDeleteModel: noop,
  onSetDefaultModel: noop,
  onFetchModels: noop,
  onToggleBatchDeleteMode: noop,
  onBatchDeleteModels: noop,
  onOfficialAccountLogin: noop,
  onOfficialLocalAccountSave: noop,
  onOfficialAccountApply: noop,
  onOfficialAccountDelete: noop,
  onOfficialAccountRefresh: noop,
  onOfficialAccountRefreshToken: noop,
  onOfficialAccountViewDetails: noop,
  gatewayStatus: null,
  gatewayTakeoverActive: false,
};

/* ------------------------------------------------------------------ OmO Native */

const omoProviders = [
  {
    key: 'omo-default',
    builtin: false,
    oauth: false,
    custom: true,
    hasKey: true,
    apiKey: 'sk-omo-default',
    config: {
      name: 'OmO Default',
      baseUrl: 'https://omo.example.test/v1',
      api: 'openai-completions',
      models: [{ id: 'model-default', name: 'Default Model' }],
    },
  },
  {
    key: 'omo-other',
    builtin: false,
    oauth: false,
    custom: true,
    hasKey: true,
    apiKey: 'sk-omo-other',
    config: {
      name: 'OmO Other',
      baseUrl: 'https://omo-other.example.test/v1',
      api: 'anthropic-messages',
      models: [{ id: 'model-other', name: 'Other Model' }],
    },
  },
];

const omoProps = {
  onEdit: noop,
  onCopy: noop,
  onDelete: noop,
  selectable: false,
  selected: false,
  onSelectChange: noop,
  onAddModel: noop,
  onEditModel: noop,
  onCopyModel: noop,
  onDeleteModel: noop,
  onReorderModels: noop,
  onTestModels: noop,
  onFetchModels: noop,
  onToggleModelSelection: noop,
  onToggleBatchDeleteMode: noop,
  onBatchDeleteModels: noop,
};

/* ------------------------------------------------------------------ OpenClaw */

const openClawProvider = {
  baseUrl: 'https://relay.openclaw.example.test/v1',
  apiKey: 'sk-openclaw-fixture-key',
  api: 'openai-completions',
  models: [
    { id: 'gpt-5', name: 'GPT-5', contextWindow: 400000, maxTokens: 128000 },
    { id: 'claude-sonnet-4-5', name: 'Claude Sonnet 4.5' },
  ],
};

const openClawProps = {
  onEdit: noop,
  onCopy: noop,
  onShare: noop,
  onDelete: noop,
  onAddModel: noop,
  onEditModel: noop,
  onCopyModel: noop,
  onDeleteModel: noop,
  onReorderModels: noop,
  onTestModels: noop,
  testModelsDisabled: false,
  testModelsDisabledTooltip: '未配置可测试模型',
  onFetchModels: noop,
  fetchDisabled: false,
  fetchDisabledTooltip: '请先填写 Base URL 与 API Key',
  onSelectChange: noop,
  onToggleModelSelection: noop,
  onToggleBatchDeleteMode: noop,
  onBatchDeleteModels: noop,
};

/* ------------------------------------------------------------------ Hermes */

const hermesProvider = {
  providerKey: 'anthropic-direct',
  displayName: 'Anthropic Direct',
  apiMode: 'anthropic',
  provider: {
    id: 'anthropic-direct',
    api_mode: 'anthropic',
    base_url: 'https://api.anthropic.com',
    api_key: 'sk-ant-hermes-fixture-key',
    models: [
      { id: 'claude-opus-4-8', name: 'Claude Opus 4.8' },
      { id: 'claude-sonnet-4-5', name: 'Claude Sonnet 4.5' },
    ],
  },
  modelIds: ['claude-opus-4-8', 'claude-sonnet-4-5'],
  isBuiltin: false,
  isReadOnly: false,
  isDefault: true,
};

/** A built-in `providers:` entry: no edit, no copy, no delete, no catalog. */
const hermesReadOnlyProvider = {
  providerKey: 'openrouter',
  displayName: 'OpenRouter Built-in',
  apiMode: 'openai',
  provider: { id: 'openrouter', base_url: 'https://openrouter.ai/api/v1' },
  modelIds: [],
  isBuiltin: true,
  isReadOnly: true,
  isDefault: false,
};

const hermesProps = {
  onEdit: noop,
  onCopy: noop,
  onShare: noop,
  onDelete: noop,
  onAddModel: noop,
  onEditModel: noop,
  onCopyModel: noop,
  onDeleteModel: noop,
  onSetPrimaryModel: noop,
  onTestModels: noop,
  testModelsDisabled: false,
  onFetchModels: noop,
  fetchDisabled: false,
  onSelectChange: noop,
  onToggleModelSelection: noop,
  onToggleBatchDeleteMode: noop,
  onBatchDeleteModels: noop,
};

/* ------------------------------------------------------------------ fixture API */

const cards = (name) => [...document.querySelectorAll(`[data-fixture="${name}"]`)];
const desktopGatewayActionKeys = {
  direct: 'claudecode.provider.apply',
  proxy: 'gateway.proxy.applyWithProxyButton',
  restore: 'gateway.proxy.restoreDirectButton',
};
const desktopGatewayButtons = action => {
  const label = i18n.t(desktopGatewayActionKeys[action]).replace(/\s+/g, '');
  return [...(cards('desktop-gateway')[0]?.querySelectorAll('button') ?? [])]
    .filter(button => button.textContent.replace(/\s+/g, '') === label);
};
const desktopGatewayButton = action => {
  const buttons = desktopGatewayButtons(action);
  if (buttons.length !== 1) throw new Error('Expected exactly one real Desktop action: ' + action);
  return buttons[0];
};
const isVisible = node => {
  const rect = node.getBoundingClientRect();
  if (!rect.width || !rect.height || !node.getClientRects().length) return false;
  for (let ancestor = node; ancestor; ancestor = ancestor.parentElement) {
    const style = getComputedStyle(ancestor);
    if (style.display === 'none' || style.visibility !== 'visible' || Number(style.opacity) === 0) return false;
  }
  return true;
};

window.migratedProviderCardsFixture = {
  desktopGatewayActions() {
    return Object.fromEntries(Object.keys(desktopGatewayActionKeys).map(action => [action,
      desktopGatewayButtons(action).map(button => ({ visible: isVisible(button), disabled: button.disabled })),
    ]));
  },
  desktopGatewayTarget(action) {
    const button = desktopGatewayButton(action);
    const rect = button.getBoundingClientRect();
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;
    const hit = document.elementFromPoint(x, y);
    return { x, y, width: rect.width, height: rect.height, visible: isVisible(button),
      inViewport: x > 0 && x < innerWidth && y > 0 && y < innerHeight,
      hit: Boolean(hit && button.contains(hit)), hitTag: hit?.tagName ?? null };
  },
  /** The rendered text of one fixture block, whitespace-normalised. */
  text(name) {
    const node = cards(name)[0];
    if (!node) throw new Error('No fixture block: ' + name);
    return node.textContent.replace(/\s+/g, ' ').trim();
  },

  /** Left/right edges of the first element matching `selector` inside a block. */
  edges(name, selector) {
    const root = cards(name)[0];
    const node = root?.querySelector(selector);
    if (!node) return null;
    const rect = node.getBoundingClientRect();
    return { left: Math.round(rect.left), right: Math.round(rect.right), top: Math.round(rect.top), bottom: Math.round(rect.bottom) };
  },

  /**
   * Expands a collapsed account block (the list card's fold starts closed, as
   * it always has) by clicking its list title.
   */
  async expandAccounts(name) {
    const root = cards(name)[0];
    const titleButton = [...root.querySelectorAll('button')]
      .find(button => button.textContent.includes('账号列表'));
    if (!titleButton) return false;
    titleButton.click();
    await new Promise(resolve => setTimeout(resolve, 300));
    return true;
  },

  /** Whether a block renders a drag handle. */
  hasHandle(name) {
    return Boolean(cards(name)[0]?.querySelector('[data-provider-card-handle]'));
  },

  /**
   * Opens the list card's model fold.
   *
   * The rows exist only after the fold opens: `ModelListSection` keeps the body
   * lazily mounted, so reading a row's state before this clicks returns nothing
   * and would read as "the row says nothing".
   */
  async expandModels(name) {
    const root = cards(name)[0];
    // The fold's header is antd's `role="button"` header div, not a <button>.
    const header = [...root.querySelectorAll('[role="button"], button')]
      .find(node => node.textContent.includes('模型列表'));
    if (!header) return false;
    header.click();
    await new Promise(resolve => setTimeout(resolve, 300));
    return true;
  },

  /** The second line of a compact cell, as written. */
  subtitle(name) {
    const node = cards(name)[0]?.querySelector('[data-provider-subtitle]');
    return node ? node.textContent.replace(/\s+/g, ' ').trim() : null;
  },

  expandModels(name) {
    const header = cards(name)[0]?.querySelector('.ant-collapse-header');
    if (!header) throw new Error('Missing real model-list header: ' + name);
    header.click();
  },

  state,
  resetCallbacks() { state.callbacks.length = 0; },
  surface(name) {
    const root = cards(name)[0];
    const selector = name.endsWith('-list') ? '.ant-card' : '[data-provider-card-clickable]';
    const nodes = root?.querySelectorAll(selector);
    if (nodes?.length !== 1) throw new Error('Expected one actual card surface: ' + name);
    return nodes[0];
  },
  surfaceStyle(name) {
    const surface = this.surface(name);
    const rect = surface.getBoundingClientRect();
    const background = getComputedStyle(surface).backgroundColor;
    const accented = name.startsWith('claude-desktop-') || name.startsWith('gemini-');
    // CardShell uses Antd's container token at rest; compact cards use App.css.
    // Applied cards in either layout use the shared selected-background token.
    if (!state.antdContainer) throw new Error('ConfigProvider tokens are not ready');
    const probe = document.createElement('span');
    probe.style.backgroundColor = accented ? 'var(--color-bg-selected)'
      : name.endsWith('-list') ? state.antdContainer : 'var(--color-bg-container)';
    document.body.append(probe);
    const expectedBackground = getComputedStyle(probe).backgroundColor;
    probe.remove();
    return { background, expectedBackground, hasBox: rect.width > 0 && rect.height > 0,
      actualSurface: surface.matches(name.endsWith('-list') ? '.ant-card' : '[data-provider-card-clickable]') };
  },
  scrollMenuTarget(name) {
    const button = cards(name)[0]?.querySelector('[data-provider-card-actions] button');
    if (!button) throw new Error('Missing real compact menu trigger: ' + name);
    button.scrollIntoView({ block: 'center', behavior: 'instant' });
  },
  menuTarget(name) {
    const button = cards(name)[0]?.querySelector('[data-provider-card-actions] button');
    if (!button) throw new Error('Missing real compact menu trigger: ' + name);
    const rect = button.getBoundingClientRect();
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;
    const hit = document.elementFromPoint(x, y);
    return {
      x, y, width: rect.width, height: rect.height,
      inViewport: x > 0 && x < innerWidth && y > 0 && y < innerHeight,
      hit: Boolean(hit && button.contains(hit)),
      opacity: Number(getComputedStyle(button.closest('[data-provider-card-actions]')).opacity),
      hitTag: hit?.tagName ?? null,
      hitClass: hit?.getAttribute('class') ?? null,
    };
  },
  deleteMenuItem() {
    const menus = [...document.querySelectorAll('.ant-dropdown')].filter(node =>
      node.getClientRects().length && getComputedStyle(node).display !== 'none'
      && !node.classList.contains('ant-dropdown-hidden'));
    if (menus.length !== 1) throw new Error('Expected exactly one visible dropdown');
    const items = [...menus[0].querySelectorAll('[role="menuitem"]')]
      .filter(item => item.textContent.trim() === i18n.t('common.delete'));
    if (items.length !== 1) throw new Error('Expected one actual delete menu item');
    return items[0];
  },
  deleteMenuState() {
    const item = this.deleteMenuItem();
    return { found: true, disabled: item.getAttribute('aria-disabled') === 'true'
      || item.classList.contains('ant-dropdown-menu-item-disabled'), title: item.getAttribute('title') };
  },
  clickMenuDelete() { this.deleteMenuItem().click(); },
  openDetail(name) { this.surface(name).click(); },
  closeDetail() { document.querySelector('[data-provider-detail-close]').click(); },
  clickDetailDelete() {
    const buttons = [...document.querySelectorAll('[data-provider-detail-footer] button')]
      .filter(button => button.textContent.trim() === i18n.t('common.delete'));
    if (buttons.length !== 1) throw new Error('Expected one real detail delete button');
    buttons[0].click();
  },
  answerConfirm(confirm) {
    const modal = document.querySelector('.ant-modal-confirm');
    const label = i18n.t(confirm ? 'common.confirm' : 'common.cancel');
    const button = [...(modal?.querySelectorAll('button') ?? [])]
      .find(node => node.textContent.replace(/\s+/g, '') === label.replace(/\s+/g, ''));
    if (!button) throw new Error('Missing real confirmation button: ' + label);
    button.click();
  },
  accountRoot(viewMode) {
    return viewMode === 'list' ? cards('grok-official-list')[0]
      : document.querySelector('[data-provider-detail-panel]');
  },
  refreshButton(viewMode, accountId, action) {
    const account = grokOfficialAccounts.find(item => item.id === accountId);
    const row = [...this.accountRoot(viewMode).querySelectorAll('[style*="border-bottom"]')]
      .find(node => [...node.querySelectorAll('*')].some(child =>
        !child.children.length && child.textContent.trim() === account.email));
    const label = i18n.t(action === 'refresh' ? 'common.officialAccount.refresh' : 'grok.provider.officialAccountRefreshToken');
    const button = [...(row?.querySelectorAll('button') ?? [])]
      .find(node => node.textContent.trim() === label);
    if (!button) throw new Error('Missing actual account action: ' + accountId + '/' + action);
    return button;
  },
  clickRefresh(viewMode, accountId, action) { this.refreshButton(viewMode, accountId, action).click(); },
  loadingState(viewMode) {
    return grokOfficialAccounts.map(account => ({
      accountId: account.id,
      refresh: this.refreshButton(viewMode, account.id, 'refresh').classList.contains('ant-btn-loading'),
      token: this.refreshButton(viewMode, account.id, 'token').classList.contains('ant-btn-loading'),
    }));
  },
  finishRefresh() { document.querySelector('[data-fixture-finish-refresh]').click(); },
};

function ThemeTokenProbe() {
  const { token } = theme.useToken();
  React.useLayoutEffect(() => { state.antdContainer = token.colorBgContainer; }, [token.colorBgContainer]);
  return null;
}

function FixtureHost() {
  const [pending, setPending] = React.useState(null);
  const refresh = (viewMode, action) => (provider, account) => {
    // Record every call: the fixture must not implement a missing product guard.
    state.callbacks.push({ action, viewMode, providerId: provider.id, accountId: account.id });
    setPending({ viewMode, action, accountId: account.id });
  };
  return (
    <ConfigProvider theme={{ algorithm: resolvedTheme === 'dark' ? theme.darkAlgorithm : theme.defaultAlgorithm }}>
      <ThemeTokenProbe />
      <MemoryRouter>
        {desktopGatewayCase ? <ClaudeDesktopGatewayFixture /> : <>
        <button data-fixture-finish-refresh onClick={() => setPending(null)}>Finish synthetic refresh</button>
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 12 }}>
          {['list', 'card'].map(viewMode => (
            <React.Fragment key={viewMode}>
              <div data-fixture={'claude-desktop-' + viewMode}>
                <ClaudeDesktopProviderCard {...claudeDesktopProps} viewMode={viewMode} />
              </div>
              <div data-fixture={'gemini-' + viewMode}>
                <GeminiCliProviderCard {...geminiProps} viewMode={viewMode} />
              </div>
              <div data-fixture={'grok-official-' + viewMode}>
                <GrokProviderCard {...grokCommonProps} provider={grokOfficialProvider} isApplied={false}
                  officialAccounts={grokOfficialAccounts} viewMode={viewMode}
                  onOfficialAccountRefresh={refresh(viewMode, 'refresh')}
                  onOfficialAccountRefreshToken={refresh(viewMode, 'token')}
                  refreshingOfficialAccountId={pending?.viewMode === viewMode && pending.action === 'refresh' ? pending.accountId : null}
                  refreshingOfficialTokenAccountId={pending?.viewMode === viewMode && pending.action === 'token' ? pending.accountId : null}
                />
              </div>
              <div data-fixture={'grok-custom-' + viewMode}>
                <GrokProviderCard {...grokCommonProps} provider={grokCustomProvider} isApplied={false} viewMode={viewMode} />
              </div>
              {omoProviders.map((provider, index) => {
                const name = (index === 0 ? 'omo-default-' : 'omo-other-') + viewMode;
                return (
                  <div data-fixture={name} key={provider.key}>
                    <OmoNativeProviderCard {...omoProps} provider={provider} viewMode={viewMode}
                      defaultModelId={index === 0 ? 'model-default' : undefined}
                      deleteDisabledReason={index === 0 ? '默认渠道不能删除' : undefined}
                      onDelete={() => recordDelete(name, provider.key)}
                    />
                  </div>
                );
              })}
            </React.Fragment>
          ))}
        <div data-fixture="openclaw-list">
          <OpenClawProviderCard
            {...openClawProps}
            providerId="relay-main"
            config={openClawProvider}
            viewMode="list"
          />
        </div>
        <div data-fixture="openclaw-card">
          <OpenClawProviderCard
            {...openClawProps}
            providerId="relay-main"
            config={openClawProvider}
            viewMode="card"
          />
        </div>
        {/* The default provider: delete stays visible but explains itself. */}
        <div data-fixture="openclaw-default-list">
          <OpenClawProviderCard
            {...openClawProps}
            providerId="relay-main"
            config={openClawProvider}
            viewMode="list"
            deleteDisabledReason="该渠道已设为默认，不可删除"
          />
        </div>

        <div data-fixture="hermes-list">
          <HermesProviderCard
            {...hermesProps}
            provider={hermesProvider}
            defaultModelId="claude-opus-4-8"
            viewMode="list"
          />
        </div>
        <div data-fixture="hermes-card">
          <HermesProviderCard
            {...hermesProps}
            provider={hermesProvider}
            defaultModelId="claude-opus-4-8"
            viewMode="card"
          />
        </div>
        {/* The page owns the read-only gating: it passes no callbacks at all,
            which is what the card must render as "no actions". */}
        <div data-fixture="hermes-readonly-list">
          <HermesProviderCard
            provider={hermesReadOnlyProvider}
            viewMode="list"
          />
        </div>

        </div>
        </>}
      </MemoryRouter>
    </ConfigProvider>
  );
}

createRoot(document.getElementById('root')).render(<FixtureHost />);
