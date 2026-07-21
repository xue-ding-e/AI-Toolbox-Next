/**
 * Monarch rule for invalid / unclosed double-quoted TOML strings to end-of-line.
 *
 * Do NOT use /"(\\.|[^"])*$/ — `\\.` and `[^"]` both consume backslashes, so a long
 * closed string full of `\` (e.g. Codex `notify` with Windows paths) triggers
 * catastrophic backtracking and freezes the WebView main thread.
 */
export const INVALID_UNCLOSED_DOUBLE_QUOTE_STRING_PATTERN = /"(?:\\.|[^"\\])*$/;
