import type { CodexCatalogModel } from '../../../../types/codex';
import type { PresetModel } from '../../../../constants/presetModels';

/** Modalities Codex understands; preset values outside this set are dropped. */
export const CODEX_SUPPORTED_MODALITIES = ['text', 'image', 'audio'] as const;

/** config.toml fallback used when the main model declares no default level. */
export const CODEX_FALLBACK_REASONING_EFFORT = 'xhigh';

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
  if (!model.modalities) {
    const input = normalizeCodexSupportedModalities(preset.modalities?.input);
    const output = normalizeCodexSupportedModalities(preset.modalities?.output);
    if (input || output) {
      model.modalities = {
        ...(input ? { input } : {}),
        ...(output ? { output } : {}),
      };
    }
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

/** Keep only modalities the Codex catalog generator can carry over. */
export function normalizeCodexSupportedModalities(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const allowed = new Set<string>(CODEX_SUPPORTED_MODALITIES);
  const items = value
    .map((item) => (typeof item === 'string' ? item.trim() : ''))
    .filter((item) => allowed.has(item));

  return items.length > 0 ? items : undefined;
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

/**
 * Row identity for the provider card's model list.
 *
 * Catalog rows are unique per (model, displayName) pair — the same backend
 * dedup key — so a row key survives reorders and stays unique when one upstream
 * model is exposed under several menu names.
 */
export function codexCatalogRowKey(item: Pick<CodexCatalogModel, 'model' | 'displayName'>): string {
  return `${item.model.trim()}\u0000${item.displayName?.trim() ?? ''}`;
}

export function findCodexCatalogRowIndex(models: CodexCatalogModel[], rowKey: string): number {
  return models.findIndex((item) => codexCatalogRowKey(item) === rowKey);
}

/** Replace the row behind `previousRowKey`, or append when it is gone. */
export function upsertCodexCatalogModel(
  models: CodexCatalogModel[],
  next: CodexCatalogModel,
  previousRowKey?: string,
): CodexCatalogModel[] {
  const index = previousRowKey ? findCodexCatalogRowIndex(models, previousRowKey) : -1;
  if (index < 0) {
    return [...models, next];
  }
  return models.map((item, itemIndex) => (itemIndex === index ? next : item));
}

export function removeCodexCatalogModels(
  models: CodexCatalogModel[],
  rowKeys: string[],
): CodexCatalogModel[] {
  const keys = new Set(rowKeys);
  return models.filter((item) => !keys.has(codexCatalogRowKey(item)));
}

export function reorderCodexCatalogModels(
  models: CodexCatalogModel[],
  orderedRowKeys: string[],
): CodexCatalogModel[] {
  const byKey = new Map(models.map((item) => [codexCatalogRowKey(item), item]));
  const ordered: CodexCatalogModel[] = [];
  for (const key of orderedRowKeys) {
    const item = byKey.get(key);
    if (item) {
      ordered.push(item);
      byKey.delete(key);
    }
  }
  // Rows missing from the incoming order keep their previous relative order.
  for (const item of models) {
    if (byKey.has(codexCatalogRowKey(item))) {
      ordered.push(item);
    }
  }
  return ordered;
}

/**
 * Model ids a Codex connectivity test should exercise: the config.toml default
 * model plus every catalog row's upstream id, de-duplicated in order.
 */
export function buildCodexConnectivityModelIds(
  defaultModelId: string | undefined,
  catalogModels: CodexCatalogModel[] | undefined,
): string[] {
  const modelIds: string[] = [];
  const seen = new Set<string>();
  const push = (value: string | undefined) => {
    const modelId = value?.trim();
    if (!modelId || seen.has(modelId)) {
      return;
    }
    seen.add(modelId);
    modelIds.push(modelId);
  };

  push(defaultModelId);
  for (const catalogModel of catalogModels ?? []) {
    push(catalogModel.model);
  }
  return modelIds;
}

/**
 * The reasoning effort config.toml should carry for the current default model.
 *
 * A main-model row without an explicit default level falls back to
 * `CODEX_FALLBACK_REASONING_EFFORT`; an unknown main model returns undefined so
 * callers leave the existing `model_reasoning_effort` value untouched.
 */
export function resolveCodexDefaultReasoningEffort(
  models: CodexCatalogModel[],
  defaultModelId: string | undefined,
): string | undefined {
  const modelId = defaultModelId?.trim();
  if (!modelId) {
    return undefined;
  }
  const row = models.find((item) => item.model.trim() === modelId);
  if (!row) {
    return undefined;
  }
  return row.defaultReasoningLevel?.trim() || CODEX_FALLBACK_REASONING_EFFORT;
}

/** Three-state image-input choice: unset (auto), explicit true, explicit false. */
export type CodexImageSupportValue = 'auto' | 'supports' | 'rejects';

export interface CodexCatalogModelFormValues {
  model: string;
  displayName?: string;
  contextWindow?: number;
  reasoningLevels?: string[];
  defaultReasoningLevel?: string;
  serviceTiers?: string[];
  inputModalities?: string[];
  outputModalities?: string[];
  imageSupport?: CodexImageSupportValue;
}

export function toCodexCatalogModelFormValues(
  item: CodexCatalogModel,
): CodexCatalogModelFormValues {
  const rawContextWindow = typeof item.contextWindow === 'number'
    ? item.contextWindow
    : Number.parseInt(String(item.contextWindow ?? '').replace(/[^\d]/g, ''), 10);
  return {
    model: item.model,
    displayName: item.displayName,
    contextWindow: Number.isFinite(rawContextWindow) && rawContextWindow > 0
      ? rawContextWindow
      : undefined,
    reasoningLevels: item.reasoningLevels,
    defaultReasoningLevel: item.defaultReasoningLevel,
    serviceTiers: item.serviceTiers,
    inputModalities: item.modalities?.input,
    outputModalities: item.modalities?.output,
    imageSupport: item.supportsImage === undefined
      ? 'auto'
      : item.supportsImage ? 'supports' : 'rejects',
  };
}

/**
 * Build the stored catalog row from the model dialog.
 *
 * `vision` / `attachment` are legacy image-capability aliases the runtime still
 * reads; they are preserved from the edited row but never edited here. An
 * explicit "Auto" image choice removes `supportsImage` so gateway detection
 * falls back to input modalities / the built-in library.
 */
export function fromCodexCatalogModelFormValues(
  values: CodexCatalogModelFormValues,
  base?: CodexCatalogModel,
): CodexCatalogModel {
  const model = values.model.trim();
  const displayName = values.displayName?.trim();
  const reasoningLevels = normalizeCodexCatalogReasoningLevels(values.reasoningLevels);
  const requestedDefaultLevel = values.defaultReasoningLevel?.trim();
  const serviceTiers = normalizeCodexCatalogServiceTiers(values.serviceTiers);
  const inputModalities = normalizeStringArray(values.inputModalities);
  const outputModalities = normalizeStringArray(values.outputModalities);
  const modalities = inputModalities || outputModalities
    ? {
        ...(inputModalities ? { input: inputModalities } : {}),
        ...(outputModalities ? { output: outputModalities } : {}),
      }
    : undefined;
  const contextWindow = typeof values.contextWindow === 'number'
    && Number.isFinite(values.contextWindow)
    && values.contextWindow > 0
    ? Math.floor(values.contextWindow)
    : undefined;

  const next: CodexCatalogModel = { model };
  if (displayName) {
    next.displayName = displayName;
  }
  if (contextWindow) {
    next.contextWindow = contextWindow;
  }
  if (typeof base?.vision === 'boolean') {
    next.vision = base.vision;
  }
  if (typeof base?.attachment === 'boolean') {
    next.attachment = base.attachment;
  }
  if (values.imageSupport === 'supports') {
    next.supportsImage = true;
  } else if (values.imageSupport === 'rejects') {
    next.supportsImage = false;
  } else if (values.imageSupport === undefined && typeof base?.supportsImage === 'boolean') {
    // The dialog no longer edits image input, so an unset choice means "keep
    // whatever the stored row declared" rather than "drop it".
    next.supportsImage = base.supportsImage;
  }
  if (modalities) {
    next.modalities = modalities;
  }
  if (reasoningLevels) {
    next.reasoningLevels = reasoningLevels;
  }
  if (requestedDefaultLevel && reasoningLevels?.includes(requestedDefaultLevel)) {
    next.defaultReasoningLevel = requestedDefaultLevel;
  }
  if (serviceTiers) {
    next.serviceTiers = serviceTiers;
  }
  return next;
}
