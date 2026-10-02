/**
 * Declared-vs-tested vision capability comparison for connectivity tests.
 *
 * A provider/model may *declare* image support (`attachment`, `vision`, or an
 * `image` input modality) while the deployment behind it does not actually
 * load a vision model. The connectivity probe sends a tiny test image and this
 * helper reports whether the declared capability matches what the probe saw,
 * so the UI can flag the mismatch instead of trusting the declaration.
 */

export type VisionProbeStatus = 'passed' | 'failed' | 'unavailable';

/** The subset of a model record that can declare image input capability. */
export interface DeclaredVisionModel {
  attachment?: boolean;
  vision?: boolean;
  supportsImage?: boolean;
  modalities?: { input?: string[] };
}

/**
 * Read the declared image capability from a model record. Returns `undefined`
 * when the model says nothing about images (unknown), matching the backend
 * gateway resolver which only classifies on an explicit value.
 */
export function declaredVisionCapability(model: DeclaredVisionModel | undefined): boolean | undefined {
  if (!model) {
    return undefined;
  }
  for (const value of [model.supportsImage, model.vision, model.attachment]) {
    if (typeof value === 'boolean') {
      return value;
    }
  }
  const input = model.modalities?.input;
  if (Array.isArray(input)) {
    return input.some((entry) => entry.toLowerCase() === 'image');
  }
  return undefined;
}

/**
 * Whether the probe contradicts the declaration. Only a real pass/fail counts;
 * `unavailable` (no probe ran) and unknown declarations never report a
 * mismatch, because absence of evidence is not evidence.
 */
export function visionProbeMismatchesDeclaration(
  declared: boolean | undefined,
  tested: VisionProbeStatus | undefined,
): boolean {
  if (declared === undefined || tested === undefined) {
    return false;
  }
  if (tested === 'unavailable') {
    return false;
  }
  return (tested === 'passed') !== declared;
}
