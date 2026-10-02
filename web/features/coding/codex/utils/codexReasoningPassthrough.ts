/**
 * Codex provider-form reasoning-effort passthrough (issue #412).
 *
 * Codex speaks OpenAI Responses natively. When its upstream is a *custom*
 * OpenAI Chat provider, the gateway converts the request and the generic
 * third-party Chat cleanup drops the top-level `reasoning_effort` unless the
 * provider declares a `codexChatReasoning` dialect. A custom provider has no
 * such profile dialect, so the form exposes an explicit opt-in that persists
 * `meta.preserveReasoningEffort`.
 *
 * The switch only has meaning for an `openai_chat` target: `openai_responses`
 * is a same-protocol passthrough that already carries the effort verbatim, and
 * `anthropic_messages` / `gemini_native` map the effort into their own thinking
 * budget instead of this field.
 *
 * Kept free of runtime `@/` alias and JSON imports so the node test loader can
 * load it directly (see `web-test-harness-constraints`).
 */

export interface CodexReasoningPassthroughMeta {
  preserveReasoningEffort?: unknown;
  preserve_reasoning_effort?: unknown;
}

export const codexReasoningPassthroughApplies = (params: {
  isOfficial: boolean;
  isCustomProvider: boolean;
  apiFormat?: string | null;
}): boolean =>
  !params.isOfficial &&
  params.isCustomProvider &&
  params.apiFormat === 'openai_chat';

/**
 * The switch defaults to ON: a custom Chat provider keeps the client's effort
 * unless the user explicitly turned it off (`preserveReasoningEffort: false`).
 */
export const readCodexPreserveReasoningEffort = (
  meta: CodexReasoningPassthroughMeta | undefined,
): boolean =>
  meta?.preserveReasoningEffort !== false &&
  meta?.preserve_reasoning_effort !== false;

/**
 * Writes the opt-in onto provider meta when the switch applies, and clears any
 * stale value otherwise (official / profile-endpoint / non-Chat targets), so a
 * profile-owned reasoning dialect is never overridden by a leftover flag.
 */
export const withCodexPreserveReasoningEffort = <T extends object>(
  meta: T | undefined,
  enabled: boolean,
  applies: boolean,
): T | undefined => {
  const nextMeta = { ...(meta || {}) } as Record<string, unknown>;
  delete nextMeta.preserveReasoningEffort;
  delete nextMeta.preserve_reasoning_effort;
  if (applies) {
    nextMeta.preserveReasoningEffort = enabled;
  }
  return Object.keys(nextMeta).length > 0 ? (nextMeta as T) : undefined;
};