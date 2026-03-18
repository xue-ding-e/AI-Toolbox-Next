import type { OpenCodeModelVariant } from '@/types/opencode';

/**
 * Preset models configuration for different AI SDK types.
 *
 * The canonical data lives in tauri/resources/preset_models.json.
 * On app startup the Rust backend loads the bundled defaults (or local
 * cache) and populates PRESET_MODELS, then the frontend background-
 * fetches the latest version from the remote repository.
 */

export interface PresetModel {
  id: string;
  name: string;
  contextLimit?: number;
  outputLimit?: number;
  modalities?: { input: string[]; output: string[] };
  attachment?: boolean;
  reasoning?: boolean;
  tool_call?: boolean;
  temperature?: boolean;
  variants?: Record<string, OpenCodeModelVariant>;
  options?: Record<string, unknown>;
}

/**
 * Remote URL for fetching the latest preset models JSON.
 * Points to the raw file in the main branch of the repository.
 */
export const PRESET_MODELS_REMOTE_URL =
  'https://raw.githubusercontent.com/coulsontl/ai-toolbox/main/tauri/resources/preset_models.json';

type PresetModelsListener = () => void;

/**
 * Preset models grouped by npm SDK type.
 *
 * Starts empty and is populated at startup from the Rust backend
 * (bundled defaults or local cache), then updated from remote.
 * Components that need reactive updates should subscribe to the
 * version change exposed below.
 */
export const PRESET_MODELS: Record<string, PresetModel[]> = {};

let presetModelsVersion = 0;
const presetModelsListeners = new Set<PresetModelsListener>();

export const getPresetModelsVersion = (): number => presetModelsVersion;

export const subscribePresetModels = (listener: PresetModelsListener): (() => void) => {
  presetModelsListeners.add(listener);
  return () => {
    presetModelsListeners.delete(listener);
  };
};

const notifyPresetModelsUpdated = () => {
  presetModelsVersion += 1;
  presetModelsListeners.forEach((listener) => listener());
};

/**
 * Replace the contents of PRESET_MODELS with `models`.
 * The object reference stays the same so existing imports remain valid.
 *
 * If `models` is empty or invalid the call is a no-op so that
 * existing data is never accidentally wiped out.
 */
export const updatePresetModels = (models: Record<string, PresetModel[]>) => {
  // Guard: never replace with empty / invalid data
  if (!models || typeof models !== 'object' || Object.keys(models).length === 0) {
    return;
  }
  // Remove old keys
  for (const key of Object.keys(PRESET_MODELS)) {
    delete PRESET_MODELS[key];
  }
  // Copy new keys
  Object.assign(PRESET_MODELS, models);
  notifyPresetModelsUpdated();
};
