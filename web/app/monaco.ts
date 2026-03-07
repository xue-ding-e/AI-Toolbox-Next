import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import cssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker';
import htmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker';
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker';
import tsWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker';

type MonacoWorkerFactory = new () => Worker;

interface MonacoEnvironmentConfig {
  getWorker: (_moduleId: string, label: string) => Worker;
}

const workerFactories: Record<string, MonacoWorkerFactory> = {
  css: cssWorker,
  editor: editorWorker,
  handlebars: htmlWorker,
  html: htmlWorker,
  javascript: tsWorker,
  json: jsonWorker,
  less: cssWorker,
  razor: htmlWorker,
  scss: cssWorker,
  typescript: tsWorker,
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
