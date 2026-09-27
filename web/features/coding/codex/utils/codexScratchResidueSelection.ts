import type {
  CodexScratchCleanResult,
  CodexScratchResidue,
  CodexScratchResidueWorkspace,
} from '@/types/codex';
// A relative specifier, not the `@/` alias: this module is exercised by the unit
// tests, whose loader only resolves relative paths.
import {
  describeCleanupReport,
  isRemovableWorkspace,
} from '../../shared/sessionManager/codexCleanupPolicy';

/** The residue items the user selected for removal. */
export interface CodexScratchResidueSelection {
  workspaces: string[];
  trustKeys: string[];
  dateDirs: string[];
}

export type CodexScratchResidueGroup = 'workspace' | 'trustKey' | 'dateDir';

export const emptyResidueSelection = (): CodexScratchResidueSelection => ({
  workspaces: [],
  trustKeys: [],
  dateDirs: [],
});

/**
 * A workspace the backend would refuse to remove, so it is not offered.
 *
 * The rule is the delete confirmation's rule, not a second copy of it: what one
 * surface is willing to delete, the other must not treat as deletable.
 */
export const isWorkspaceRemovable = (workspace: CodexScratchResidueWorkspace): boolean =>
  isRemovableWorkspace(workspace.info);

/**
 * The preselection the dialog opens with.
 *
 * Empty workspaces, trust entries and empty dated directories hold nothing, so
 * they are preselected; a workspace with files is left to the user, and one
 * holding a git repository is never offered.
 */
export const defaultResidueSelection = (
  report: CodexScratchResidue | null | undefined,
): CodexScratchResidueSelection => {
  if (!report) {
    return emptyResidueSelection();
  }

  return {
    workspaces: report.workspaces
      .filter((workspace) => isWorkspaceRemovable(workspace) && workspace.info.isEmpty)
      .map((workspace) => workspace.info.path),
    trustKeys: report.trustEntries.map((entry) => entry.key),
    dateDirs: [...report.emptyDateDirs],
  };
};

const valuesOf = (
  selection: CodexScratchResidueSelection,
  group: CodexScratchResidueGroup,
): string[] => {
  switch (group) {
    case 'workspace':
      return selection.workspaces;
    case 'trustKey':
      return selection.trustKeys;
    case 'dateDir':
      return selection.dateDirs;
  }
};

const withValues = (
  selection: CodexScratchResidueSelection,
  group: CodexScratchResidueGroup,
  values: string[],
): CodexScratchResidueSelection => {
  switch (group) {
    case 'workspace':
      return { ...selection, workspaces: values };
    case 'trustKey':
      return { ...selection, trustKeys: values };
    case 'dateDir':
      return { ...selection, dateDirs: values };
  }
};

export const toggleResidueSelection = (
  selection: CodexScratchResidueSelection,
  group: CodexScratchResidueGroup,
  value: string,
  checked: boolean,
): CodexScratchResidueSelection => {
  const values = valuesOf(selection, group);
  const next = checked
    ? (values.includes(value) ? values : [...values, value])
    : values.filter((item) => item !== value);
  return withValues(selection, group, next);
};

export const setResidueGroupSelection = (
  selection: CodexScratchResidueSelection,
  group: CodexScratchResidueGroup,
  values: string[],
): CodexScratchResidueSelection => withValues(selection, group, [...values]);

export const residueSelectionCount = (selection: CodexScratchResidueSelection): number =>
  selection.workspaces.length + selection.trustKeys.length + selection.dateDirs.length;

export const isResidueSelectionEmpty = (selection: CodexScratchResidueSelection): boolean =>
  residueSelectionCount(selection) === 0;

export interface CodexScratchResidueSummary {
  workspaceCount: number;
  nonEmptyWorkspaceCount: number;
  trustKeyCount: number;
  dateDirCount: number;
}

/** What the confirmation dialog reports about the selection. */
export const summarizeResidueSelection = (
  selection: CodexScratchResidueSelection,
  report: CodexScratchResidue,
): CodexScratchResidueSummary => {
  const selectedWorkspaces = report.workspaces.filter((workspace) =>
    selection.workspaces.includes(workspace.info.path),
  );

  return {
    workspaceCount: selection.workspaces.length,
    nonEmptyWorkspaceCount: selectedWorkspaces.filter((workspace) => !workspace.info.isEmpty)
      .length,
    trustKeyCount: selection.trustKeys.length,
    dateDirCount: selection.dateDirs.length,
  };
};

export interface CodexScratchResidueOutcome {
  tone: 'success' | 'warning';
  key: string;
  params: Record<string, unknown>;
}

/** What to report after a cleanup run, or `null` when nothing happened. */
export const describeResidueCleanOutcome = (
  result: CodexScratchCleanResult | null | undefined,
): CodexScratchResidueOutcome | null =>
  describeCleanupReport(result, {
    failed: 'codex.scratchResidue.cleanFailed',
    skipped: 'codex.scratchResidue.cleanSkipped',
    success: 'codex.scratchResidue.cleanSuccess',
  });