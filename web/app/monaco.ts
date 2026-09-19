import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker';
import { StandaloneServices } from 'monaco-editor/esm/vs/editor/standalone/browser/standaloneServices';
import { SyncDescriptor } from 'monaco-editor/esm/vs/platform/instantiation/common/descriptors.js';
import { IClipboardService } from 'monaco-editor/esm/vs/platform/clipboard/common/clipboardService.js';
import { TauriClipboardService } from './monacoClipboardService';

type MonacoWorkerFactory = new () => Worker;

interface MonacoEnvironmentConfig {
  getWorker: (_moduleId: string, label: string) => Worker;
}

// Only editor + json workers are registered: every editor in the app uses
// one of `plaintext`, `markdown`, `yaml`, `toml`, or `json` — the first four
// fall back to the generic editor worker (Monaco runs their tokenizers on the
// main thread), and only `json` has a dedicated worker. The css/html/ts
// workers were previously imported and bundled (~8.7 MB combined) but never
// loaded — no editor sets `language` to css/html/typescript/javascript — so
// they only inflated the webview's resident JS heap. Removing them keeps the
// bundle lean without changing any editor's behaviour.
const workerFactories: Record<string, MonacoWorkerFactory> = {
  editor: editorWorker,
  json: jsonWorker,
};

const globalScope = self as typeof globalThis & {
  MonacoEnvironment?: MonacoEnvironmentConfig;
};

globalScope.MonacoEnvironment = {
  getWorker(_moduleId: string, label: string) {
    const WorkerFactory = workerFactories[label] ?? editorWorker;

    return new WorkerFactory();
  },
};

// Replace Monaco's clipboard service with one backed by the OS clipboard
// (issue #369, implementation in `./monacoClipboardService`): Monaco's
// context-menu copy/paste cannot use the browser's native clipboard events the
// way Ctrl+C/Ctrl+V do, so it falls back to `navigator.clipboard`, which is
// unavailable in the Tauri WebViews. The Web API remains the fallback of the
// base service for plain-browser dev.
//
// `StandaloneServices.initialize` only takes effect on the first call and only
// overrides services that have not been instantiated yet, so this module must
// stay in the `main.tsx` import graph ahead of the first editor creation.
StandaloneServices.initialize({
  // IClipboardService's decorator stringifies to 'clipboardService' — the key
  // `StandaloneServices.initialize` re-resolves via createDecorator.
  [IClipboardService.toString()]: new SyncDescriptor(TauriClipboardService),
});
