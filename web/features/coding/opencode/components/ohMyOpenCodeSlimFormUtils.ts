import type { OhMyOpenCodeSlimAgent, OhMyOpenCodeSlimAgents } from '@/types/ohMyOpenCodeSlim';

export interface SlimModelState {
  primaryModel?: string;
  primaryVariant?: string;
  fallbackModels?: string[];
}

export const getSlimModelEntryId = (value: unknown): string | undefined => {
  if (typeof value === 'string') {
    const trimmedValue = value.trim();
    return trimmedValue ? trimmedValue : undefined;
  }

  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return undefined;
  }

  const id = (value as Record<string, unknown>).id;
  if (typeof id !== 'string') {
    return undefined;
  }

  const trimmedId = id.trim();
  return trimmedId ? trimmedId : undefined;
};

export const splitSlimModelValue = (modelValue: unknown): SlimModelState => {
  if (!Array.isArray(modelValue)) {
    return {
      primaryModel: getSlimModelEntryId(modelValue),
    };
  }

  const modelEntries = modelValue
    .map((entry) => ({
      id: getSlimModelEntryId(entry),
      variant: entry && typeof entry === 'object' && !Array.isArray(entry)
        ? (entry as Record<string, unknown>).variant
        : undefined,
    }))
    .filter((entry): entry is { id: string; variant: unknown } => Boolean(entry.id));

  const [primaryEntry, ...fallbackEntries] = modelEntries;
  return {
    primaryModel: primaryEntry?.id,
    primaryVariant: typeof primaryEntry?.variant === 'string' ? primaryEntry.variant : undefined,
    fallbackModels: fallbackEntries.length > 0 ? fallbackEntries.map((entry) => entry.id) : undefined,
  };
};

const asNonEmptyString = (value: unknown): string | undefined => {
  if (typeof value !== 'string') {
    return undefined;
  }

  const trimmedValue = value.trim();
  return trimmedValue ? trimmedValue : undefined;
};

const asStringArray = (value: unknown): string[] => {
  if (typeof value === 'string') {
    const trimmedValue = value.trim();
    return trimmedValue ? [trimmedValue] : [];
  }

  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter((item) => item !== '');
};

const buildModelValue = (
  primaryModel: string | undefined,
  fallbackModels: string[],
): string | string[] | undefined => {
  const modelChain: string[] = [];
  if (primaryModel) {
    modelChain.push(primaryModel);
  }

  fallbackModels.forEach((fallbackModel) => {
    if (!modelChain.includes(fallbackModel)) {
      modelChain.push(fallbackModel);
    }
  });

  if (modelChain.length === 0) {
    return undefined;
  }

  return modelChain.length === 1 ? modelChain[0] : modelChain;
};

export interface BuildSlimAgentsInput {
  builtInAgentKeys: string[];
  customAgents: string[];
  formValues: Record<string, unknown>;
  initialAgents?: OhMyOpenCodeSlimAgents;
  advancedSettings?: Record<string, Record<string, unknown>>;
}

export function buildSlimAgentsFromFormValues({
  builtInAgentKeys,
  customAgents,
  formValues,
  initialAgents,
  advancedSettings,
}: BuildSlimAgentsInput): OhMyOpenCodeSlimAgents {
  const allAgentKeys = [...builtInAgentKeys, ...customAgents];
  const agents: OhMyOpenCodeSlimAgents = {};

  allAgentKeys.forEach((agentType) => {
    const modelFieldName = `agent_${agentType}_model`;
    const variantFieldName = `agent_${agentType}_variant`;
    const fallbackModelsFieldName = `agent_${agentType}_fallback_models`;
    const modelValue = formValues[modelFieldName];
    const variantValue = formValues[variantFieldName];
    const primaryModel = asNonEmptyString(modelValue);
    const fallbackModels = asStringArray(formValues[fallbackModelsFieldName]);
    const nextModelValue = buildModelValue(primaryModel, fallbackModels);
    const existingAgent =
      initialAgents?.[agentType] && typeof initialAgents[agentType] === 'object'
        ? (initialAgents[agentType] as OhMyOpenCodeSlimAgent)
        : undefined;

    const {
      model: _existingModel,
      variant: _existingVariant,
      fallback_models: _existingFallbackModels,
      ...existingUnmanagedFields
    } =
      existingAgent || {};
    const hasAdvancedSettings = Object.prototype.hasOwnProperty.call(advancedSettings ?? {}, agentType);
    const {
      model: _advancedModel,
      variant: _advancedVariant,
      fallback_models: _advancedFallbackModels,
      ...advancedUnmanagedFields
    } = (hasAdvancedSettings ? advancedSettings?.[agentType] : existingUnmanagedFields) || {};
    const unmanagedFields = advancedUnmanagedFields as Record<string, unknown>;

    if (
      nextModelValue ||
      variantValue ||
      Object.keys(unmanagedFields).length > 0
    ) {
      agents[agentType] = {
        ...unmanagedFields,
        ...(nextModelValue ? { model: nextModelValue } : {}),
        ...(variantValue ? { variant: variantValue } : {}),
      };
    }
  });

  return agents;
}
