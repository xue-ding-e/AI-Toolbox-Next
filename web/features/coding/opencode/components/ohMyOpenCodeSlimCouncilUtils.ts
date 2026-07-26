import type { OhMyOpenCodeSlimCouncilExecutionMode } from '@/types/ohMyOpenCodeSlim';

export interface CouncilAgentFormValue {
  model?: string;
  variant?: string;
  prompt?: string;
  modelSourceValue?: unknown;
}

type SerializedCouncilAgentConfig = Record<string, unknown> & {
  model?: unknown;
  variant?: string;
  prompt?: string;
};

interface CouncilCouncillorFormValue {
  name?: string;
  model?: string;
  variant?: string;
  prompt?: string;
}

interface CouncilPresetFormValue {
  name?: string;
  councillors?: CouncilCouncillorFormValue[];
}

const EMPTY_OBJECT: Record<string, unknown> = {};
const RESERVED_COUNCIL_OTHER_FIELD_KEYS = new Set([
  'master',
  'presets',
  'default_preset',
  'timeout',
  'master_timeout',
  'councillors_timeout',
  'master_fallback',
  'councillor_execution_mode',
  'councillor_retries',
]);
const RESERVED_PRESET_KEYS = new Set(['master', 'councillors']);

const cleanObject = (obj: Record<string, unknown>): Record<string, unknown> => {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj)) {
    if (value === null || value === undefined) continue;
    if (Array.isArray(value) && value.length === 0) continue;
    if (typeof value === 'object' && value !== null && !Array.isArray(value) && Object.keys(value).length === 0) continue;
    if (typeof value === 'string' && value.trim() === '') continue;
    result[key] = value;
  }
  return result;
};

const asObject = (value: unknown): Record<string, unknown> | undefined => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
};

const asNumber = (value: unknown): number | undefined => {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
};

const parseAgentModel = (modelValue: unknown): string | undefined => {
  if (typeof modelValue === 'string' && modelValue.trim() !== '') {
    return modelValue.trim();
  }

  if (Array.isArray(modelValue)) {
    for (const entry of modelValue) {
      if (typeof entry === 'string' && entry.trim() !== '') {
        return entry.trim();
      }
      const entryObject = asObject(entry);
      if (typeof entryObject?.id === 'string' && entryObject.id.trim() !== '') {
        return entryObject.id.trim();
      }
    }
  }

  const modelObject = asObject(modelValue);
  if (typeof modelObject?.id === 'string' && modelObject.id.trim() !== '') {
    return modelObject.id.trim();
  }

  return undefined;
};

const modelEntryId = (value: unknown): string | undefined => {
  if (typeof value === 'string') {
    return value.trim() || undefined;
  }
  const modelObject = asObject(value);
  return typeof modelObject?.id === 'string' ? modelObject.id.trim() || undefined : undefined;
};

const modelEntriesFromValue = (value: unknown): unknown[] => {
  const values = Array.isArray(value) ? value : [value];
  return values.filter((entry) => modelEntryId(entry));
};

const mergeModelFallbacks = (modelValue: unknown, fallbackValue: unknown): unknown => {
  const fallbackEntries = modelEntriesFromValue(fallbackValue);
  if (fallbackEntries.length === 0) {
    return modelValue;
  }

  const entries = modelEntriesFromValue(modelValue);
  const seenModelIds = new Set(entries.map(modelEntryId).filter((id): id is string => Boolean(id)));
  for (const fallbackEntry of fallbackEntries) {
    const fallbackId = modelEntryId(fallbackEntry);
    if (fallbackId && !seenModelIds.has(fallbackId)) {
      seenModelIds.add(fallbackId);
      entries.push(fallbackEntry);
    }
  }
  if (entries.length === 0) {
    return undefined;
  }
  return entries.length === 1 ? entries[0] : entries;
};

