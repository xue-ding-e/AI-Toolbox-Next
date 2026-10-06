import { parse as parseToml } from 'smol-toml';

/**
 * `[secondary_model]` — the Kimi Code CLI's subagent (swarm) model pool.
 *
 * The main agent picks a model for each subagent from `models`; `force` pins
 * every subagent to `default_model` and removes that choice, so the CLI rejects
 * `force` combined with `models`. The key `"primary"` is reserved (it always
 * means the caller's own model) and `default_model` is required whenever
 * `models` is configured.
 */
export interface KimiSecondaryModelConfig {
  defaultModel: string;
  /** Alias keys the main agent may choose from. */
  models: string[];
  /** Pin subagents to `defaultModel` instead of offering the pool. */
  force: boolean;
}

export const KIMI_SECONDARY_MODEL_SECTION = 'secondary_model';
/** Reserved `models` key: always binds the caller's own model. */
export const KIMI_SECONDARY_MODEL_PRIMARY_KEY = 'primary';

export function emptyKimiSecondaryModelConfig(): KimiSecondaryModelConfig {
  return { defaultModel: '', models: [], force: false };
}

/**
 * Validation failures, as i18n keys rather than prose: the caller renders them
 * through i18next so zh-CN users see translated text.
 */
export type KimiSecondaryModelValidationError =
  | 'forceWithModels'
  | 'forceWithoutDefault'
  | 'modelsWithoutDefault'
  | 'reservedPrimaryKey'
  | 'unresolvableAlias';

/**
 * Validation mirrors the CLI's own schema rules so the user sees the same rule
 * the runtime enforces.
 *
 * `resolvableKeys` is the set of `[models.<key>]` aliases the applied provider
 * projects; a pool key that resolves to no table makes the CLI refuse to start
 * a session. Pass `undefined` when the applied catalog is unknown — the
 * resolvability check is skipped rather than blocking a save on missing data.
 */
export function validateKimiSecondaryModelConfig(
  config: KimiSecondaryModelConfig,
  resolvableKeys?: string[],
): KimiSecondaryModelValidationError | null {
  const defaultModel = config.defaultModel.trim();
  const models = config.models.map((model) => model.trim()).filter(Boolean);

  if (config.force && models.length > 0) {
    return 'forceWithModels';
  }
  if (config.force && !defaultModel) {
    return 'forceWithoutDefault';
  }
  if (models.length > 0 && !defaultModel) {
    return 'modelsWithoutDefault';
  }
  if (models.some((model) => model === KIMI_SECONDARY_MODEL_PRIMARY_KEY)) {
    return 'reservedPrimaryKey';
  }
  if (resolvableKeys) {
    const known = new Set(resolvableKeys.map((key) => key.trim()).filter(Boolean));
    // `primary` is legal anywhere it appears as a *value*; it is only rejected
    // as a pool key above.
    const unresolvable = [defaultModel, ...models].filter(
      (key) => key && key !== KIMI_SECONDARY_MODEL_PRIMARY_KEY && !known.has(key),
    );
    if (unresolvable.length > 0) {
      return 'unresolvableAlias';
    }
  }
  return null;
}

/** Whether the section declares anything worth writing. */
export function isKimiSecondaryModelConfigEmpty(config: KimiSecondaryModelConfig): boolean {
  return !config.defaultModel.trim() && config.models.length === 0 && !config.force;
}

/**
 * Read `[secondary_model]` out of a stored common-config TOML payload.
 *
 * The section may legitimately be absent (the CLI runs without a pool), so a
 * missing or malformed section yields the empty config rather than an error —
 * the free TOML editor stays the source of truth for anything this form does
 * not understand.
 */
export function parseKimiSecondaryModelConfig(toml: string): KimiSecondaryModelConfig {
  if (!toml.trim()) {
    return emptyKimiSecondaryModelConfig();
  }
  let document: unknown;
  try {
    document = parseToml(toml);
  } catch {
    return emptyKimiSecondaryModelConfig();
  }
  if (!document || typeof document !== 'object') {
    return emptyKimiSecondaryModelConfig();
  }
  const section = (document as Record<string, unknown>)[KIMI_SECONDARY_MODEL_SECTION];
  if (!section || typeof section !== 'object' || Array.isArray(section)) {
    return emptyKimiSecondaryModelConfig();
  }
  const record = section as Record<string, unknown>;

  // `model` is the deprecated v1 key the CLI still honors as a fallback default.
  const rawDefault = typeof record.default_model === 'string'
    ? record.default_model
    : typeof record.model === 'string'
      ? record.model
      : '';

  const rawModels = record.models;
  const models = rawModels && typeof rawModels === 'object' && !Array.isArray(rawModels)
    ? Object.keys(rawModels as Record<string, unknown>)
    : [];

  return {
    defaultModel: rawDefault.trim(),
    models,
    force: record.force === true,
  };
}

/**
 * Render `[secondary_model]` into a TOML fragment.
 *
 * Returns an empty string for an empty config so callers can drop the section
 * entirely (disabling the pool must remove it, not write an empty table).
 * `models` values are always empty strings — the CLI only reads the keys.
 */
