/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  declaredVisionCapability,
  visionProbeMismatchesDeclaration,
} from '../../../../../features/coding/shared/providerConnectivity/visionProbe.ts';

test('declaredVisionCapability reads explicit booleans and modalities', () => {
  assert.equal(declaredVisionCapability({ attachment: true }), true);
  assert.equal(declaredVisionCapability({ vision: false }), false);
  assert.equal(declaredVisionCapability({ supportsImage: true }), true);
  assert.equal(declaredVisionCapability({ modalities: { input: ['text', 'image'] } }), true);
  assert.equal(declaredVisionCapability({ modalities: { input: ['text'] } }), false);
  // Explicit declaration wins over modalities.
  assert.equal(
    declaredVisionCapability({ supportsImage: false, modalities: { input: ['image'] } }),
    false,
  );
});

test('declaredVisionCapability returns undefined when the model says nothing', () => {
  assert.equal(declaredVisionCapability(undefined), undefined);
  assert.equal(declaredVisionCapability({}), undefined);
});

test('visionProbeMismatchesDeclaration flags declared-vs-tested conflicts only', () => {
  // Declared vision but the probe could not read the image: mismatch.
  assert.equal(visionProbeMismatchesDeclaration(true, 'failed'), true);
  // Declared text-only but the probe succeeded: mismatch.
  assert.equal(visionProbeMismatchesDeclaration(false, 'passed'), true);
  // Agreement either way: no mismatch.
  assert.equal(visionProbeMismatchesDeclaration(true, 'passed'), false);
  assert.equal(visionProbeMismatchesDeclaration(false, 'failed'), false);
  // No probe ran or nothing declared: never a mismatch.
  assert.equal(visionProbeMismatchesDeclaration(true, 'unavailable'), false);
  assert.equal(visionProbeMismatchesDeclaration(undefined, 'failed'), false);
  assert.equal(visionProbeMismatchesDeclaration(true, undefined), false);
});
