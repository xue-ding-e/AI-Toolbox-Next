import type { CodexMemoriesSourceMode, CodexMemoriesSourceOption } from '@/types/codex';

export const resolveMemoriesSourceMode = (
  sourceMode: CodexMemoriesSourceMode,
  availableSources?: readonly CodexMemoriesSourceOption[],
): CodexMemoriesSourceMode => {
  if (
    sourceMode === 'local' &&
    availableSources?.some((option) => option.source === 'wsl') &&
    !availableSources.some((option) => option.source === 'local')
  ) {
    return 'wsl';
  }
  return sourceMode;
};
