/**
 * Extract a human-readable reason from a rejected `invoke()` call.
 *
 * Tauri commands declared as `Result<T, String>` reject with the bare string, so
 * `error instanceof Error` is not enough: the raw-string case has to be handled
 * explicitly or every backend reason degrades into a generic UI message and the
 * user has nothing to report. Returns an empty string when nothing usable can be
 * extracted, so callers can skip the detail line instead of rendering
 * "undefined".
 */
export const formatUpdateFailureReason = (error: unknown): string => {
  if (typeof error === 'string') return error.trim();
  if (error instanceof Error) return error.message.trim();
  if (error && typeof error === 'object' && 'message' in error) {
    const { message } = error as { message?: unknown };
    if (typeof message === 'string') return message.trim();
  }
  return '';
};