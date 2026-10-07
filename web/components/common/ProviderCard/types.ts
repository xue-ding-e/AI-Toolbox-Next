/**
 * Shared types for ProviderCard and ModelItem components
 */

/**
 * Unified provider display data interface
 */
export interface ProviderDisplayData {
  id: string;
  name: string;
  sdkName: string;
  baseUrl: string;
}

/**
 * Unified model display data interface
 */
export interface ModelDisplayData {
  id: string;
  name: string;
  contextLimit?: number;
  outputLimit?: number;
  isPrimary?: boolean;
  /**
   * The CLI has this model switched off.
   *
   * Only rendered as a switch when the list also passes `onToggleDisabled`; a
   * list that cannot change the flag shows nothing.
   */
  isDisabled?: boolean;
}

/**
 * Official model display data interface (read-only)
 */
export interface OfficialModelDisplayData {
  id: string;
  name: string;
  isFree: boolean;
  context?: number;
  output?: number;
  status?: string;
}

export type ProviderConnectivityState = 'idle' | 'running' | 'success' | 'error';

export interface ProviderConnectivityStatusItem {
  status: ProviderConnectivityState;
  errorMessage?: string;
  tooltipMessage?: string;
  modelId?: string;
  totalMs?: number;
}

/**
 * i18n prefix type for different pages
 */
export type I18nPrefix = 'settings' | 'opencode' | 'openclaw' | 'pi' | 'grok' | 'ohMyPi' | 'dsh' | 'codex';
