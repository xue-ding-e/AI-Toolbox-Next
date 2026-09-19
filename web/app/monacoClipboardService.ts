import { BrowserClipboardService } from 'monaco-editor/esm/vs/platform/clipboard/browser/clipboardService.js';
import { copyTextToClipboard, readClipboardText } from '../services/clipboardApi';

/**
 * Monaco clipboard service backed by the OS clipboard (issue #369).
 *
 * Monaco's context-menu copy/paste cannot use the browser's native clipboard
 * events the way Ctrl+C/Ctrl+V do (Monaco deliberately leaves those to the
 * browser), so it falls back to `navigator.clipboard`, which fails silently
 * inside WebKitGTK (WSLg bridge) and still fails in WebView2 even with
 * `enable_clipboard_access` granting the permission. The Tauri backend
 * (arboard) talks to the OS clipboard directly; on failure this class falls
 * back to the base service, whose Web-API paths remain the plain-browser dev
 * route.
 *
 * Relative imports (not `@/`) so the node test runner can load this module.
 */
export class TauriClipboardService extends BrowserClipboardService {
  override async readText(type?: string | undefined): Promise<string> {
    try {
      return await readClipboardText();
    } catch {
      return super.readText(type);
    }
  }

  override async writeText(text: string, type?: string | undefined): Promise<void> {
    try {
      await copyTextToClipboard(text);
    } catch {
      await super.writeText(text, type);
    }
  }
}
