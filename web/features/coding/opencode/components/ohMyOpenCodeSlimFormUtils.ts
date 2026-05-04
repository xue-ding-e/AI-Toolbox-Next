import type { OhMyOpenCodeSlimAgent, OhMyOpenCodeSlimAgents } from '@/types/ohMyOpenCodeSlim';

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
    const modelValue = formValues[modelFieldName];
    const variantValue = formValues[variantFieldName];
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
      modelValue ||
      variantValue ||
      Object.keys(unmanagedFields).length > 0
    ) {
      agents[agentType] = {
        ...unmanagedFields,
        ...(modelValue ? { model: modelValue } : {}),
        ...(variantValue ? { variant: variantValue } : {}),
      };
    }
  });

  return agents;
}
