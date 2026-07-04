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

export function normalizeCodexCatalogModels(models: CodexCatalogModel[]): CodexCatalogModel[] {
  const seenModels = new Set<string>();
  const normalizedModels: CodexCatalogModel[] = [];

  for (const item of models) {
    const model = item.model.trim();
    if (!model || seenModels.has(model)) {
      continue;
    }
    seenModels.add(model);

    const displayName = item.displayName?.trim();
    const rawContextWindow = String(item.contextWindow ?? '').replace(/[^\d]/g, '');
    const contextWindow = rawContextWindow ? Number.parseInt(rawContextWindow, 10) : undefined;
    const modalities = normalizeCodexCatalogModalities(item.modalities);

    normalizedModels.push({
      model,
      ...(displayName ? { displayName } : {}),
      ...(contextWindow && contextWindow > 0 ? { contextWindow } : {}),
      ...(typeof item.supportsImage === 'boolean' ? { supportsImage: item.supportsImage } : {}),
      ...(typeof item.vision === 'boolean' ? { vision: item.vision } : {}),
      ...(typeof item.attachment === 'boolean' ? { attachment: item.attachment } : {}),
      ...(modalities ? { modalities } : {}),
    });
  }

  return normalizedModels;
}
