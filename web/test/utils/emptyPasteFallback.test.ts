/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { replayEmptyPaste, type PasteFallbackEditor } from '../../utils/emptyPasteFallback.ts';

function clipboardWith(text: string) {
  return { getData: (format: string) => (format === 'text/plain' ? text : '') };
}

function editorStub(focused = true): PasteFallbackEditor & {
  pasted: string[];
  focused: boolean;
} {
  return {
    pasted: [] as string[],
    focused,
    hasTextFocus() {
      return this.focused;
    },
    trigger(_source, _handlerId, payload) {
      this.pasted.push(payload.text);
    },
  };
}

test('replayEmptyPaste leaves a native paste that carries text alone', async () => {
  const editor = editorStub();
  let reads = 0;

  await replayEmptyPaste(clipboardWith('from the WebView'), {
    findFocusedEditor: () => editor,
    readClipboard: async () => {
      reads += 1;
      return 'from arboard';
    },
  });

  assert.equal(reads, 0);
  assert.deepEqual(editor.pasted, []);
});

test('replayEmptyPaste pastes the backend text when the native paste is empty', async () => {
  const editor = editorStub();

  await replayEmptyPaste(clipboardWith(''), {
    findFocusedEditor: () => editor,
    readClipboard: async () => 'from arboard',
  });

  assert.deepEqual(editor.pasted, ['from arboard']);
});

test('replayEmptyPaste treats a missing clipboardData as empty', async () => {
  const editor = editorStub();

  await replayEmptyPaste(null, {
    findFocusedEditor: () => editor,
    readClipboard: async () => 'from arboard',
  });

  assert.deepEqual(editor.pasted, ['from arboard']);
});

test('replayEmptyPaste does nothing without a focused editor', async () => {
  let reads = 0;

  await replayEmptyPaste(clipboardWith(''), {
    findFocusedEditor: () => undefined,
    readClipboard: async () => {
      reads += 1;
      return 'from arboard';
    },
  });

  assert.equal(reads, 0);
});

test('replayEmptyPaste does nothing when the editor lost text focus', async () => {
  const editor = editorStub(false);

  await replayEmptyPaste(clipboardWith(''), {
    findFocusedEditor: () => editor,
    readClipboard: async () => 'from arboard',
  });

  assert.deepEqual(editor.pasted, []);
});

test('replayEmptyPaste re-checks focus when the clipboard read resolves', async () => {
  const editor = editorStub();

  await replayEmptyPaste(clipboardWith(''), {
    findFocusedEditor: () => editor,
    readClipboard: async () => {
      // The modal closes / focus moves while the async read is in flight.
      editor.focused = false;
      return 'from arboard';
    },
  });

  assert.deepEqual(editor.pasted, []);
});

test('replayEmptyPaste survives an editor disposed mid-read', async () => {
  const editor = editorStub();
  editor.trigger = () => {
    throw new Error('editor disposed');
  };

  await assert.doesNotReject(
    replayEmptyPaste(clipboardWith(''), {
      findFocusedEditor: () => editor,
      readClipboard: async () => 'from arboard',
    }),
  );
});

test('replayEmptyPaste stays silent when both transports fail', async () => {
  const editor = editorStub();

  await replayEmptyPaste(clipboardWith(''), {
    findFocusedEditor: () => editor,
    readClipboard: async () => {
      throw new Error('backend unavailable');
    },
  });

  assert.deepEqual(editor.pasted, []);
});

test('replayEmptyPaste stays silent when the backend has no text', async () => {
  const editor = editorStub();

  await replayEmptyPaste(clipboardWith(''), {
    findFocusedEditor: () => editor,
    readClipboard: async () => '',
  });

  assert.deepEqual(editor.pasted, []);
});