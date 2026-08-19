/**
 * Custom User-Agent validity check.
 *
 * Byte-aligned with the Rust gateway's `parse_custom_user_agent` (based on
 * `http::HeaderValue::from_str`): a byte is legal when `b >= 32 && b != 127 ||
 * b == '\t'`. Concretely: tab, visible ASCII (0x20–0x7E), and any non-ASCII
 * byte (UTF-8 bytes are all >= 0x80) are legal; only control chars — every
 * 0x00–0x1F except `\t`, plus 0x7F (DEL) — are illegal. An empty/whitespace
 * string is treated as "unset" and is valid. Invalid values are silently
 * ignored at runtime; this helper powers the frontend's non-blocking red hint.
 */
export function isValidUserAgentHeader(value: string): boolean {
  const trimmed = value.trim();
  if (trimmed === '') return true;
  // eslint-disable-next-line no-control-regex
  return !/[\x00-\x08\x0a-\x1f\x7f]/.test(trimmed);
}
