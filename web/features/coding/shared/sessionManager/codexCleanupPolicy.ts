import type { CodexCleanupSummary } from './types';

/**
 * The decisions both Codex cleanup surfaces share.
 *
 * The delete confirmation and the residue dialog ask the same two questions —
 * may this directory be deleted, and how is a cleanup result described — and
 * they must answer them identically: what one surface is willing to delete, the
 * other must not be seen offering. Only the wording of the two surfaces differs,
 * so it is passed in rather than duplicated.
 *
 * This module deliberately imports nothing but types: it is reachable from the
 * unit tests, which run the sources through a loader that resolves relative
 * specifiers only, so a value import of the surrounding UI or API layers would
 * make it untestable on its own.
 */

/**
 * Whether a scratch workspace may be offered for removal.
 *
 * A directory that is already gone has nothing to remove, and one holding a git
 * repository is a real project the backend refuses — anywhere below it, not only
 * at its root.
 */
export const isRemovableWorkspace = (
  workspace: { exists: boolean; hasGit: boolean } | null | undefined,
): boolean => Boolean(workspace?.exists) && !workspace?.hasGit;

export interface CodexCleanupOutcome {
  tone: 'success' | 'warning';
  key: string;
  params: Record<string, unknown>;
}

/** The i18n keys one cleanup surface reports its result with. */
export interface CodexCleanupReportKeys {
  failed: string;
  skipped: string;
  success: string;
}

/**
 * Describe what a cleanup did, or `null` when it had nothing to say.
 *
 * Failures and skips both mean a selected item was not cleaned, so they take
 * precedence over the summary of what was — the user needs to know what is still
 * there. `keys` carries the wording of the surface that asked.
 */
export const describeCleanupReport = (
  summary: CodexCleanupSummary | null | undefined,
  keys: CodexCleanupReportKeys,
): CodexCleanupOutcome | null => {
  if (!summary) {
    return null;
  }

  const removed = {
    workspaces: summary.removedWorkspaces.length,
    trustKeys: summary.removedTrustKeys.length,
    dateDirs: summary.removedDateDirs.length,
  };

  if (summary.failures.length > 0) {
    return {
      tone: 'warning',
      key: keys.failed,
      params: {
        ...removed,
        count: summary.failures.length,
        error: summary.failures[0].error,
        target: summary.failures[0].target,
      },
    };
  }

  if (summary.skipped.length > 0) {
    return {
      tone: 'warning',
      key: keys.skipped,
      params: {
        ...removed,
        count: summary.skipped.length,
        reason: summary.skipped[0].reason,
        target: summary.skipped[0].target,
      },
    };
  }

  const removedCount = removed.workspaces + removed.trustKeys + removed.dateDirs;
  if (removedCount === 0) {
    return null;
  }

  return { tone: 'success', key: keys.success, params: removed };
};

/** The minimum of antd's message API the reporting below needs. */
export interface CleanupMessageApi {
  success: (content: string) => void;
  warning: (content: string) => void;
}

/** A translate function for one cleanup message. */
export type CleanupTranslate = (key: string, params: Record<string, unknown>) => string;

/**
 * Report a cleanup outcome next to the result of the operation that asked for it.
 *
 * Every entry point calls this, so the same summary is described the same way
 * however the operation was started.
 */
export const reportCleanupOutcome = (
  message: CleanupMessageApi,
  t: CleanupTranslate,
  summary: CodexCleanupSummary | null | undefined,
  keys: CodexCleanupReportKeys,
): void => {
  const outcome = describeCleanupReport(summary, keys);
  if (!outcome) {
    return;
  }
  const text = t(outcome.key, outcome.params);
  if (outcome.tone === 'warning') {
    message.warning(text);
  } else {
    message.success(text);
  }
};