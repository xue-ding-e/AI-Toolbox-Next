/** Back up the whole selection before the first destructive write. */
export async function backupProvidersBeforeDelete<T>(
  providers: readonly T[],
  backupProvider: (provider: T) => Promise<unknown>,
  getFailureMessage: (provider: T) => string,
): Promise<void> {
  for (const provider of providers) {
    try {
      await backupProvider(provider);
    } catch (error) {
      console.error('Failed to back up provider before batch deletion:', error);
      throw new Error(getFailureMessage(provider));
    }
  }
}

export function reconcileProviderSelection<K extends string>(
  selectedIds: Set<K>,
  selectableIds: ReadonlySet<K>,
): Set<K> {
  const remainingIds = [...selectedIds].filter((id) => selectableIds.has(id));
  return remainingIds.length === selectedIds.size ? selectedIds : new Set(remainingIds);
}
