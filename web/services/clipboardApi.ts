import { invoke } from '@tauri-apps/api/core';

/**
 * System clipboard helpers for environments where the async Web Clipboard API
 * is unavailable or unreliable (issue #369).
 *
 * Monaco's context-menu paste has no native paste event to lean on, so it goes
 * through the service below; WSLg's WebKitGTK bridge rejects
 * `navigator.clipboard.readText()` entirely and WebView2 still fails it even
 * with the permission granted. The Tauri backend (arboard) talks to the OS
 * clipboard directly; the Web API remains the fallback for plain-browser dev.
 */
export const copyTextToClipboard = async (text: string): Promise<void> => {
  try {
    await invoke('copy_text_to_clipboard', { text });
    return;
  } catch (nativeError) {
    try {
      await navigator.clipboard.writeText(text);
    } catch (webError) {
      throw webError instanceof Error
        ? webError
        : (nativeError instanceof Error ? nativeError : new Error(String(webError ?? nativeError)));
    }
  }
};

export const readClipboardText = async (): Promise<string> => {
  try {
    return await invoke<string>('read_clipboard_text');
  } catch (nativeError) {
    try {
      return await navigator.clipboard.readText();
    } catch (webError) {
      throw webError instanceof Error
        ? webError
        : (nativeError instanceof Error ? nativeError : new Error(String(webError ?? nativeError)));
    }
  }
};
