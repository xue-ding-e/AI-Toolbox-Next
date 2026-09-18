/**
 * Mini browser service.
 *
 * Opens a relay's console in an embedded native webview window so the user can
 * read API balance / usage without leaving the toolbox and without losing the
 * main window's state.
 *
 * A native top-level window is used rather than an `<iframe>` because relay
 * dashboards normally send `X-Frame-Options: DENY` (or `frame-ancestors 'none'`
 * in their CSP), which would render an empty frame. It also reuses the webview
 * engine the app already ships, so there is no extra runtime cost.
 */

import { invoke } from '@tauri-apps/api/core';

/** Longest address accepted, mirroring the backend guard. */
export const MINI_BROWSER_MAX_URL_LENGTH = 2048;

/**
 * Normalise user input into an absolute http/https URL.
 *
 * Mirrors the backend `normalise_browser_url` so the prompt can reject bad
 * input before invoking: a bare host gets `https://` prefixed, and non-web
 * schemes (`javascript:`, `file:`, `data:`) are refused instead of rewritten —
 * those are exactly the strings that would escape the browser sandbox.
 *
 * Returns `null` when the value is not usable.
 */
export const normaliseMiniBrowserUrl = (raw: string): string | null => {
  const trimmed = raw.trim();
  if (!trimmed || trimmed.length > MINI_BROWSER_MAX_URL_LENGTH) return null;
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(trimmed)) return null;

  const hasScheme = /^[a-zA-Z][a-zA-Z\d+\-.]*:\/\//.test(trimmed);
  const candidate = hasScheme ? trimmed : `https://${trimmed}`;
  try {
    const parsed = new URL(candidate);
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return null;
    if (!parsed.hostname) return null;
    return parsed.toString();
  } catch {
    return null;
  }
};

/** Open a URL in the embedded browser, creating its window on first use. */
export const openMiniBrowser = async (url: string): Promise<void> => {
  await invoke('mini_browser_open', { url });
};

/** Navigate the embedded browser to another URL, opening it when necessary. */
export const navigateMiniBrowser = async (url: string): Promise<void> => {
  await invoke('mini_browser_navigate', { url });
};

/** Address currently shown, or `null` when the browser window is closed. */
export const getMiniBrowserCurrentUrl = async (): Promise<string | null> => {
  return await invoke<string | null>('mini_browser_current_url');
};

/** Whether the embedded browser window currently exists. */
export const isMiniBrowserOpen = async (): Promise<boolean> => {
  return await invoke<boolean>('mini_browser_is_open');
};

/** Close the embedded browser window. No-op when it is not open. */
export const closeMiniBrowser = async (): Promise<void> => {
  await invoke('mini_browser_close');
};
