import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildCodexCleanupPlan,
  describeCodexCleanupOutcome,
  supportsCodexCleanup,
} from '../../../../../features/coding/shared/sessionManager/codexCleanupOptions.ts';
import type {
  CodexCleanupPreview,
  CodexCleanupPreviewItem,
  ScratchWorkspaceInfo,
} from '../../../../../features/coding/shared/sessionManager/types.ts';

const workspace = (
  overrides: Partial<ScratchWorkspaceInfo> = {},
): ScratchWorkspaceInfo => ({
  path: '/Users/me/Documents/Codex/2026-09-25/xi',
  exists: true,
  isEmpty: true,
  fileCount: 0,
  totalBytes: 0,
  hasGit: false,
  truncated: false,
  ...overrides,
});

const item = (
  overrides: Partial<CodexCleanupPreviewItem> = {},
): CodexCleanupPreviewItem => ({
  sourcePath: '/Users/me/.codex/sessions/2026/09/25/rollout-a.jsonl',
  projectDir: '/Users/me/Documents/Codex/2026-09-25/xi',
  workspace: workspace(),
  trustKeys: ['/Users/me/Documents/Codex/2026-09-25/xi'],
  configPath: '/Users/me/.codex/config.toml',
  runtimeSource: 'local',
  runtimeDistro: null,
  ...overrides,
});

const preview = (items: CodexCleanupPreviewItem[]): CodexCleanupPreview => ({ items });

test('only Codex sessions offer scratch cleanup', () => {
  assert.equal(supportsCodexCleanup('codex'), true);
  assert.equal(supportsCodexCleanup('claudecode'), false);
  assert.equal(supportsCodexCleanup('opencode'), false);
});

test('an empty workspace and its trust entry are preselected', () => {
  const plan = buildCodexCleanupPlan(preview([item()]));

  assert.ok(plan);
  assert.equal(plan.choice.removeWorkspace, true);
  assert.equal(plan.choice.removeTrustEntry, true);
  assert.equal(plan.workspaceCount, 1);
  assert.equal(plan.nonEmptyWorkspaceCount, 0);

  const workspaceRow = plan.rows.find((row) => row.kind === 'workspace');
  assert.equal(workspaceRow?.defaultChecked, true);
  assert.equal(workspaceRow?.detailKey, 'sessionManager.codexCleanupWorkspaceEmpty');
});

test('a workspace with files is offered but not preselected', () => {
  const plan = buildCodexCleanupPlan(
    preview([item({ workspace: workspace({ isEmpty: false, fileCount: 12, totalBytes: 4096 }) })]),
  );

  assert.ok(plan);
  assert.equal(plan.choice.removeWorkspace, false);
  assert.equal(plan.nonEmptyWorkspaceCount, 1);

  const workspaceRow = plan.rows.find((row) => row.kind === 'workspace');
  assert.equal(workspaceRow?.defaultChecked, false);
  assert.equal(workspaceRow?.detailKey, 'sessionManager.codexCleanupWorkspaceFiles');
  assert.equal(workspaceRow?.params.fileCount, 12);
});

test('a workspace holding a git repository is never offered', () => {
  const plan = buildCodexCleanupPlan(
    preview([item({ workspace: workspace({ hasGit: true }), trustKeys: [] })]),
  );

  assert.equal(plan, null);
});

test('a workspace that is already gone is not offered', () => {
  // Removed by hand, or by an earlier cleanup: there is nothing to delete, and
  // describing it as a downloadable-looking "0 files" directory would also flip
  // the non-empty default, keeping an empty sibling workspace unchecked.
  const plan = buildCodexCleanupPlan(
    preview([item({ workspace: workspace({ exists: false, isEmpty: false }) })]),
  );

  assert.ok(plan);
  assert.equal(
    plan.rows.some((row) => row.kind === 'workspace'),
    false,
  );
  assert.equal(plan.choice.removeWorkspace, false);
  assert.equal(plan.choice.removeTrustEntry, true);
});

test('bulk cleanup aggregates the selection and its risk', () => {
  const plan = buildCodexCleanupPlan(
    preview([
      item(),
      item({
        sourcePath: '/Users/me/.codex/sessions/2026/09/25/rollout-b.jsonl',
        workspace: workspace({
          path: '/Users/me/Documents/Codex/2026-09-25/other',
          isEmpty: false,
          fileCount: 3,
        }),
        trustKeys: ['/Users/me/Documents/Codex/2026-09-25/other'],
      }),
    ]),
  );

  assert.ok(plan);
  assert.equal(plan.workspaceCount, 2);
  assert.equal(plan.nonEmptyWorkspaceCount, 1);
  assert.equal(plan.trustKeyCount, 2);
  // One directory holds files, so the removal is left to the user.
  assert.equal(plan.choice.removeWorkspace, false);
  assert.equal(plan.choice.removeTrustEntry, true);
});

test('sessions without scratch residue produce no plan', () => {
  assert.equal(buildCodexCleanupPlan(null), null);
  assert.equal(buildCodexCleanupPlan(preview([])), null);
  assert.equal(
    buildCodexCleanupPlan(preview([item({ workspace: null, trustKeys: [] })])),
    null,
  );
});

test('a cleanup outcome reports removals, skips and failures separately', () => {
  assert.equal(describeCodexCleanupOutcome(null), null);
  assert.equal(
    describeCodexCleanupOutcome({
      removedWorkspaces: [],
      removedTrustKeys: [],
      removedDateDirs: [],
      skipped: [],
      failures: [],
    }),
    null,
  );

  const success = describeCodexCleanupOutcome({
    removedWorkspaces: ['/Users/me/Documents/Codex/2026-09-25/xi'],
    removedTrustKeys: ['/Users/me/Documents/Codex/2026-09-25/xi'],
    removedDateDirs: [],
    skipped: [],
    failures: [],
  });
  assert.equal(success?.tone, 'success');
  assert.equal(success?.key, 'sessionManager.codexCleanupSuccess');
  assert.equal(success?.params.workspaces, 1);
  assert.equal(success?.params.trustKeys, 1);

  const skipped = describeCodexCleanupOutcome({
    removedWorkspaces: [],
    removedTrustKeys: [],
    removedDateDirs: [],
    skipped: [{ target: '/Users/me/x', reason: 'another Codex session still uses this directory' }],
    failures: [],
  });
  assert.equal(skipped?.tone, 'warning');
  assert.equal(skipped?.key, 'sessionManager.codexCleanupSkipped');

  const failed = describeCodexCleanupOutcome({
    removedWorkspaces: [],
    removedTrustKeys: [],
    removedDateDirs: [],
    skipped: [],
    failures: [{ target: '/Users/me/.codex/config.toml', error: 'Failed to parse config.toml' }],
  });
  assert.equal(failed?.tone, 'warning');
  assert.equal(failed?.key, 'sessionManager.codexCleanupFailed');
  assert.equal(failed?.params.error, 'Failed to parse config.toml');
});