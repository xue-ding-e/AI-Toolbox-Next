export interface BatchReplaceSourceUsage {
  usedModels: Set<string>;
  variantsByModel: Map<string, Set<string>>;
}

export interface CollectBatchReplaceSourceUsageInput {
  values: Record<string, unknown>;
  modelFieldNames: string[];
  getVariantFieldName: (modelFieldName: string) => string;
  getFallbackFieldName: (modelFieldName: string) => string;
}

export interface ApplyBatchReplaceModelInput {
  values: Record<string, unknown>;
  modelFieldNames: string[];
  fromModel: string;
  toModel: string;
  fromVariant?: string;
  toVariant?: string;
  targetVariants: string[];
  getVariantFieldName: (modelFieldName: string) => string;
  getFallbackFieldName: (modelFieldName: string) => string;
}

export interface ApplyBatchReplaceModelResult {
  updateValues: Record<string, unknown>;
  replacedCount: number;
  clearedVariantCount: number;
}

const normalizeFallbackModels = (value: unknown): string[] | undefined => {
  if (typeof value === 'string') {
    const trimmedValue = value.trim();
    return trimmedValue ? [trimmedValue] : undefined;
  }

  if (!Array.isArray(value)) {
    return undefined;
  }

  const models = value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter((item) => item !== '');

  return models.length > 0 ? models : undefined;
};

export const collectBatchReplaceSourceUsage = ({
  values,
  modelFieldNames,
  getVariantFieldName,
  getFallbackFieldName,
}: CollectBatchReplaceSourceUsageInput): BatchReplaceSourceUsage => {
  const usedModels = new Set<string>();
  const variantsByModel = new Map<string, Set<string>>();

  modelFieldNames.forEach((modelFieldName) => {
    const modelValue = values[modelFieldName];
    if (typeof modelValue === 'string' && modelValue) {
      usedModels.add(modelValue);

      const variantValue = values[getVariantFieldName(modelFieldName)];
      if (typeof variantValue === 'string' && variantValue) {
        const modelVariants = variantsByModel.get(modelValue) ?? new Set<string>();
        modelVariants.add(variantValue);
        variantsByModel.set(modelValue, modelVariants);
      }
    }

    const fallbackModels = normalizeFallbackModels(values[getFallbackFieldName(modelFieldName)]);
    fallbackModels?.forEach((fallbackModel) => {
      usedModels.add(fallbackModel);
    });
  });

  return { usedModels, variantsByModel };
};

export const applyBatchReplaceModel = ({
  values,
  modelFieldNames,
  fromModel,
  toModel,
  fromVariant,
  toVariant,
  targetVariants,
  getVariantFieldName,
  getFallbackFieldName,
}: ApplyBatchReplaceModelInput): ApplyBatchReplaceModelResult => {
  const updateValues: Record<string, unknown> = {};
  let replacedCount = 0;
  let clearedVariantCount = 0;
  const hasTargetVariants = targetVariants.length > 0;

  modelFieldNames.forEach((modelFieldName) => {
    const variantFieldName = getVariantFieldName(modelFieldName);
    const fallbackFieldName = getFallbackFieldName(modelFieldName);
    const variantValue = values[variantFieldName];

    if (values[modelFieldName] === fromModel) {
      const matchesVariant =
        !fromVariant || (typeof variantValue === 'string' && variantValue === fromVariant);

      if (matchesVariant) {
        updateValues[modelFieldName] = toModel;
        replacedCount += 1;

        if (toVariant) {
          updateValues[variantFieldName] = toVariant;
        } else if (typeof variantValue === 'string' && variantValue) {
          if (!hasTargetVariants || !targetVariants.includes(variantValue)) {
            updateValues[variantFieldName] = undefined;
            clearedVariantCount += 1;
          }
        }
      }
    }

    if (fromVariant) {
      return;
    }

    const fallbackModels = normalizeFallbackModels(values[fallbackFieldName]);
    if (!fallbackModels) {
      return;
    }

    let fallbackReplacedCount = 0;
    const nextFallbackModels = fallbackModels.map((fallbackModel) => {
      if (fallbackModel !== fromModel) {
        return fallbackModel;
      }

      fallbackReplacedCount += 1;
      return toModel;
    });

    if (fallbackReplacedCount > 0) {
      updateValues[fallbackFieldName] = nextFallbackModels;
      replacedCount += fallbackReplacedCount;
    }
  });

  return {
    updateValues,
    replacedCount,
    clearedVariantCount,
  };
};
