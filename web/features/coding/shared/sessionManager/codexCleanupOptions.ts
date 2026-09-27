import { previewCodexSessionCleanup } from './sessionManagerApi';
import type { CodexCleanupOptions, CodexCleanupPreview, SessionTool } from './types';
import {
  describeCleanupReport,
  isRemovableWorkspace,
  type CodexCleanupOutcome,
  type CodexCleanupReportKeys,
} from './codexCleanupPolicy';

/** Whether a tool's sessions can have Codex scratch residue. */
export const supportsCodexCleanup = (tool: SessionTool): boolean => tool === 'codex';

/** One selectable cleanup action in the delete confirmation. */
export interface CodexCleanupDialogRow {
  kind: 'workspace' | 'trust';
  /** i18n key of the checkbox label. */
  labelKey: string;
  /** i18n key of the helper line under the label. */
  detailKey: string;
  params: Record<string, unknown>;
  /** Preselected state (empty workspaces and trust entries only). */
  defaultChecked: boolean;
}

export interface CodexCleanupDialogPlan {
  rows: CodexCleanupDialogRow[];
  /** The preselected choice the dialog starts from. */
  choice: CodexCleanupOptions;
  workspaceCount: number;
  nonEmptyWorkspaceCount: number;
  trustKeyCount: number;
}

/**
 * Turn a cleanup preview into the rows and preselection of the confirmation
 * dialog.
 *
 * The default follows the risk: an empty scratch directory holds nothing and is
 * preselected, a directory with files is shown with its file count and left to
 * the user, and a directory holding a git repository is not offered at all.
 * Trust entries are Codex's own setting and are reselected by default.
 *
 * Returns `null` when these sessions have nothing to clean up.
 */
export const buildCodexCleanupPlan = (
  preview: CodexCleanupPreview | null | undefined,
): CodexCleanupDialogPlan | null => {
  if (!preview || preview.items.length === 0) {
    return null;
  }

  const workspaces = preview.items
    .map((item) => item.workspace)
    .filter((workspace): workspace is NonNullable<typeof workspace> => Boolean(workspace));
  const removable = workspaces.filter(isRemovableWorkspace);
  const nonEmpty = removable.filter((workspace) => !workspace.isEmpty);

  const trustKeys = preview.items.flatMap((item) => item.trustKeys);
  if (removable.length === 0 && trustKeys.length === 0) {
    return null;
  }

  const rows: CodexCleanupDialogRow[] = [];

  if (removable.length > 0) {
    rows.push({
      kind: 'workspace',
      labelKey:
        removable.length === 1
          ? 'sessionManager.codexCleanupWorkspace'
          : 'sessionManager.codexCleanupWorkspaces',
      detailKey:
        nonEmpty.length === 0
          ? 'sessionManager.codexCleanupWorkspaceEmpty'
          : 'sessionManager.codexCleanupWorkspaceFiles',
      params: {
        count: removable.length,
        path: removable[0].path,
        fileCount: nonEmpty.reduce((total, workspace) => total + workspace.fileCount, 0),
        nonEmptyCount: nonEmpty.length,
      },
      defaultChecked: nonEmpty.length === 0,
    });
  }

  if (trustKeys.length > 0) {
    rows.push({
      kind: 'trust',
      labelKey:
        trustKeys.length === 1
          ? 'sessionManager.codexCleanupTrust'
          : 'sessionManager.codexCleanupTrustEntries',
      detailKey: 'sessionManager.codexCleanupTrustHint',
      params: {
        count: trustKeys.length,
        key: trustKeys[0],
        keys: trustKeys.join('\n'),
      },
      defaultChecked: true,
    });
  }

  const choice: CodexCleanupOptions = {
    removeWorkspace: rows.some((row) => row.kind === 'workspace' && row.defaultChecked),
    removeTrustEntry: rows.some((row) => row.kind === 'trust' && row.defaultChecked),
  };

  return {
    rows,
    choice,
    workspaceCount: removable.length,
    nonEmptyWorkspaceCount: nonEmpty.length,
    trustKeyCount: trustKeys.length,
  };
};

/** The keys the delete confirmation reports its cleanup with. */
export const CODEX_CLEANUP_REPORT_KEYS: CodexCleanupReportKeys = {
  failed: 'sessionManager.codexCleanupFailed',
  skipped: 'sessionManager.codexCleanupSkipped',
  success: 'sessionManager.codexCleanupSuccess',
};

/**
 * Describe what an auxiliary cleanup did, in the delete confirmation's wording.
 *
 * Skips and failures mean the session was deleted but its residue was not fully
 * cleaned, so they are surfaced as a warning next to the success message rather
 * than folded into the deletion result.
 */
export const describeCodexCleanupOutcome = (
  summary: Parameters<typeof describeCleanupReport>[0],
): CodexCleanupOutcome | null =>
  describeCleanupReport(summary, CODEX_CLEANUP_REPORT_KEYS);

/**
 * Fetch the cleanup options for a delete confirmation.
 *
 * A failed preview must never block the deletion: it logs and resolves to
 * `null`, which renders today's confirmation with no cleanup options.
 */
export const requestCodexCleanupPlan = async (
  tool: SessionTool,
  sourcePaths: string[],
): Promise<CodexCleanupDialogPlan | null> => {
  if (!supportsCodexCleanup(tool) || sourcePaths.length === 0) {
    return null;
  }

  try {
    return buildCodexCleanupPlan(await previewCodexSessionCleanup(tool, sourcePaths));
  } catch (error) {
    console.warn('[sessionManager] Codex cleanup preview failed', error);
    return null;
  }
};