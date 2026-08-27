import type { CodexCatalogModel } from '../../../../types/codex';

function normalizeStringArray(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const items = value
    .map((item) => (typeof item === 'string' ? item.trim() : ''))
    .filter((item) => item.length > 0);

  return items.length > 0 ? items : undefined;
}

export function normalizeCodexCatalogModalities(value: unknown): CodexCatalogModel['modalities'] | undefined {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return undefined;
  }

  const modalities = value as { input?: unknown; output?: unknown };
  const input = normalizeStringArray(modalities.input);
  const output = normalizeStringArray(modalities.output);

  if (!input && !output) {
    return undefined;
  }

  return {
    ...(input ? { input } : {}),
    ...(output ? { output } : {}),
  };
}

export function normalizeCodexCatalogReasoningLevels(value: unknown): string[] | undefined {
  return normalizeStringArray(value);
}

export function normalizeCodexCatalogModels(models: CodexCatalogModel[]): CodexCatalogModel[] {
  // Dedup by (model, displayName) so the same actual request model can appear
  // multiple times under different menu display names (e.g. mapping both
  // "luna" and "terra" menu entries to the same upstream model). Fully
  // identical rows are still collapsed.
  const seenKeys = new Set<string>();
  const normalizedModels: CodexCatalogModel[] = [];

  for (const item of models) {
    const model = item.model.trim();
    if (!model) {
      continue;
    }
    const displayName = item.displayName?.trim();
    const dedupKey = `${model}\0${displayName ?? ''}`;
    if (seenKeys.has(dedupKey)) {
      continue;
    }
    seenKeys.add(dedupKey);

    const rawContextWindow = String(item.contextWindow ?? '').replace(/[^\d]/g, '');
    const contextWindow = rawContextWindow ? Number.parseInt(rawContextWindow, 10) : undefined;
    const modalities = normalizeCodexCatalogModalities(item.modalities);
    const reasoningLevels = normalizeCodexCatalogReasoningLevels(item.reasoningLevels);
    const defaultReasoningLevel =
      typeof item.defaultReasoningLevel === 'string' && item.defaultReasoningLevel.trim()
        ? item.defaultReasoningLevel.trim()
        : undefined;

    normalizedModels.push({
      model,
      ...(displayName ? { displayName } : {}),
      ...(contextWindow && contextWindow > 0 ? { contextWindow } : {}),
      ...(typeof item.supportsImage === 'boolean' ? { supportsImage: item.supportsImage } : {}),
      ...(typeof item.vision === 'boolean' ? { vision: item.vision } : {}),
      ...(typeof item.attachment === 'boolean' ? { attachment: item.attachment } : {}),
      ...(modalities ? { modalities } : {}),
      ...(reasoningLevels ? { reasoningLevels } : {}),
      ...(defaultReasoningLevel ? { defaultReasoningLevel } : {}),
    });
  }

  return normalizedModels;
}
