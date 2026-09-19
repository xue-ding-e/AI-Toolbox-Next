import assert from 'node:assert/strict';
import test from 'node:test';

import { copyTextToClipboard, readClipboardText } from '../../services/clipboardApi';

// `invoke` resolves through `window.__TAURI_INTERNALS__`; stubbing it lets the
// tests drive the Tauri-backend path, its Web fallback and the double failure.

interface TauriInternalsStub {
  invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
}

type NavigatorStub = {
  clipboard?: {
    writeText?: (text: string) => Promise<void>;
    readText?: () => Promise<string>;
  };
};

const originalWindow = (globalThis as { window?: unknown }).window;
const originalNavigator = (globalThis as { navigator?: unknown }).navigator;

function withStubs(internals: TauriInternalsStub | undefined, navigator: NavigatorStub): void {
  (globalThis as { window?: unknown }).window = internals
    ? { __TAURI_INTERNALS__: internals }
    : undefined;
  Object.defineProperty(globalThis, 'navigator', {
    value: navigator,
    configurable: true,
  });
}

test.afterEach(() => {
  (globalThis as { window?: unknown }).window = originalWindow;
  Object.defineProperty(globalThis, 'navigator', {
    value: originalNavigator,
    configurable: true,
  });
});

test('copy prefers the Tauri backend when it succeeds', async () => {
  const invokedCommands: string[] = [];
  let webWriteCalls = 0;
  withStubs(
    {
      invoke: async (cmd, args) => {
        invokedCommands.push(cmd);
        assert.equal((args as { text?: string }).text, 'token-1');
      },
    },
    {
      clipboard: {
        writeText: async () => {
          webWriteCalls += 1;
        },
      },
    },
  );

  await copyTextToClipboard('token-1');

  assert.deepEqual(invokedCommands, ['copy_text_to_clipboard']);
  assert.equal(webWriteCalls, 0);
});

test('read prefers the Tauri backend when it succeeds', async () => {
  const invokedCommands: string[] = [];
  withStubs(
    {
      invoke: async (cmd) => {
        invokedCommands.push(cmd);
        return 'pasted text';
      },
    },
    {},
  );

  assert.equal(await readClipboardText(), 'pasted text');
  assert.deepEqual(invokedCommands, ['read_clipboard_text']);
});

test('copy falls back to the Web API when the backend fails', async () => {
  const written: string[] = [];
  withStubs(
    {
      invoke: async () => {
        throw new Error('backend unavailable');
      },
    },
    {
      clipboard: {
        writeText: async (text) => {
          written.push(text);
        },
      },
    },
  );

  await copyTextToClipboard('fallback-copy');
  assert.deepEqual(written, ['fallback-copy']);
});

test('read falls back to the Web API when the backend fails', async () => {
  withStubs(
    {
      invoke: async () => {
        throw new Error('backend unavailable');
      },
    },
    {
      clipboard: {
        readText: async () => 'web-clipboard',
      },
    },
  );

  assert.equal(await readClipboardText(), 'web-clipboard');
});

test('copy and read surface an error when both transports fail', async () => {
  withStubs(
    {
      invoke: async () => {
        throw new Error('backend unavailable');
      },
    },
    {
      clipboard: {
        writeText: async () => {
          throw new Error('web write denied');
        },
        readText: async () => {
          throw new Error('web read denied');
        },
      },
    },
  );

  await assert.rejects(copyTextToClipboard('x'), /web write denied/);
  await assert.rejects(readClipboardText(), /web read denied/);
});
