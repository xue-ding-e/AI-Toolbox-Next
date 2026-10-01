/**
 * Shared rule for editor surfaces that only own a slice of a config file.
 *
 * Such an editor must never rebuild the whole file from a long-lived page
 * snapshot: the fields it hides are written by other surfaces — the MCP page,
 * the tray, deep-link imports, backup restores — and those write the file
 * directly. Whenever the file can still be read, it wins over the in-memory
 * copy. Merging onto a stale copy silently reverted whatever another surface
 * had just written, i.e. a newly added MCP server disappearing from the config
 * file (issue #406, OpenCode; same shape in OpenClaw).
 */

/** Minimal shape shared by the read commands (`read_*_config`). */
export interface ConfigReadResult<TConfig> {
  status: 'success' | 'notFound' | 'parseError' | 'error';
  config?: TConfig;
}

export const pickConfigSaveBase = <TConfig>(
  readResult: ConfigReadResult<TConfig> | null | undefined,
  fallback: TConfig | null | undefined,
): TConfig | null => (
  readResult?.status === 'success' && readResult.config ? readResult.config : (fallback ?? null)
);
