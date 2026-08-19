/**
 * Claude Desktop Configuration Types
 *
 * Type definitions for Claude Desktop (desktop GUI) provider / config management.
 * Field naming follows serde `camelCase` (all backend fields arrive already
 * converted by the adapter / serde rename), mirroring `types/claudecode.ts`.
 */

import type { GatewayProviderProfileReference } from '@/features/coding/shared/gateway/providerProfiles';
import type { CustomUserAgentState } from '@/features/coding/shared/providerUserAgent/customUserAgentUtils';

/** How a Claude Desktop provider is applied to the on-disk 3P gateway profile. */
export type ClaudeDesktopMode = 'direct' | 'proxy';

/** A single model route entry inside `meta.claudeDesktopModelRoutes`. */
export interface ClaudeDesktopModelRoute {
  model: string;
  labelOverride?: string;
  supports1m: boolean;
}

/** `meta.claudeDesktopModelRoutes`: route_id (claude-safe, e.g. `claude-sonnet-5`) → route. */
export type ClaudeDesktopModelRoutes = Record<string, ClaudeDesktopModelRoute>;

/**
 * Provider-level metadata. Backend stores this as an opaque JSON `Value`
 * (`meta`), so unknown extra fields are tolerated.
 */
export interface ClaudeDesktopMeta {
  claudeDesktopMode?: ClaudeDesktopMode;
  claudeDesktopModelRoutes?: ClaudeDesktopModelRoutes;
  /** Gateway provider profile reference used when proxying through the local gateway. */
  gatewayProfile?: GatewayProviderProfileReference;
  /** Upstream API format hint read by the gateway runtime (e.g. `anthropic_messages`). */
  apiFormat?: string;
  /** Upstream provider type hint (e.g. `deepseek`, `openrouter`). */
  providerType?: string;
  /** Provider-level custom User-Agent injected by the gateway on upstream requests. */
  customUserAgent?: string;
  [key: string]: unknown;
}

/** Claude Desktop provider settings configuration (direct-mode credentials). */
export interface ClaudeDesktopSettingsConfig {
  env?: {
    ANTHROPIC_BASE_URL?: string;
    ANTHROPIC_AUTH_TOKEN?: string;
    ANTHROPIC_API_KEY?: string; // legacy read fallback
    ANTHROPIC_MODEL?: string;
    ANTHROPIC_DEFAULT_HAIKU_MODEL?: string;
    ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME?: string;
    ANTHROPIC_DEFAULT_SONNET_MODEL?: string;
    ANTHROPIC_DEFAULT_SONNET_MODEL_NAME?: string;
    ANTHROPIC_DEFAULT_OPUS_MODEL?: string;
    ANTHROPIC_DEFAULT_OPUS_MODEL_NAME?: string;
    ANTHROPIC_DEFAULT_FABLE_MODEL?: string;
    ANTHROPIC_DEFAULT_FABLE_MODEL_NAME?: string;
  };
  [key: string]: unknown;
}

/** Claude Desktop provider record returned by `list_/create_/update_…`. */
export interface ClaudeDesktopProvider {
  id: string;
  name: string;
  category: string;
  settingsConfig: string; // JSON string of ClaudeDesktopSettingsConfig
  sourceProviderId?: string;
  websiteUrl?: string;
  notes?: string;
  icon?: string;
  iconColor?: string;
  sortIndex?: number;
  meta?: ClaudeDesktopMeta;
  isApplied: boolean;
  isDisabled: boolean;
  createdAt: string;
  updatedAt: string;
}

/** Input for `create_claude_desktop_provider` / internal update payloads. */
export interface ClaudeDesktopProviderInput {
  id?: string;
  name: string;
  category: string;
  settingsConfig: string;
  sourceProviderId?: string;
  websiteUrl?: string;
  notes?: string;
  icon?: string;
  iconColor?: string;
  sortIndex?: number;
  meta?: ClaudeDesktopMeta;
}

/** Common (base) config - the raw JSON body, e.g. `{ "mcpServers": … }`. */
export interface ClaudeDesktopCommonConfig {
  config: string;
  updatedAt: string;
}

export interface ClaudeDesktopCommonConfigInput {
  config: string;
}

/** Resolved on-disk paths for the Claude Desktop 3P profile. */
export interface ClaudeDesktopPathInfo {
  supported: boolean;
  normalConfigPath?: string;
  threepConfigPath?: string;
  configLibraryPath?: string;
  profilePath?: string;
  metaPath?: string;
  message?: string;
}

/** Status of the current Claude Desktop on-disk configuration. */
export interface ClaudeDesktopStatus {
  supported: boolean;
  configured: boolean;
  appliedId?: string;
  profilePath?: string;
  configLibraryPath?: string;
  mode?: ClaudeDesktopMode;
  actualBaseUrl?: string;
}

/** Form values for creating / editing a Claude Desktop provider. */
export interface ClaudeDesktopFormValues {
  name: string;
  category?: 'official' | 'custom';
  providerEndpointKey?: string;
  providerProfileId?: string;
  providerEndpointId?: string;
  apiFormat?: string;
  baseUrl?: string;
  apiKey?: string;
  model?: string;
  haikuModel?: string;
  haikuModelName?: string;
  sonnetModel?: string;
  sonnetModelName?: string;
  opusModel?: string;
  opusModelName?: string;
  fableModel?: string;
  fableModelName?: string;
  notes?: string;
  /** Provider-level custom User-Agent state (gateway-injected). */
  customUserAgent?: CustomUserAgentState;
}
