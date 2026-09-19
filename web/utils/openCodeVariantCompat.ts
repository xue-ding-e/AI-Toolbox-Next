/**
 * Variant compatibility between AI SDK packages for OpenCode providers.
 *
 * `thinkingConfig` is the `@ai-sdk/google` package's option and never reaches
 * an `@ai-sdk/openai-compatible` upstream: OpenCode 1.x masked the mismatch by
 * merging auto-generated `reasoningEffort` variants with the configured ones,
 * but OpenCode 2.x uses configured variants verbatim and drops `thinkingConfig`
 * on the OpenAI-compatible path, silently losing the thinking level
 * (`reasoning_effort` disappears from upstream requests).
 */

export const OPENAI_COMPATIBLE_NPM = '@ai-sdk/openai-compatible';

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

/**
 * Rewrite `thinkingConfig` variant options into `reasoningEffort` when the
 * target provider uses the OpenAI-compatible package.
 *
 * The effort level comes from `thinkingConfig.thinkingLevel` and falls back to
 * the variant name, which presets name after effort levels (low/high/max) —
 * budget-only variants keep their meaning through the name. Any other
 * Google-package fields inside `thinkingConfig` have no OpenAI-compatible
 * equivalent and are dropped; sibling options are preserved.
 *
 * Variants on other npm packages (including `@ai-sdk/google`, where
 * `thinkingConfig` is the correct spelling) are returned untouched.
 */
export const normalizeVariantsForProviderNpm = <T extends Record<string, unknown>>(
  variants: T | undefined | null,
  providerNpm?: string,
): T | undefined => {
  if (!variants || !isRecord(variants) || providerNpm !== OPENAI_COMPATIBLE_NPM) {
    return variants ?? undefined;
  }

  let changed = false;
  const converted = Object.fromEntries(
    Object.entries(variants).map(([variantName, options]) => {
      if (!isRecord(options) || !isRecord(options.thinkingConfig)) {
        return [variantName, options];
      }
      const level =
        (typeof options.thinkingConfig.thinkingLevel === 'string'
          && options.thinkingConfig.thinkingLevel.trim()) || variantName;
      const { thinkingConfig: _dropped, ...rest } = options;
      changed = true;
      return [variantName, { ...rest, reasoningEffort: level }];
    }),
  );

  return changed ? (converted as T) : variants;
};
