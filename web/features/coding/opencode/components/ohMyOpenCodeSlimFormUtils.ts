import type { OhMyOpenCodeSlimAgent, OhMyOpenCodeSlimAgents } from '@/types/ohMyOpenCodeSlim';

export interface BuildSlimAgentsInput {
  builtInAgentKeys: string[];
  customAgents: string[];
  formValues: Record<string, unknown>;
  initialAgents?: OhMyOpenCodeSlimAgents;
}

export function buildSlimAgentsFromFormValues({
  builtInAgentKeys,
  customAgents,
  formValues,
  initialAgents,
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

    const { model: _existingModel, variant: _existingVariant, ...existingUnmanagedFields } =
      existingAgent || {};

    if (
      modelValue ||
      variantValue ||
      Object.keys(existingUnmanagedFields).length > 0
    ) {
      agents[agentType] = {
        ...existingUnmanagedFields,
        ...(modelValue ? { model: modelValue } : {}),
        ...(variantValue ? { variant: variantValue } : {}),
      };
    }
  });

  return agents;
}
