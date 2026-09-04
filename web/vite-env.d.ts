/// <reference types="vite/client" />

// monaco-editor's package.json exports "monaco-editor/*" → "./*" without a
// dedicated "types" condition, so TS cannot resolve the deep editor.api
// subpath even though the .d.ts exists. Re-declare it against the package's
// own types so `import * as monaco from 'monaco-editor/esm/vs/editor/editor.api'`
// type-checks. Runtime resolution works fine via Vite; this is types-only.
declare module 'monaco-editor/esm/vs/editor/editor.api' {
  export * from 'monaco-editor';
}

// Same situation for the language contribution subpaths — they are side-effect
// imports with no own types, and we never use their exports.
declare module 'monaco-editor/esm/vs/language/json/monaco.contribution';
