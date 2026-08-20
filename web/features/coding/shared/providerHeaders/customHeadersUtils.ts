/**
 * Provider-level custom request-header override meta read/merge helpers.
 *
 * Mirrors the billing-config / custom-UA pattern: each "feature subset" owns
 * its `get*FromMeta` / `merge*IntoMeta` pair and only touches its own keys,
 * so they can be chained in a form's submit `meta:` builder without
 * clobbering one another. `mergeGatewayProfileReferenceIntoMeta` uses a
 * delete whitelist that does not include `customHeaders`, so switching a
 * gateway profile preserves the user-entered header overrides.
 *
 * Persisted shape: `meta.customHeaders: CustomHeaderEntry[]`. The Rust gateway
 * parses the same array (`CustomHeaderOverride`) and applies each operation in
 * `build_upstream_headers` as the last injector — overrides win over every
 * preceding injector. See `tauri/.../proxy_gateway/types.rs` and
 * `.../runtime/upstream.rs::inject_custom_headers`.
 */
export type CustomHeaderOp = 'set' | 'delete' | 'rename' | 'copy';

export interface CustomHeaderEntry {
  op: CustomHeaderOp;
  /** Target header for `set`/`delete`. */
  name: string;
  /** Replacement value for `set`. */
  value: string;
  /** Source header for `rename`/`copy`. */
  from: string;
  /** Destination header for `rename`/`copy`. */
  to: string;
}

export interface CustomHeadersState {
  enabled: boolean;
  headers: CustomHeaderEntry[];
}

export interface GatewayProviderHeadersMeta {
  customHeaders?: CustomHeaderEntry[];
}

/** A fresh blank row defaulting to `set` (the most common op). */
export function emptyHeaderEntry(): CustomHeaderEntry {
  return { op: 'set', name: '', value: '', from: '', to: '' };
}

/**
 * Read the custom header overrides from provider meta into an editable state.
 * The state is `enabled` when any persisted row exists; an empty meta yields a
 * single blank row so the editor is immediately usable when toggled on.
 */
export function getCustomHeadersFromMeta(
  meta?: GatewayProviderHeadersMeta | null,
): CustomHeadersState {
  const headers = meta?.customHeaders ?? [];
  return {
    enabled: headers.length > 0,
    headers: headers.length > 0 ? headers.map(normalizeEntry) : [emptyHeaderEntry()],
  };
}

/** Merge the custom header state back into provider meta (field-level). */
export function mergeCustomHeadersIntoMeta<T extends GatewayProviderHeadersMeta>(
  meta: T | undefined,
  state: CustomHeadersState,
): T | undefined {
  const nextMeta = { ...(meta || {}) } as T;
  delete (nextMeta as Record<string, unknown>).customHeaders;

  if (state.enabled) {
    const meaningful = state.headers
      .map(trimEntry)
      .filter(isMeaningfulEntry);
    if (meaningful.length > 0) {
      (nextMeta as unknown as GatewayProviderHeadersMeta).customHeaders = meaningful;
    }
  }

  return hasMeaningfulMeta(nextMeta) ? nextMeta : undefined;
}

function normalizeEntry(entry: CustomHeaderEntry): CustomHeaderEntry {
  return {
    op: isCustomHeaderOp(entry.op) ? entry.op : 'set',
    name: entry.name ?? '',
    value: entry.value ?? '',
    from: entry.from ?? '',
    to: entry.to ?? '',
  };
}

function trimEntry(entry: CustomHeaderEntry): CustomHeaderEntry {
  return {
    op: entry.op,
    name: entry.name.trim(),
    value: entry.value.trim(),
    from: entry.from.trim(),
    to: entry.to.trim(),
  };
}

/** A row is meaningful when it has enough non-empty fields to act on at runtime. */
function isMeaningfulEntry(entry: CustomHeaderEntry): boolean {
  switch (entry.op) {
    case 'set':
      return entry.name !== '' && entry.value !== '';
    case 'delete':
      return entry.name !== '';
    case 'rename':
    case 'copy':
      return entry.from !== '' && entry.to !== '';
    default:
      return false;
  }
}

function isCustomHeaderOp(value: unknown): value is CustomHeaderOp {
  return value === 'set' || value === 'delete' || value === 'rename' || value === 'copy';
}

function hasMeaningfulMeta(meta: GatewayProviderHeadersMeta): boolean {
  return Object.values(meta).some(
    (value) => value !== undefined && value !== null && value !== '',
  );
}
