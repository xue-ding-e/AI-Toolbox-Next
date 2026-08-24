/**
 * Flatten a multi-line SKILL.md description into a single line for compact UI
 * surfaces (skill cards, pick previews).
 *
 * YAML block scalars (`|` / `>`) keep newlines in the parsed value. On a
 * one-line card we collapse every run of whitespace-around-newline into a
 * single space so the description renders as a continuous line instead of
 * showing only the first line.
 */
export function flattenDescription(description: string | null | undefined): string {
  if (!description) {
    return '';
  }
  return description
    .replace(/\s*\n\s*/g, ' ')
    .trim();
}
