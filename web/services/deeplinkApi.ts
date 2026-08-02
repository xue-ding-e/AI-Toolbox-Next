/**
 * Deep-Link API Service
 *
 * Handles the `aitoolbox://` provider-import deep-link communication with the
 * Tauri backend. The backend parses the URL and emits a `deep-link-import`
 * event carrying a `DeepLinkImportRequest`; after the frontend listener is
 * attached it drains any cold-start pending request, then the dialog confirms
 * with the user and invokes `import_from_deeplink_unified` to persist.
 */

import { invoke } from '@tauri-apps/api/core';

/** The app targeted by the deep link (v1 supports the three env-shaped tools). */
export type DeepLinkApp = 'claude' | 'codex' | 'gemini';

/** Normalized provider category, matching the backend's `normalize_category`. */
export type DeepLinkCategory = 'official' | 'third_party' | 'custom';

/** A parsed deep-link import request (mirrors the Rust `DeepLinkImportRequest`). */
export interface DeepLinkImportRequest {
  resource: 'provider';
  app: DeepLinkApp;
  name: string;
  category: DeepLinkCategory;
  apiKey?: string;
  baseUrl?: string;
  model?: string;
  homepage?: string;
  notes?: string;
  icon?: string;
  iconColor?: string;
  sourceProviderId?: string;
  /** Decoded tool-specific JSON/TOML override for `settings_config` / Codex TOML. */
  config?: string;
  /** Decoded Claude `extra_settings_config` override. */
  extra?: string;
  rawUrl: string;
}

/** Error payload for the `deep-link-error` event. */
export interface DeepLinkErrorPayload {
  url: string;
  error: string;
}

/** Result returned by the unified import command. */
export interface DeepLinkImportResult {
  type: 'provider';
  app: DeepLinkApp;
  id: string;
}

/**
 * Tell the backend that the frontend listener is attached and drain the latest
 * cold-start request, if one arrived before React mounted.
 */
export const markDeepLinkFrontendReady =
  async (): Promise<DeepLinkImportRequest | null> => {
    return await invoke<DeepLinkImportRequest | null>('mark_deeplink_frontend_ready');
  };

/**
 * Persist a deep-link provider after the user confirms in the dialog. This is
 * the only write path — the backend never writes on URL receipt.
 */
export const importFromDeeplinkUnified = async (
  request: DeepLinkImportRequest,
): Promise<DeepLinkImportResult> => {
  return await invoke<DeepLinkImportResult>('import_from_deeplink_unified', { request });
};
