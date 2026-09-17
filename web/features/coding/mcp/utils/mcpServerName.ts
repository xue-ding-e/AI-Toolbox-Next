/**
 * Whether an MCP server name can be used by the DeepSeek Harness (dsh) adapter.
 *
 * dsh's MCP client (`@deepseek-ai/dsh-mcp-client`) constrains `serverName` to
 * `^[A-Za-z0-9_-]{1,32}$`. The name is the row's identity key, so a violating
 * value cannot be sanitized — dsh refuses to load that row.
 *
 * The authoritative guard is `validate_dsh_server_name` in
 * `tauri/src/coding/mcp/cordis_patch.rs`; this mirror only gives the add/edit
 * form early feedback, so changes belong in both places.
 */
export const MCP_SERVER_NAME_MAX_LENGTH = 32;
export const MCP_SERVER_NAME_PATTERN = new RegExp(
  `^[A-Za-z0-9_-]{1,${MCP_SERVER_NAME_MAX_LENGTH}}$`,
);

export const isMcpServerNameValidForDsh = (name: string): boolean =>
  MCP_SERVER_NAME_PATTERN.test(name);
