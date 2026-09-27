import assert from 'node:assert/strict';
import test from 'node:test';

import type {
  CodexScratchResidue,
  CodexScratchResidueWorkspace,
} from '../../../../../types/codex.ts';
import {
  defaultResidueSelection,
  describeResidueCleanOutcome,
  emptyResidueSelection,
  isResidueSelectionEmpty,
  isWorkspaceRemovable,
  residueSelectionCount,
  setResidueGroupSelection,
  summarizeResidueSelection,
  toggleResidueSelection,
} from '../../../../../features/coding/codex/utils/codexScratchResidueSelection.ts';

const workspace = (
  path: string,
  overrides: Partial<CodexScratchResidueWorkspace['info']> = {},
): CodexScratchResidueWorkspace => ({
  info: {
    path,
    exists: true,
    isEmpty: true,
    fileCount: 0,
    totalBytes: 0,
    hasGit: false,
    truncated: false,
    ...overrides,
  },
  runtimeSource: 'local',
  runtimeDistro: null,
});

const report = (overrides: Partial<CodexScratchResidue> = {}): CodexScratchResidue => ({
  source: 'all',
  unavailable: false,
  scanComplete: true,
  configPaths: ['/Users/me/.codex/config.toml'],
  workspaces: [],
  trustEntries: [],
  emptyDateDirs: [],
  ...overrides,
});

test('only empty, non-git workspaces are preselected', () => {
  const scan = report({
    workspaces: [
      workspace('/Users/me/Documents/Codex/2026-09-25/empty'),
      workspace('/Users/me/Documents/Codex/2026-09-25/files', { isEmpty: false, fileCount: 4 }),
      workspace('/Users/me/Documents/Codex/2026-09-25/repo', { hasGit: true }),
    ],
    trustEntries: [
      {
        key: '/Users/me/Documents/Codex/2026-09-25/empty',
        configPath: '/Users/me/.codex/config.toml',
        dirExists: true,
        runtimeSource: 'local',
        runtimeDistro: null,
      },
    ],
    emptyDateDirs: ['/Users/me/Documents/Codex/2026-09-19'],
  });

  const selection = defaultResidueSelection(scan);

  assert.deepEqual(selection.workspaces, ['/Users/me/Documents/Codex/2026-09-25/empty']);
  assert.deepEqual(selection.trustKeys, ['/Users/me/Documents/Codex/2026-09-25/empty']);
  assert.deepEqual(selection.dateDirs, ['/Users/me/Documents/Codex/2026-09-19']);
  assert.equal(residueSelectionCount(selection), 3);
  assert.equal(isWorkspaceRemovable(scan.workspaces[2]), false);
});

test('an empty report preselects nothing', () => {
  const selection = defaultResidueSelection(report());
  assert.equal(isResidueSelectionEmpty(selection), true);
  assert.equal(isResidueSelectionEmpty(emptyResidueSelection()), true);
  assert.equal(residueSelectionCount(emptyResidueSelection()), 0);
});

test('toggling and replacing a group keeps the other groups intact', () => {
  const scan = report({
    workspaces: [
      workspace('/Users/me/Documents/Codex/2026-09-25/a'),
      workspace('/Users/me/Documents/Codex/2026-09-25/b'),
    ],
    emptyDateDirs: ['/Users/me/Documents/Codex/2026-09-19'],
  });

  let selection = defaultResidueSelection(scan);
  selection = toggleResidueSelection(selection, 'workspace', '/Users/me/Documents/Codex/2026-09-25/b', true);
  assert.equal(selection.workspaces.length, 2);
  assert.equal(selection.dateDirs.length, 1);

  // Toggling the same value off must not duplicate or drop others.
  selection = toggleResidueSelection(selection, 'workspace', '/Users/me/Documents/Codex/2026-09-25/b', false);
  assert.deepEqual(selection.workspaces, ['/Users/me/Documents/Codex/2026-09-25/a']);

  selection = setResidueGroupSelection(selection, 'workspace', []);
  assert.equal(selection.workspaces.length, 0);
  assert.equal(selection.dateDirs.length, 1);
});

test('the summary counts the risk inside the selection', () => {
  const scan = report({
    workspaces: [
      workspace('/Users/me/Documents/Codex/2026-09-25/a'),
      workspace('/Users/me/Documents/Codex/2026-09-25/b', { isEmpty: false, fileCount: 2 }),
    ],
    trustEntries: [
      {
        key: '/Users/me/Documents/Codex/2026-09-25/a',
        configPath: '/Users/me/.codex/config.toml',
        dirExists: false,
        runtimeSource: 'local',
        runtimeDistro: null,
      },
    ],
    emptyDateDirs: ['/Users/me/Documents/Codex/2026-09-19'],
  });

  const summary = summarizeResidueSelection(
    {
      workspaces: [
        '/Users/me/Documents/Codex/2026-09-25/a',
        '/Users/me/Documents/Codex/2026-09-25/b',
      ],
      trustKeys: ['/Users/me/Documents/Codex/2026-09-25/a'],
      dateDirs: ['/Users/me/Documents/Codex/2026-09-19'],
    },
    scan,
  );

  assert.deepEqual(summary, {
    workspaceCount: 2,
    nonEmptyWorkspaceCount: 1,
    trustKeyCount: 1,
    dateDirCount: 1,
  });
});

test('a clean outcome separates removals, skips and failures', () => {
  assert.equal(describeResidueCleanOutcome(null), null);
  assert.equal(
    describeResidueCleanOutcome({
      removedWorkspaces: [],
      removedTrustKeys: [],
      removedDateDirs: [],
      skipped: [],
      failures: [],
    }),
    null,
  );

  const success = describeResidueCleanOutcome({
    removedWorkspaces: ['/a'],
    removedTrustKeys: ['/b'],
    removedDateDirs: ['/c'],
    skipped: [],
    failures: [],
  });
  assert.equal(success?.tone, 'success');
  assert.equal(success?.params.workspaces, 1);
  assert.equal(success?.params.trustKeys, 1);
  assert.equal(success?.params.dateDirs, 1);

  const partiallyFailed = describeResidueCleanOutcome({
    removedWorkspaces: [],
    removedTrustKeys: [],
    removedDateDirs: [],
    skipped: [],
    failures: [{ target: '/a', error: 'denied' }],
  });
  assert.equal(partiallyFailed?.tone, 'warning');
  assert.equal(partiallyFailed?.key, 'codex.scratchResidue.cleanFailed');

  const partiallySkipped = describeResidueCleanOutcome({
    removedWorkspaces: [],
    removedTrustKeys: [],
    removedDateDirs: [],
    skipped: [{ target: '/a', reason: 'another Codex session still uses this directory' }],
    failures: [],
  });
  assert.equal(partiallySkipped?.tone, 'warning');
  assert.equal(partiallySkipped?.key, 'codex.scratchResidue.cleanSkipped');
});