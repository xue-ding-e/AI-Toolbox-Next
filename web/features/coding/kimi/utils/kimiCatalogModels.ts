import type { KimiCatalogModel } from '@/types/kimi';

/**
 * Capabilities the Kimi CLI understands. Values outside this set are dropped
 * when the model catalog is projected into `config.toml`.
 *
 * `thinking` lets the user disable reasoning; `always_thinking` marks a model
 * that always reasons (the CLI adds one of the two automatically for known
 * Anthropic profiles). `*_in` entries declare input modalities.
 */
export const KIMI_SUPPORTED_CAPABILITIES = [
  'image_in',
  'video_in',
  'audio_in',
  'thinking',
  'always_thinking',
  'tool_use',
] as const;

/**
 * Thinking efforts the Kimi CLI ships profiles for. `support_efforts` is free
 * text in the schema, but the built-in Anthropic profiles only declare these
 * four, so the picker offers exactly this set.
 */
export const KIMI_SUPPORTED_EFFORTS = ['low', 'medium', 'high', 'max'] as const;

/**
 * Row identity for the provider card's model list.
 *
 * A catalog row is identified by its alias key: the `[models."<key>"]` table
 * name in `config.toml`. Unlike Codex (which dedups on model+displayName), Kimi
 * keys are unique per entry, so the key alone is the row identity.
 */
export function kimiCatalogRowKey(item: Pick<KimiCatalogModel, 'key'>): string {
  return item.key.trim();
}

export function findKimiCatalogRowIndex(models: KimiCatalogModel[], rowKey: string): number {
  return models.findIndex((item) => kimiCatalogRowKey(item) === rowKey);
}

/** Replace the row behind `previousRowKey`, or append when it is gone. */
export function upsertKimiCatalogModel(
  models: KimiCatalogModel[],
  next: KimiCatalogModel,
  previousRowKey?: string,
): KimiCatalogModel[] {
  const index = previousRowKey ? findKimiCatalogRowIndex(models, previousRowKey) : -1;
  if (index < 0) {
    return [...models, next];
  }
  return models.map((item, itemIndex) => (itemIndex === index ? next : item));
}

export function removeKimiCatalogModels(
  models: KimiCatalogModel[],
  rowKeys: string[],
): KimiCatalogModel[] {
  const keys = new Set(rowKeys);
  return models.filter((item) => !keys.has(kimiCatalogRowKey(item)));
}

/**
 * Normalize a capability list to the vocabulary the CLI understands.
 * Returns undefined for an empty result so callers omit the field instead of
 * writing an empty array.
 */
export function normalizeKimiCapabilities(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }
  const allowed = new Set<string>(KIMI_SUPPORTED_CAPABILITIES);
  const items = value
    .map((item) => (typeof item === 'string' ? item.trim() : ''))
    .filter((item) => allowed.has(item));
  return items.length > 0 ? items : undefined;
}

/** Normalize a free-form string list (efforts); drops blanks. */
export function normalizeKimiStringArray(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }
  const items = value
    .map((item) => (typeof item === 'string' ? item.trim() : ''))
    .filter((item) => item.length > 0);
  return items.length > 0 ? items : undefined;
}
