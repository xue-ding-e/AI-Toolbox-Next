/**
 * Custom request-header override validity checks.
 *
 * Byte-aligned with the Rust gateway's `parse_header_override_value` (based
 * on `http::HeaderValue::from_str`) and `HeaderName::from_bytes`:
 *
 * - Header **value**: a byte is legal when `b >= 32 && b != 127 || b == '\t'`.
 *   Concretely tab, visible ASCII (0x20–0x7E), and any non-ASCII byte (UTF-8
 *   bytes are all >= 0x80) are legal; only control chars — every 0x00–0x1F
 *   except `\t`, plus 0x7F (DEL) — are illegal. An empty/whitespace value is
 *   treated as "unset" and is valid (silently skipped at runtime).
 * - Header **name**: RFC 7230 `token` — `A-Za-z0-9` and
 *   `!#$%&'*+-.^_`|~`; empty is invalid.
 *
 * Invalid values are silently ignored at runtime; these helpers power the
 * frontend's non-blocking red hints.
 */

// eslint-disable-next-line no-control-regex
const INVALID_VALUE_CHARS = /[\x00-\x08\x0a-\x1f\x7f]/;
const VALID_NAME_CHARS = /^[A-Za-z0-9!#$%&'*+.^_`|~-]+$/;

export function isValidHeaderValue(value: string): boolean {
  const trimmed = value.trim();
  if (trimmed === '') return true;
  return !INVALID_VALUE_CHARS.test(trimmed);
}

export function isValidHeaderName(name: string): boolean {
  const trimmed = name.trim();
  if (trimmed === '') return false;
  return VALID_NAME_CHARS.test(trimmed);
}

export interface HeaderEntryValidation {
  /** Whether the row has enough non-empty fields to act on at runtime. */
  meaningful: boolean;
  /** Whether every filled field is byte-legal. */
  valid: boolean;
}

/** Validate a single override row, op-aware. */
export function validateHeaderEntry(entry: {
  op: string;
  name: string;
  value: string;
  from: string;
  to: string;
}): HeaderEntryValidation {
  const name = entry.name.trim();
  const value = entry.value.trim();
  const from = entry.from.trim();
  const to = entry.to.trim();

  switch (entry.op) {
    case 'set': {
      const meaningful = name !== '' && value !== '';
      const valid = isValidHeaderName(name) && isValidHeaderValue(value);
      return { meaningful, valid };
    }
    case 'delete': {
      const meaningful = name !== '';
      const valid = isValidHeaderName(name);
      return { meaningful, valid };
    }
    case 'rename':
    case 'copy': {
      const meaningful = from !== '' && to !== '';
      const valid = isValidHeaderName(from) && isValidHeaderName(to);
      return { meaningful, valid };
    }
    default:
      return { meaningful: false, valid: false };
  }
}