export function buildKimiSecondaryModelToml(config: KimiSecondaryModelConfig): string {
  if (isKimiSecondaryModelConfigEmpty(config)) {
    return '';
  }
  const lines: string[] = [`[${KIMI_SECONDARY_MODEL_SECTION}]`];
  const defaultModel = config.defaultModel.trim();
  if (defaultModel) {
    lines.push(`default_model = ${JSON.stringify(defaultModel)}`);
  }
  if (config.force) {
    lines.push('force = true');
  }
  const models = config.models.map((model) => model.trim()).filter(Boolean);
  if (models.length > 0) {
    lines.push('');
    lines.push(`[${KIMI_SECONDARY_MODEL_SECTION}.models]`);
    for (const model of models) {
      lines.push(`${JSON.stringify(model)} = ""`);
    }
  }
  return `${lines.join('\n')}\n`;
}

/**
 * A table header at the start of a line: `[name]` / `[[name]]`, optionally
 * quoted, optionally followed by a comment. Trailing content after the closing
 * bracket is legal TOML, so the match must not require end-of-line.
 */
const TABLE_HEADER_PATTERN = /^\s*\[\[?\s*(.+?)\s*\]?\]\s*(?:#.*)?$/;

/** Strip surrounding quotes from a TOML key path segment. */
function unquoteKeyPath(raw: string): string {
  return raw
    .split('.')
    .map((segment) => {
      const trimmed = segment.trim();
      if (
        (trimmed.startsWith('"') && trimmed.endsWith('"') && trimmed.length >= 2)
        || (trimmed.startsWith("'") && trimmed.endsWith("'") && trimmed.length >= 2)
      ) {
        return trimmed.slice(1, -1);
      }
      return trimmed;
    })
    .join('.');
}

/** Whether a header path is `[secondary_model]` or one of its sub-tables. */
function isSecondaryModelPath(path: string): boolean {
  const normalized = unquoteKeyPath(path);
  return normalized === KIMI_SECONDARY_MODEL_SECTION
    || normalized.startsWith(`${KIMI_SECONDARY_MODEL_SECTION}.`);
}

/** A dotted top-level key that assigns into the section without a header. */
const DOTTED_SECTION_PATTERN = new RegExp(
  `^\\s*(?:"${KIMI_SECONDARY_MODEL_SECTION}"|'${KIMI_SECONDARY_MODEL_SECTION}'|${KIMI_SECONDARY_MODEL_SECTION})\\s*\\.`,
);

/**
 * Replace (or drop) the `[secondary_model]` section in a common-config TOML
 * payload, leaving every other line byte-identical.
 *
 * A line-based splice is deliberate: re-serializing the whole document would
 * reorder keys and strip the user's comments from the free TOML editor. In
 * TOML a table body runs until the next table header, so the section is the
 * run of lines from its header to the next one — including the
 * `[secondary_model.models]` sub-table.
 *
 * Every legal spelling of the header must be recognized (a quoted
 * `["secondary_model"]`, a trailing comment, the dotted form
 * `secondary_model.default_model = "x"`). Missing one would leave the old table
 * in place while appending a fresh one, and the duplicate table makes the whole
 * config unparseable — so it could never be saved again. Multi-line strings are
 * tracked so a `[secondary_model]` line inside one is not read as a header.
 */
export function applyKimiSecondaryModelToml(
  toml: string,
  config: KimiSecondaryModelConfig,
): string {
  const kept: string[] = [];
  let skipping = false;
  let multilineDelimiter: '"""' | "'''" | null = null;

  for (const line of toml.split('\n')) {
    // Inside a multi-line string only the closing delimiter is meaningful.
    if (multilineDelimiter) {
      if (line.includes(multilineDelimiter)) {
        multilineDelimiter = null;
      }
      if (!skipping) {
        kept.push(line);
      }
      continue;
    }

    const header = TABLE_HEADER_PATTERN.exec(line);
    if (header) {
      skipping = isSecondaryModelPath(header[1]);
      if (skipping) {
        continue;
      }
      kept.push(line);
      continue;
    }

    if (skipping) {
      continue;
    }
    if (DOTTED_SECTION_PATTERN.test(line)) {
      continue;
    }

    // Track entry into a multi-line string so its body is never parsed as TOML.
    if ((line.match(/"""/g) ?? []).length % 2 === 1) {
      multilineDelimiter = '"""';
    } else if ((line.match(/'''/g) ?? []).length % 2 === 1) {
      multilineDelimiter = "'''";
    }
    kept.push(line);
  }

  // Drop the blank lines the removed section left behind, then append the new
  // one so the result stays stable across repeated edits.
  while (kept.length > 0 && kept[kept.length - 1].trim() === '') {
    kept.pop();
  }
  const body = kept.join('\n');
  const rendered = buildKimiSecondaryModelToml(config);
  if (!rendered) {
    return body ? `${body}\n` : '';
  }
  return body ? `${body}\n\n${rendered}` : rendered;
}
