export const hasCompleteModelLimitPair = (
  contextLimit: number | undefined,
  outputLimit: number | undefined,
): boolean => (contextLimit === undefined) === (outputLimit === undefined);

/**
 * Renders a token limit the way a reader scans it: `200K`, `1M`, `1.5M`.
 *
 * Deliberately coarse. The number sits beside the model id on a list row, where
 * the question is "is this one bigger than that one" — not "how many tokens
 * exactly". So below a thousand the value is shown as-is (a small custom model
 * has no meaningful unit), thousands round to whole K, and millions keep one
 * decimal only when there is one.
 *
 * Mirrors ZCode's own `formatContextWindow`, including that rounding rule: its
 * `8.2K`-style output would suggest a precision the row does not carry.
 */
export const formatModelLimit = (value: number | undefined): string | undefined => {
  if (value === undefined || !Number.isFinite(value) || value < 0) {
    return undefined;
  }
  if (value < 1_000) {
    return String(value);
  }
  if (value < 1_000_000) {
    return `${Math.round(value / 1_000)}K`;
  }
  const millions = value / 1_000_000;
  return Number.isInteger(millions) ? `${millions}M` : `${millions.toFixed(1)}M`;
};
