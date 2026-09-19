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

// Internal DI/service paths used by `web/app/monaco.ts` to override Monaco's
// clipboard service with a Tauri-backend one (issue #369). These modules ship
// JS only — declare just the members we actually touch.
declare module 'monaco-editor/esm/vs/editor/standalone/browser/standaloneServices' {
  export const StandaloneServices: {
    initialize(overrides: Record<string, unknown>): unknown;
  };
}

declare module 'monaco-editor/esm/vs/platform/instantiation/common/descriptors.js' {
  export class SyncDescriptor<Ctor> {
    constructor(ctor: Ctor, staticArguments?: unknown[], supportsDelayedInstantiation?: boolean);
  }
}

declare module 'monaco-editor/esm/vs/platform/clipboard/common/clipboardService.js' {
  export const IClipboardService: ((...args: unknown[]) => unknown) & {
    toString(): string;
  };
}

declare module 'monaco-editor/esm/vs/platform/clipboard/browser/clipboardService.js' {
  export declare class BrowserClipboardService {
    // Constructor args are Monaco DI services (ILayoutService/ILogService),
    // injected by the container when the SyncDescriptor is instantiated.
    constructor(...args: unknown[]);
    readText(type?: string): Promise<string>;
    writeText(text: string, type?: string): Promise<void>;
  }
}
