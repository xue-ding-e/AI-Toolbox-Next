import type { ZcodeModelRow, ZcodeSettingsConfig } from '@/types/zcode';

/**
 * Parses a provider's stored `settings_config`.
 *
 * Returns `null` rather than throwing so a malformed row degrades to an empty
 * card instead of breaking the whole page.
 */
export const parseZcodeProviderSettings = (
  settingsConfig: string,
): ZcodeSettingsConfig | null => {
  try {
    const parsed = JSON.parse(settingsConfig) as ZcodeSettingsConfig;
    if (!parsed || typeof parsed !== 'object' || !parsed.providerId) {
      return null;
    }
    return { ...parsed, models: parsed.models ?? [] };
  } catch {
    return null;
  }
};

/**
 * Picks the model id ZCode should select by default for this provider.
 *
 * Prefers the explicitly marked default, then the stored fallback, then the
 * first row — the same order the backend uses when projecting.
 */
export const resolveZcodeDefaultModelId = (settingsConfig: string): string | null => {
  const settings = parseZcodeProviderSettings(settingsConfig);
  if (!settings) {
    return null;
  }
  const marked = settings.models.find((model) => model.isDefault);
  return marked?.modelId ?? settings.defaultModelId ?? settings.models[0]?.modelId ?? null;
};

/** Human-readable summary of a model row for the provider card list. */
export const describeZcodeModel = (model: ZcodeModelRow): string => {
  const parts: string[] = [];
  if (model.properties?.contextWindow) {
    parts.push(`${model.properties.contextWindow.toLocaleString()} ctx`);
  }
  const maxOutputTokens = model.optionSpecs?.maxOutputTokens?.max;
  if (maxOutputTokens) {
    parts.push(`max ${maxOutputTokens.toLocaleString()}`);
  }
  const levels = model.optionSpecs?.reasoningLevel?.values;
  if (levels && levels.length > 0) {
    parts.push(levels.join('/'));
  }
  return parts.join(' · ');
};

/**
 * Builds a `custom:` provider id from a display name.
 *
 * ZCode treats `builtin:` and `account:` as reserved namespaces, so managed
 * providers are namespaced to avoid colliding with either.
 */
export const buildZcodeProviderId = (name: string): string => {
  const slug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return `custom:${slug || 'provider'}`;
};
