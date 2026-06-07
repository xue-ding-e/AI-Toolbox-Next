import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildSessionDetailPath,
  getSessionDetailRoutePath,
  parseSessionDetailSearchParams,
} from '../../../../../features/coding/shared/sessionManager/sessionDetailNavigation.ts';

test('buildSessionDetailPath encodes Windows source paths in query string', () => {
  const sourcePath = 'D:\\Users\\测试 项目\\session file.jsonl';
  const path = buildSessionDetailPath('claudecode', sourcePath);
  const url = new URL(path, 'http://localhost');

  assert.equal(url.pathname, '/coding/claudecode/sessions/detail');
  assert.equal(url.searchParams.get('sourcePath'), sourcePath);
});

test('buildSessionDetailPath keeps OpenCode sqlite source round-trippable', () => {
  const sourcePath = 'sqlite:C:\\Users\\me\\opencode.db:ses_123456';
  const path = buildSessionDetailPath('opencode', sourcePath);
  const parsed = parseSessionDetailSearchParams(new URL(path, 'http://localhost').searchParams);

  assert.deepEqual(parsed, { sourcePath });
});

test('buildSessionDetailPath supports parent and subagent source paths', () => {
  const parentSourcePath = 'D:\\GitHub\\project\\.claude\\parent.jsonl';
  const subagentSourcePath = 'D:\\GitHub\\project\\.claude\\subagents\\child.jsonl';
  const path = buildSessionDetailPath('geminicli', parentSourcePath, subagentSourcePath);
  const parsed = parseSessionDetailSearchParams(new URL(path, 'http://localhost').searchParams);

  assert.deepEqual(parsed, {
    sourcePath: parentSourcePath,
    subagentSourcePath,
  });
});

test('parseSessionDetailSearchParams rejects missing sourcePath', () => {
  assert.equal(parseSessionDetailSearchParams(new URLSearchParams()), null);
});

test('getSessionDetailRoutePath returns the hidden route path for each tool', () => {
  assert.equal(getSessionDetailRoutePath('codex'), '/coding/codex/sessions/detail');
  assert.equal(getSessionDetailRoutePath('openclaw'), '/coding/openclaw/sessions/detail');
});
