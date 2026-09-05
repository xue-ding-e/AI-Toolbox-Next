/**
 * Provider-level model rewrite meta read/merge helpers (issue #321).
 *
 * Mirrors the custom-headers pattern: this feature subset owns its
 * `getModelRewritesFromMeta` / `mergeModelRewritesIntoMeta` pair and only
 * touches its own keys, so it can be chained in a form's submit `meta:`
 * builder without clobbering other subsets.
 * `mergeGatewayProfileReferenceIntoMeta` does not delete `modelRewrites`,
 * so switching a gateway profile preserves the user's rewrite rules.
 *
 * Persisted shape: `meta.modelRewrites: ModelRewriteEntry[]`. The Rust
 * gateway parses the same array (`ModelRewriteRule`) and rewrites the
 * request model by exact, trim + case-insensitive match after stripping
 * the `[1M]` context marker, in every proxy mode (connectivity tests keep
 * the pinned model). See `tauri/.../proxy_gateway/types.rs` and
 * `.../runtime/upstream.rs::resolve_upstream_model_id`.
 */
export interface ModelRewriteEntry {
  /** Requested model to match (exact, trimmed, case-insensitive). */
  from: string;
  /** Upstream model forwarded instead. */
  to: string;
}

export interface ModelRewritesState {
  enabled: boolean;
  rewrites: ModelRewriteEntry[];
}

export interface GatewayProviderModelRewritesMeta {
  modelRewrites?: ModelRewriteEntry[];
  /** Legacy snake_case variant, kept for parity with the Rust adapter. */
  model_rewrites?: ModelRewriteEntry[];
}

/** A fresh blank rewrite row. */
export function emptyModelRewriteEntry(): ModelRewriteEntry {
  return { from: '', to: '' };
}

/**
 * Read the model rewrite rules from provider meta into an editable state.
 * The state is `enabled` when any persisted row exists; an empty meta yields
 * a single blank row so the editor is immediately usable when toggled on.
 */
export function getModelRewritesFromMeta(
  meta?: GatewayProviderModelRewritesMeta | null,
): ModelRewritesState {
  const rawRewrites = Array.isArray(meta?.modelRewrites)
    ? meta.modelRewrites
    : Array.isArray(meta?.model_rewrites)
      ? meta.model_rewrites
      : [];
  const rewrites = rawRewrites.length > 0 ? rawRewrites.map(normalizeEntry) : [];
  return {
    enabled: rewrites.length > 0,
    rewrites: rewrites.length > 0 ? rewrites : [emptyModelRewriteEntry()],
  };
}

/** Merge the model rewrite state back into provider meta (field-level). */
export function mergeModelRewritesIntoMeta<T extends GatewayProviderModelRewritesMeta>(
  meta: T | undefined,
  state: ModelRewritesState,
): T | undefined {
  const nextMeta = { ...(meta || {}) } as T;
  const record = nextMeta as unknown as Record<string, unknown>;
  delete record.modelRewrites;
  delete record.model_rewrites;

  if (state.enabled) {
    const meaningful = state.rewrites
      .map((entry) => ({ from: entry.from.trim(), to: entry.to.trim() }))
      .filter((entry) => entry.from !== '' && entry.to !== '');
    if (meaningful.length > 0) {
      record.modelRewrites = meaningful;
    }
  }

  return hasMeaningfulMeta(nextMeta) ? nextMeta : undefined;
}

function normalizeEntry(entry: ModelRewriteEntry): ModelRewriteEntry {
  return {
    from: entry.from ?? '',
    to: entry.to ?? '',
  };
}

function hasMeaningfulMeta(meta: GatewayProviderModelRewritesMeta): boolean {
  return Object.values(meta).some((value) => {
    if (value === undefined || value === null || value === '') return false;
    if (Array.isArray(value)) return value.length > 0;
    return true;
  });
}