const replacePrimaryModel = (modelValue: unknown, nextModel: string): unknown => {
  const currentPrimaryModel = parseAgentModel(modelValue);
  if (currentPrimaryModel === nextModel) {
    return modelValue;
  }

  if (Array.isArray(modelValue)) {
    const nextEntries = [...modelValue];
    const primaryIndex = nextEntries.findIndex((entry) => modelEntryId(entry));
    if (primaryIndex < 0) {
      nextEntries.unshift(nextModel);
    } else {
      const primaryObject = asObject(nextEntries[primaryIndex]);
      nextEntries[primaryIndex] = primaryObject
        ? { ...primaryObject, id: nextModel }
        : nextModel;
    }
    return nextEntries;
  }

  const primaryObject = asObject(modelValue);
  return primaryObject ? { ...primaryObject, id: nextModel } : nextModel;
};

const serializeCouncilAgentConfig = (
  agent: CouncilAgentFormValue | undefined,
): SerializedCouncilAgentConfig | undefined => {
  if (!agent) {
    return undefined;
  }

  const primaryModel = agent.model?.trim();

  const result = cleanObject({
    model: primaryModel ? replacePrimaryModel(agent.modelSourceValue, primaryModel) : undefined,
    variant: agent.variant?.trim(),
    prompt: agent.prompt?.trim(),
  }) as SerializedCouncilAgentConfig;

  return Object.keys(result).length > 0 ? result : undefined;
};

const parseCouncilAgentFormValue = (rawAgent: unknown): CouncilAgentFormValue | undefined => {
  const agentObject = asObject(rawAgent);
  if (!agentObject) {
    return undefined;
  }

  const result = cleanObject({
    model: parseAgentModel(agentObject.model),
    variant: typeof agentObject.variant === 'string' ? agentObject.variant : undefined,
    prompt: typeof agentObject.prompt === 'string' ? agentObject.prompt : undefined,
  }) as CouncilAgentFormValue;
  if (result.model && Object.prototype.hasOwnProperty.call(agentObject, 'model')) {
    result.modelSourceValue = agentObject.model;
  }

  return Object.keys(result).length > 0 ? result : undefined;
};

const parsePresetCouncillors = (presetObject: Record<string, unknown>): CouncilCouncillorFormValue[] => {
  const nestedCouncillors = asObject(presetObject.councillors);
  if (nestedCouncillors) {
    return Object.entries(nestedCouncillors).map(([councillorName, councillorValue]) => {
      const councillorObject = asObject(councillorValue) ?? EMPTY_OBJECT;
      return {
        name: councillorName,
        model: parseAgentModel(councillorObject.model),
        variant: typeof councillorObject.variant === 'string' ? councillorObject.variant : undefined,
        prompt: typeof councillorObject.prompt === 'string' ? councillorObject.prompt : undefined,
      };
    });
  }

  return Object.entries(presetObject)
    .filter(([key]) => !RESERVED_PRESET_KEYS.has(key))
    .map(([councillorName, councillorValue]) => {
      const councillorObject = asObject(councillorValue) ?? EMPTY_OBJECT;
      return {
        name: councillorName,
        model: parseAgentModel(councillorObject.model),
        variant: typeof councillorObject.variant === 'string' ? councillorObject.variant : undefined,
        prompt: typeof councillorObject.prompt === 'string' ? councillorObject.prompt : undefined,
      };
    });
};

export interface ParseSlimCouncilFormValuesInput {
  council?: Record<string, unknown> | null;
  /**
   * Preferred source for the synthesizer model after upstream removed council.master.
   * Accepts either agents.council or a bare council agent object.
   */
  councilAgent?: Record<string, unknown> | null;
  agents?: Record<string, unknown> | null;
}

