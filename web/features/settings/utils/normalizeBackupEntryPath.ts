/** Keep empty optional paths local; all nonempty paths use the shared backend. */
export async function normalizeBackupEntryPath(
  path: string,
  normalize: (path: string) => Promise<string>,
): Promise<string> {
  const trimmedPath = path.trim();
  if (!trimmedPath) return '';
  // A rejected service result must abort the caller's save, not bypass its limits.
  return normalize(trimmedPath);
}
