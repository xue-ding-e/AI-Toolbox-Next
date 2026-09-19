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
 * Reasoning effort spellings that OpenCode and AI Toolbox understand. Anything
 * else has no `reasoningEffort` equivalent and must not be invented.
 */
const canonicalReasoningEffort = (value: string): string | undefined => {
  const normalized = value.trim().toLowerCase();
  switch (normalized) {
    case 'none':
    case 'off':
    case 'disabled':
      return 'none';
    case 'minimal':
    case 'min':
      return 'minimal';
    case 'low':
    case 'medium':
    case 'high':
    case 'xhigh':
    case 'max':
      return normalized;
    default:
      return undefined;
  }
};

/**
 * Gemini thinking budgets only partly map onto `reasoningEffort`: `0` disables
 * thinking, positive budgets use the same thresholds as the gateway
 * transformer, and negative budgets mean dynamic thinking, which has no
 * equivalent and must stay unset.
 */
const reasoningEffortFromThinkingBudget = (budget: number): string | undefined => {
  if (budget === 0) return 'none';
  if (budget < 0) return undefined;
  if (budget <= 1024) return 'minimal';
  if (budget <= 4096) return 'low';
  if (budget <= 10240) return 'medium';
  if (budget <= 32768) return 'high';
  return 'xhigh';
};

/**
 * Effort level implied by one variant, preferring explicit labels
 * (`thinkingLevel`, then the variant name the user picks in the UI) over the
 * derived thinking budget. Returns undefined when nothing equivalent can be
 * derived, so callers keep the variant untouched instead of inventing a value.
 */
const variantReasoningEffort = (
  variantName: string,
  thinking: Record<string, unknown>,
): string | undefined => {
  if (typeof thinking.thinkingLevel === 'string') {
    const fromLevel = canonicalReasoningEffort(thinking.thinkingLevel);
    if (fromLevel) return fromLevel;
  }
  const fromName = canonicalReasoningEffort(variantName);
  if (fromName) return fromName;
  return typeof thinking.thinkingBudget === 'number'
    ? reasoningEffortFromThinkingBudget(thinking.thinkingBudget)
    : undefined;
};

/**
 * Rewrite `thinkingConfig` variant options into `reasoningEffort` when the
 * target provider uses the OpenAI-compatible package.
 *
 * Any other Google-package fields inside `thinkingConfig` have no
 * OpenAI-compatible equivalent and are dropped; sibling options are preserved.
 * Variants whose effort cannot be derived keep `thinkingConfig` untouched so no
 * invalid `reasoningEffort` value (for example a budget-only `auto` variant) is
 * ever written.
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
      const level = variantReasoningEffort(variantName, options.thinkingConfig);
      if (!level) {
        return [variantName, options];
      }
      const { thinkingConfig: _dropped, ...rest } = options;
      changed = true;
      return [variantName, { ...rest, reasoningEffort: level }];
    }),
  );

  return changed ? (converted as T) : variants;
};
