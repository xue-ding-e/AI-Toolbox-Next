import { createHash } from 'node:crypto';

export const SOURCE_PATH = 'tauri/src/lib.rs';

// A deliberately narrow Rust lexer, not a Rust type/schema parser. Preserve
// token offsets and reject unsupported handler-list syntax instead of skipping it.
export function rustTokens(source) {
  const tokens = [];
  let i = 0;
  const literal = (end) => { tokens.push({ value: '<literal>', offset: i }); i = end; };
  while (i < source.length) {
    if (/\s/.test(source[i])) { i++; continue; }
    if (source.startsWith('//', i)) {
      const end = source.indexOf('\n', i + 2);
      i = end < 0 ? source.length : end;
      continue;
    }
    if (source.startsWith('/*', i)) {
      let depth = 1;
      i += 2;
      while (i < source.length && depth) {
        if (source.startsWith('/*', i)) { depth++; i += 2; }
        else if (source.startsWith('*/', i)) { depth--; i += 2; }
        else i++;
      }
      if (depth) throw new Error('Unterminated Rust block comment');
      continue;
    }
    const raw = source.slice(i).match(/^(?:br|cr|r)(#*)"/);
    if (raw) {
      const close = `"${raw[1]}`;
      const end = source.indexOf(close, i + raw[0].length);
      if (end < 0) throw new Error('Unterminated Rust raw string');
      literal(end + close.length);
      continue;
    }
    const quoted = source.slice(i).match(/^(?:b|c)?"/);
    if (quoted) {
      let end = i + quoted[0].length;
      while (end < source.length && source[end] !== '"') {
        end += source[end] === '\\' ? 2 : 1;
      }
      if (end >= source.length) throw new Error('Unterminated Rust string');
      literal(end + 1);
      continue;
    }
    // Only a complete character literal is consumed; 'a and '_ lifetimes stay tokens.
    const character = source.slice(i).match(/^(?:b)?'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|[^\r\n])|[^'\\\r\n])'/u);
    if (character) { literal(i + character[0].length); continue; }
    const identifier = source.slice(i).match(/^(?:r#)?[A-Za-z_][A-Za-z_0-9]*/);
    const value = identifier?.[0] ?? (source.startsWith('::', i) ? '::' : source[i]);
    tokens.push({ value, offset: i });
    i += value.length;
  }
  return tokens;
}

export function extractHandlers(source) {
  const tokens = rustTokens(source);
  const opening = ['tauri', '::', 'generate_handler', '!', '['];
  const starts = tokens.flatMap((_, i) => opening.every((v, j) => tokens[i + j]?.value === v) ? [i + opening.length] : []);
  if (starts.length !== 1) throw new Error(`Expected exactly one tauri::generate_handler! registry, found ${starts.length}`);
  const handlers = [];
  const seen = new Set();
  let i = starts[0];
  const ident = (token) => token && /^(?:r#)?[A-Za-z_][A-Za-z_0-9]*$/.test(token.value);
  while (tokens[i]?.value !== ']') {
    const first = tokens[i];
    if (!ident(first)) throw new Error(`Unsupported handler entry at offset ${first?.offset ?? source.length}`);
    let handler = tokens[i++].value;
    while (tokens[i]?.value === '::') {
      i++;
      if (!ident(tokens[i])) throw new Error('Expected identifier after handler path separator');
      handler += `::${tokens[i++].value}`;
    }
    if (seen.has(handler)) throw new Error(`Duplicate handler: ${handler}`);
    seen.add(handler);
    handlers.push({
      source_id: `tauri:${handler}`,
      handler,
      path: SOURCE_PATH,
      line: source.slice(0, first.offset).split('\n').length,
      operation_id: null,
      classification: 'needs_review',
      cli_status: 'unimplemented',
      mcp_status: 'unimplemented',
    });
    if (tokens[i]?.value === ',') i++;
    else if (tokens[i]?.value !== ']') throw new Error(`Unsupported handler-list syntax after ${handler}`);
    if (!tokens[i]) throw new Error('Unterminated handler registry');
  }
  if (!handlers.length) throw new Error('Empty handler registry');
  return handlers.sort((a, b) => a.source_id < b.source_id ? -1 : a.source_id > b.source_id ? 1 : 0);
}

export function buildInventory(source) {
  const handlers = extractHandlers(source);
  return {
    schema_version: 1,
    generator_version: 1,
    source: { path: SOURCE_PATH, sha256: createHash('sha256').update(source).digest('hex') },
    scope: {
      handlers: 'complete_syntactic_inventory',
      service_calls: 'not_inventoried',
      tray: 'not_inventoried',
      background: 'not_inventoried',
      business_coverage: 'incomplete',
    },
    limitations: [
      'Registration paths only; function definitions, re-exports, cfg conditions and runtime availability are not resolved.',
      'No business operation IDs, CLI/MCP adapters, input/output schemas, effects or platform guarantees have been implemented or verified.',
      'Frontend service, tray and background entry points require separate inventory and semantic mapping; handler count is not a business-operation denominator.',
    ],
    unreviewed_contract: { input_schema: null, output_schema: null, effects: 'unknown', platforms: 'unknown', tests: 'not_run' },
    summary: { registered_handlers: handlers.length, classified: 0, cli_implemented: 0, mcp_implemented: 0, verified: 0 },
    handlers,
  };
}

export function serializeInventory(source) {
  return `${JSON.stringify(buildInventory(source), null, 2)}\n`;
}
