import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker';

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
