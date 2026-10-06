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
 * Validation messages mirror the CLI's own schema errors so the user sees the
 * same rule the runtime enforces.
 */
export function validateKimiSecondaryModelConfig(
  config: KimiSecondaryModelConfig,
): string | null {
  const defaultModel = config.defaultModel.trim();
  const models = config.models.map((model) => model.trim()).filter(Boolean);

  if (config.force && models.length > 0) {
    return 'force cannot be combined with models: the pool only exists to offer the main agent a choice';
  }
  if (config.force && !defaultModel) {
    return 'default_model is required when force is set';
  }
  if (models.length > 0 && !defaultModel) {
    return 'default_model is required when models is configured';
  }
  if (models.some((model) => model === KIMI_SECONDARY_MODEL_PRIMARY_KEY)) {
    return `models key "${KIMI_SECONDARY_MODEL_PRIMARY_KEY}" is reserved`;
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
 * Replace (or drop) the `[secondary_model]` section in a common-config TOML
 * payload, leaving every other line byte-identical.
 *
 * A line-based splice is deliberate: re-serializing the whole document would
 * reorder keys and strip the user's comments from the free TOML editor. The
 * section runs from its `[secondary_model]` header to the next table header at
 * the same or a shallower level, including the `[secondary_model.models]`
 * sub-table.
 */
export function applyKimiSecondaryModelToml(
  toml: string,
  config: KimiSecondaryModelConfig,
): string {
  const lines = toml.split('\n');
  const kept: string[] = [];
  let skipping = false;

  for (const line of lines) {
    const header = /^\s*\[\[?\s*([^\]]+?)\s*\]?\]\s*$/.exec(line);
    if (header) {
      const path = header[1].trim();
      // Enter the section on its own header or any sub-table of it.
      skipping = path === KIMI_SECONDARY_MODEL_SECTION
        || path.startsWith(`${KIMI_SECONDARY_MODEL_SECTION}.`);
      if (skipping) {
        continue;
      }
    } else if (skipping) {
      continue;
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
