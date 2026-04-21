import JSON5 from 'json5';

export interface ImportedConfigData {
  agents?: Record<string, Record<string, unknown>>;
  categories?: Record<string, Record<string, unknown>>;
  otherFields?: Record<string, unknown>;
}

export type ImportedConfigVariant = 'omo' | 'omos';

export const isPlainObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

export const deepMergeObjects = (
  base?: Record<string, unknown>,
  override?: Record<string, unknown>,
): Record<string, unknown> | undefined => {
  if (!base) return override;
  if (!override) return base;

  const result: Record<string, unknown> = { ...base };

  Object.entries(override).forEach(([key, overrideValue]) => {
    const baseValue = result[key];
    if (isPlainObject(baseValue) && isPlainObject(overrideValue)) {
      result[key] = deepMergeObjects(baseValue, overrideValue);
      return;
    }
    result[key] = overrideValue;
  });

  return result;
};

export const resolveSlimImportedAgents = (
  config: Record<string, unknown>,
): Record<string, Record<string, unknown>> | undefined => {
  const rootAgents = isPlainObject(config.agents)
    ? config.agents as Record<string, Record<string, unknown>>
    : undefined;

  const activePresetName = typeof config.preset === 'string' ? config.preset.trim() : '';
  const presets = isPlainObject(config.presets) ? config.presets : undefined;
  let presetAgents = activePresetName && presets && isPlainObject(presets[activePresetName])
    ? presets[activePresetName] as Record<string, Record<string, unknown>>
    : undefined;

  if (!presetAgents && presets) {
    const presetEntries = Object.entries(presets).filter(([, presetValue]) => isPlainObject(presetValue));
    if (presetEntries.length === 1) {
      presetAgents = presetEntries[0][1] as Record<string, Record<string, unknown>>;
    }
  }

  if (!presetAgents) {
    return rootAgents;
  }

  return deepMergeObjects(presetAgents, rootAgents) as Record<string, Record<string, unknown>>;
};

export const extractImportedConfigData = (
  config: Record<string, unknown>,
  variant: ImportedConfigVariant,
): ImportedConfigData | undefined => {
  const agents = variant === 'omos'
    ? resolveSlimImportedAgents(config)
    : (isPlainObject(config.agents)
      ? config.agents as Record<string, Record<string, unknown>>
      : undefined);

  const categories = isPlainObject(config.categories)
    ? config.categories as Record<string, Record<string, unknown>>
    : undefined;

  const otherFields: Record<string, unknown> = {};
  Object.entries(config).forEach(([key, value]) => {
    if (
      key !== 'agents' &&
      key !== 'categories' &&
      key !== '$schema' &&
      !(variant === 'omos' && (key === 'preset' || key === 'presets'))
    ) {
      otherFields[key] = value;
    }
  });

  if (!agents && !categories) {
    return undefined;
  }

  return {
    agents,
    categories,
    otherFields: Object.keys(otherFields).length > 0 ? otherFields : undefined,
  };
};

export const parseImportedConfigText = (
  raw: string,
  variant: ImportedConfigVariant,
): ImportedConfigData | undefined => {
  const parsedValue = JSON5.parse(raw);
  if (!isPlainObject(parsedValue)) {
    throw new Error('invalid-config-object');
  }

  return extractImportedConfigData(parsedValue, variant);
};