export const parseSlimCouncilFormValues = (
  rawCouncilOrInput?: Record<string, unknown> | null | ParseSlimCouncilFormValuesInput,
  maybeCouncilAgent?: Record<string, unknown> | null,
) => {
  const input: ParseSlimCouncilFormValuesInput =
    rawCouncilOrInput && typeof rawCouncilOrInput === 'object' && !Array.isArray(rawCouncilOrInput) && (
      'council' in rawCouncilOrInput ||
      'councilAgent' in rawCouncilOrInput ||
      'agents' in rawCouncilOrInput
    )
      ? (rawCouncilOrInput as ParseSlimCouncilFormValuesInput)
      : {
          council: (rawCouncilOrInput as Record<string, unknown> | null | undefined) ?? null,
          councilAgent: maybeCouncilAgent ?? null,
        };

  const council = asObject(input.council);
  const agentsObject = asObject(input.agents);
  const preferredCouncilAgentObject =
    asObject(input.councilAgent) ??
    asObject(agentsObject?.council) ??
    asObject(council?.master);
  const preferredCouncilAgent = preferredCouncilAgentObject
    ? parseCouncilAgentFormValue({
        ...preferredCouncilAgentObject,
        model: mergeModelFallbacks(preferredCouncilAgentObject.model, council?.master_fallback),
      })
    : undefined;

  if (!council) {
    return {
      councilEnabled: Boolean(preferredCouncilAgent),
      councilAgent: preferredCouncilAgent,
      councilDefaultPreset: undefined,
      councilCouncillorsTimeout: 180000,
      councilExecutionMode: 'parallel' as OhMyOpenCodeSlimCouncilExecutionMode,
      councilRetries: 3,
      councilPresets: [] as CouncilPresetFormValue[],
      councilOtherFields: undefined,
    };
  }

  const presetsObject = asObject(council.presets) ?? EMPTY_OBJECT;
  const parsedPresets: CouncilPresetFormValue[] = Object.entries(presetsObject).map(([presetName, presetValue]) => {
    const presetObject = asObject(presetValue) ?? EMPTY_OBJECT;
    return {
      name: presetName,
      councillors: parsePresetCouncillors(presetObject),
    };
  });

  const councilOtherFields = { ...council };
  delete councilOtherFields.master;
  delete councilOtherFields.presets;
  delete councilOtherFields.default_preset;
  delete councilOtherFields.timeout;
  delete councilOtherFields.master_timeout;
  delete councilOtherFields.councillors_timeout;
  delete councilOtherFields.master_fallback;
  delete councilOtherFields.councillor_execution_mode;
  delete councilOtherFields.councillor_retries;

  return {
    councilEnabled: true,
    councilAgent: preferredCouncilAgent,
    councilDefaultPreset: typeof council.default_preset === 'string' ? council.default_preset : undefined,
    councilCouncillorsTimeout: asNumber(council.timeout) ?? asNumber(council.councillors_timeout) ?? 180000,
    councilExecutionMode: council.councillor_execution_mode === 'serial' ? 'serial' as const : 'parallel' as const,
    councilRetries: asNumber(council.councillor_retries) ?? 3,
    councilPresets: parsedPresets,
    councilOtherFields: Object.keys(councilOtherFields).length > 0 ? councilOtherFields : undefined,
  };
};

