import type { ImageProviderKind } from '../services/imageApi';

export interface ImageParameterVisibility {
  size: boolean;
  quality: boolean;
  outputFormat: boolean;
  moderation: boolean;
  outputCompression: boolean;
}

interface ImageProviderProfile {
  kind: ImageProviderKind;
  label: string;
  supportsCustomPaths: boolean;
  defaultBaseUrl?: string;
  parameterVisibility: ImageParameterVisibility;
  usesModelProfileVisibility: boolean;
}

export const DEFAULT_IMAGE_PARAMETER_VISIBILITY: ImageParameterVisibility = {
  size: true,
  quality: true,
  outputFormat: true,
  moderation: true,
  outputCompression: true,
};

export const GEMINI_NATIVE_PARAMETER_VISIBILITY: ImageParameterVisibility = {
  size: true,
  quality: false,
  outputFormat: false,
  moderation: false,
  outputCompression: false,
};

export const OPENAI_RESPONSES_PARAMETER_VISIBILITY: ImageParameterVisibility = {
  size: true,
  quality: true,
  outputFormat: true,
  moderation: false,
  outputCompression: true,
};

export const IMAGE_PROVIDER_PROFILES: Record<ImageProviderKind, ImageProviderProfile> = {
  openai_compatible: {
    kind: 'openai_compatible',
    label: 'OpenAI Compatible',
    supportsCustomPaths: true,
    parameterVisibility: DEFAULT_IMAGE_PARAMETER_VISIBILITY,
    usesModelProfileVisibility: true,
  },
  gemini: {
    kind: 'gemini',
    label: 'Gemini',
    supportsCustomPaths: false,
    defaultBaseUrl: 'https://generativelanguage.googleapis.com/v1beta',
    parameterVisibility: GEMINI_NATIVE_PARAMETER_VISIBILITY,
    usesModelProfileVisibility: false,
  },
  openai_responses: {
    kind: 'openai_responses',
    label: 'OpenAI Responses',
    supportsCustomPaths: false,
    parameterVisibility: OPENAI_RESPONSES_PARAMETER_VISIBILITY,
    usesModelProfileVisibility: false,
  },
};

export const IMAGE_PROVIDER_KIND_OPTIONS = Object.values(IMAGE_PROVIDER_PROFILES).map(
  (profile) => ({
    value: profile.kind,
    label: profile.label,
  })
);

export const getImageProviderProfile = (
  providerKind: ImageProviderKind
): ImageProviderProfile => IMAGE_PROVIDER_PROFILES[providerKind];
