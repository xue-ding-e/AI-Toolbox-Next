import { readClipboardText } from '../services/clipboardApi';

/**
 * Replay a native paste that arrived empty through the Tauri clipboard backend
 * (issue #384).
 *
 * Ctrl+C/V stay with the browser's native clipboard events — Monaco never runs
 * its paste action for them (see `components/common/AGENTS.md`) — so their
 * reliability is the WebView's own. In WSLg a WebKitGTK paste can arrive with an
 * empty `clipboardData` even though the OS clipboard holds text; Monaco then
 * inserts nothing with no feedback, and the Tauri/arboard route (which the
 * context-menu paste already uses) is never consulted.
 *
 * This listener only covers that failure mode: pastes that carry text, plain
 * inputs/textareas and editors without text focus keep the untouched native
 * path. The replay is asynchronous, so it never blocks or alters a working
 * paste.
 */

/** The subset of Monaco's editor this fallback needs. */
export interface PasteFallbackEditor {
  hasTextFocus(): boolean;
  trigger(source: 'keyboard', handlerId: 'paste', payload: { text: string }): unknown;
}

/** The subset of `DataTransfer` this fallback reads. */
export interface PasteClipboardData {
  getData(format: 'text/plain'): string;
}

export interface EmptyPasteFallbackOptions {
  /**
   * Resolves the Monaco editor holding text focus, if any. Injected by tests so
   * they do not have to load Monaco; the default lazy-loads the editor API.
   */
  findFocusedEditor?: () =>
    | PasteFallbackEditor
    | undefined
    | Promise<PasteFallbackEditor | undefined>;
  /** Clipboard transport; defaults to the Tauri backend with its Web fallback. */
  readClipboard?: () => Promise<string>;
}

const defaultFindFocusedEditor = async (): Promise<PasteFallbackEditor | undefined> => {
  const monaco = await import('monaco-editor/esm/vs/editor/editor.api');
  return monaco.editor
    .getEditors()
    .find((editor) => editor.hasTextFocus()) as PasteFallbackEditor | undefined;
};

/**
 * Paste `text` into the focused editor when the native paste had nothing to
 * offer. Exported for tests; the listener below is the production entry point.
 */
export async function replayEmptyPaste(
  clipboardData: PasteClipboardData | null | undefined,
  deps: {
    findFocusedEditor: () => PasteFallbackEditor | undefined | Promise<PasteFallbackEditor | undefined>;
    readClipboard: () => Promise<string>;
  },
): Promise<void> {
  // A native paste that carries text is left exactly as it was.
  if (clipboardData?.getData('text/plain')) return;

  const editor = await deps.findFocusedEditor();
  if (!editor?.hasTextFocus()) return;

  let text = '';
  try {
    text = await deps.readClipboard();
  } catch {
    // Both transports failed; there is nothing left to fall back to.
    return;
  }
  // Reading is asynchronous: the modal may have closed (editor disposed) or the
  // focus may have moved. Re-check so the text never lands in a stale editor.
  if (!text || !editor.hasTextFocus()) return;

  try {
    editor.trigger('keyboard', 'paste', { text });
  } catch {
    // Disposed editor: the paste is lost, exactly as it was without the fallback.
  }
}

export function installEmptyPasteFallback(options: EmptyPasteFallbackOptions = {}): void {
  const deps = {
    findFocusedEditor: options.findFocusedEditor ?? defaultFindFocusedEditor,
    readClipboard: options.readClipboard ?? readClipboardText,
  };

  document.addEventListener(
    'paste',
    (event) => {
      void replayEmptyPaste((event as ClipboardEvent).clipboardData, deps).catch(() => {
        // A failed replay must never surface as an unhandled rejection: the
        // native paste already failed silently, so stay silent here too.
      });
    },
    true,
  );
}