export const buildSlimCouncilConfig = (
  formValues: Record<string, unknown>,
  t: (key: string, options?: Record<string, unknown>) => string,
): {
  council: Record<string, unknown> | null;
  councilAgent?: SerializedCouncilAgentConfig;
  errorMessage?: string;
} => {
  if (!formValues.councilEnabled) {
    return { council: null };
  }

  const councilAgent = serializeCouncilAgentConfig(formValues.councilAgent as CouncilAgentFormValue | undefined);
  if (!councilAgent?.model) {
    return {
      council: null,
      errorMessage: t('opencode.ohMyOpenCodeSlim.councilAgentModelRequired'),
    };
  }

  const presets = Array.isArray(formValues.councilPresets)
    ? (formValues.councilPresets as CouncilPresetFormValue[])
    : [];

  const councilOtherFields = asObject(formValues.councilOtherFields);
  if (councilOtherFields) {
    const reservedCouncilKey = Object.keys(councilOtherFields).find((key) =>
      RESERVED_COUNCIL_OTHER_FIELD_KEYS.has(key),
    );
    if (reservedCouncilKey) {
      return {
        council: null,
        errorMessage: t('opencode.ohMyOpenCodeSlim.councilOtherFieldsReservedKey', {
          key: reservedCouncilKey,
        }),
      };
    }
  }

  if (presets.length === 0) {
    return { council: null, councilAgent };
  }

  const serializedPresets: Record<string, Record<string, unknown>> = {};
  const seenPresetNames = new Set<string>();

  for (const preset of presets) {
    const presetName = preset?.name?.trim();
    if (!presetName) {
      return { council: null, errorMessage: t('opencode.ohMyOpenCodeSlim.councilPresetNameRequired') };
    }

    if (seenPresetNames.has(presetName)) {
      return {
        council: null,
        errorMessage: t('opencode.ohMyOpenCodeSlim.councilPresetNameDuplicate', { name: presetName }),
      };
    }
    seenPresetNames.add(presetName);

    const councillors = Array.isArray(preset.councillors) ? preset.councillors : [];
    if (councillors.length === 0) {
      return {
        council: null,
        errorMessage: t('opencode.ohMyOpenCodeSlim.councilPresetEmpty', { name: presetName }),
      };
    }

    const serializedPreset: Record<string, unknown> = {};
    const seenCouncillorNames = new Set<string>();
    for (const councillor of councillors) {
      const councillorName = councillor?.name?.trim();
      if (!councillorName) {
        return {
          council: null,
          errorMessage: t('opencode.ohMyOpenCodeSlim.councilCouncillorNameRequired', { preset: presetName }),
        };
      }

      if (councillorName === 'master') {
        return {
          council: null,
          errorMessage: t('opencode.ohMyOpenCodeSlim.councilCouncillorNameReserved', { preset: presetName }),
        };
      }

      if (seenCouncillorNames.has(councillorName)) {
        return {
          council: null,
          errorMessage: t('opencode.ohMyOpenCodeSlim.councilCouncillorNameDuplicate', {
            preset: presetName,
            name: councillorName,
          }),
        };
      }
      seenCouncillorNames.add(councillorName);

      const councillorModel = councillor?.model?.trim();
      if (!councillorModel) {
        return {
          council: null,
          errorMessage: t('opencode.ohMyOpenCodeSlim.councilCouncillorModelRequired', {
            preset: presetName,
            name: councillorName,
          }),
        };
      }

      serializedPreset[councillorName] = cleanObject({
        model: councillorModel,
        variant: councillor.variant?.trim(),
        prompt: councillor.prompt?.trim(),
      });
    }

    serializedPresets[presetName] = serializedPreset;
  }

  const defaultPreset = typeof formValues.councilDefaultPreset === 'string' && formValues.councilDefaultPreset.trim() !== ''
    ? formValues.councilDefaultPreset.trim()
    : Object.keys(serializedPresets)[0];

  if (!serializedPresets[defaultPreset]) {
    return {
      council: null,
      errorMessage: t('opencode.ohMyOpenCodeSlim.councilDefaultPresetMissing', { name: defaultPreset }),
    };
  }

  // Upstream no longer honors council.master / master_timeout / master_fallback.
  // Synthesizer model is returned separately as councilAgent for agents.council.
  const councilConfig = cleanObject({
    default_preset: defaultPreset,
    timeout: typeof formValues.councilCouncillorsTimeout === 'number' ? formValues.councilCouncillorsTimeout : undefined,
    councillor_execution_mode: formValues.councilExecutionMode,
    councillor_retries: typeof formValues.councilRetries === 'number' ? formValues.councilRetries : undefined,
    presets: serializedPresets,
    ...(councilOtherFields ?? {}),
  });

  return {
    council: councilConfig,
    councilAgent,
  };
};

export const mergeCouncilAgentIntoAgents = (
  agents: Record<string, unknown> | undefined,
  councilAgent: SerializedCouncilAgentConfig | undefined,
  existingCouncilAgent?: Record<string, unknown> | null,
): Record<string, unknown> | undefined => {
  const nextAgents: Record<string, unknown> = { ...(agents ?? {}) };

  if (!councilAgent) {
    return Object.keys(nextAgents).length > 0 ? nextAgents : undefined;
  }

  const existing =
    existingCouncilAgent && typeof existingCouncilAgent === 'object' && !Array.isArray(existingCouncilAgent)
      ? { ...existingCouncilAgent }
      : asObject(nextAgents.council) ?? {};

  const {
    model: _existingModel,
    variant: _existingVariant,
    prompt: _existingPrompt,
    ...unmanagedFields
  } = existing;

  const nextModel = typeof councilAgent.model === 'string'
    ? replacePrimaryModel(existing.model, councilAgent.model)
    : councilAgent.model;

  nextAgents.council = cleanObject({
    ...unmanagedFields,
    ...councilAgent,
    model: nextModel,
  });

  return nextAgents;
};
