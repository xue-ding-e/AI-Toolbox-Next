import type { CodexCatalogModel } from '../../../../types/codex';
import type { PresetModel } from '../../../../constants/presetModels';

/** Canonical efforts understood by the Codex catalog generator. */
export const CODEX_REASONING_LEVELS = [
  'none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'ultra',
] as const;

/** Fill empty mapping fields using the same preset rules for typing and import. */
export function fillCodexCatalogModelFromPreset(
  catalogModel: CodexCatalogModel,
  preset?: PresetModel,
): CodexCatalogModel {
  if (!preset) return catalogModel;

  const model = { ...catalogModel };
  if (!model.displayName?.trim() && preset.name?.trim()) {
    model.displayName = preset.name.trim();
  }
  if (!model.contextWindow && typeof preset.contextLimit === 'number' && preset.contextLimit > 0) {
    model.contextWindow = preset.contextLimit;
  }
  if (!model.reasoningLevels?.length && preset.reasoning !== false) {
    const presetVariants = Object.entries(preset.variants ?? {});
    const declaredEfforts = new Set(
      presetVariants
        .filter(([, variant]) => !variant.disabled)
        .map(([variantName, variant]) => {
          const thinkingConfig = variant.thinkingConfig as { thinkingLevel?: unknown } | undefined;
          const effort = variant.reasoningEffort ?? variant.effort ?? thinkingConfig?.thinkingLevel ?? variantName;
          return typeof effort === 'string' ? effort.trim().toLowerCase() : '';
        }),
    );
    const presetLevels = CODEX_REASONING_LEVELS.filter((level) => declaredEfforts.has(level));
    // Older presets only declare reasoning support. Retain the manual form's
    // existing defaults for those entries; explicit variants take precedence.
    const levels = presetLevels.length > 0
      ? presetLevels
      : preset.reasoning === true && presetVariants.length === 0 ? ['low', 'high', 'max'] : [];
    if (levels.length > 0) {
      model.reasoningLevels = levels;
      if (!model.defaultReasoningLevel) {
        model.defaultReasoningLevel = levels.includes('high') ? 'high' : levels[levels.length - 1];
      }
    }
  }
  return model;
}

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

export function normalizeCodexCatalogServiceTiers(value: unknown): string[] | undefined {
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
    const serviceTiers = normalizeCodexCatalogServiceTiers(item.serviceTiers);

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
      ...(serviceTiers ? { serviceTiers } : {}),
    });
  }

  return normalizedModels;
}

/** The caller supplies exact preset lookup without coupling merges to a store. */
export type CodexCatalogPresetResolver = (modelId: string) => PresetModel | undefined;

/**
 * Merges models imported from the provider API (FetchModelsModal) into the
 * current mapping rows.
 *
 * - The final order follows `orderedModelIds` — the modal's grouped display
 *   order of the fetched list — so the mapping mirrors what the user saw,
 *   including rows that already existed. Rows whose model is unknown to this
 *   fetch (custom/pinned models) keep their previous relative order at the
 *   end.
 * - Rows whose model id is in removedModelIds are dropped. The modal only
 *   fills this list when the user explicitly opts in to removing models that
 *   no longer exist upstream, so transient upstream fluctuations never wipe
 *   mappings behind the user's back.
 * - Selected ids that already exist in the mapping are skipped, preserving
 *   user customizations (display name / context window / levels); the modal
 *   also disables checkboxes for existing ids. Rows sharing a model id but
 *   differing in displayName are kept intact.
 * - New rows use the same preset defaults as manually entered mapping rows.
 *   Existing rows retain all user customizations, including intentionally
 *   empty fields. API names are a fallback when no preset name is available.
 */
export function importModelsIntoCatalog(
  current: CodexCatalogModel[],
  selectedModels: Array<{ id?: string; name?: string }>,
  removedModelIds: string[],
  orderedModelIds: string[],
  resolvePreset: CodexCatalogPresetResolver,
): CodexCatalogModel[] {
  const removed = new Set(removedModelIds.map((modelId) => modelId.trim()).filter(Boolean));
  // Group current rows by model id so duplicate ids with different display
  // names survive the reorder as intact groups.
  const keptGroups = new Map<string, CodexCatalogModel[]>();
  for (const item of current) {
    const modelId = item.model.trim();
    if (removed.has(modelId)) {
      continue;
    }
    const group = keptGroups.get(modelId);
    if (group) {
      group.push(item);
    } else {
      keptGroups.set(modelId, [item]);
    }
  }

  const selectedById = new Map<string, { id?: string; name?: string }>();
  for (const selected of selectedModels) {
    const modelId = selected.id?.trim();
    if (modelId) selectedById.set(modelId, selected);
  }

  const rows: CodexCatalogModel[] = [];
  const placedIds = new Set<string>();
  for (const rawId of orderedModelIds) {
    const model = rawId?.trim();
    if (!model || placedIds.has(model)) {
      continue;
    }
    placedIds.add(model);
    const group = keptGroups.get(model);
    if (group) {
      rows.push(...group);
      keptGroups.delete(model);
      continue;
    }
    const selected = selectedById.get(model);
    if (selected) {
      const row = fillCodexCatalogModelFromPreset({ model }, resolvePreset(model));
      if (!row.displayName && selected.name?.trim()) {
        row.displayName = selected.name.trim();
      }
      rows.push(row);
    }
  }

  // Models unknown to this fetch (custom/pinned entries) keep their previous
  // relative order after the grouped block.
  for (const item of current) {
    if (keptGroups.has(item.model.trim())) rows.push(item);
  }

  return rows;
}
