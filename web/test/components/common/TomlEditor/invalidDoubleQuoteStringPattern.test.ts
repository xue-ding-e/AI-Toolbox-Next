/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { INVALID_UNCLOSED_DOUBLE_QUOTE_STRING_PATTERN } from '../../../../components/common/TomlEditor/invalidDoubleQuoteStringPattern.ts';

/** Legacy Monarch pattern that catastrophically backtracks on backslash-heavy closed strings. */
const LEGACY_CATASTROPHIC_PATTERN = /"(\\.|[^"])*$/;

/**
 * Monarch applies rules from the current cursor, so a match only counts when it
 * starts at index 0. `RegExp#test` alone is wrong here: on a closed string the
 * trailing `"` would still match the pattern from mid-string.
 */
function matchesFromStart(pattern: RegExp, input: string): boolean {
  const flags = pattern.flags.includes('g') ? pattern.flags : `${pattern.flags}g`;
  const stickyPattern = new RegExp(pattern.source, flags);
  stickyPattern.lastIndex = 0;
  const match = stickyPattern.exec(input);
  return match !== null && match.index === 0;
}

test('matches unclosed double-quoted strings from the start of the remaining input', () => {
  assert.equal(matchesFromStart(INVALID_UNCLOSED_DOUBLE_QUOTE_STRING_PATTERN, '"hello'), true);
  assert.equal(
    matchesFromStart(INVALID_UNCLOSED_DOUBLE_QUOTE_STRING_PATTERN, '"C:\\\\Users\\\\Admin'),
    true,
  );
});

test('does not match properly closed double-quoted strings from the start', () => {
  assert.equal(matchesFromStart(INVALID_UNCLOSED_DOUBLE_QUOTE_STRING_PATTERN, '"hello"'), false);
  assert.equal(
    matchesFromStart(
      INVALID_UNCLOSED_DOUBLE_QUOTE_STRING_PATTERN,
      '"C:\\\\Users\\\\Admin\\\\hook.exe"',
    ),
    false,
  );
});

test('stays linear on long closed notify-style lines with many backslashes', () => {
  // ~40 path segments of doubled backslashes + closing quote.
  // Legacy pattern takes multi-second (or freezes); fixed pattern must finish near-instantly.
  const heavyPath = Array.from({ length: 40 }, (_, index) => `dir${index}`).join('\\\\');
  const closedHeavyLine = `"C:\\\\Users\\\\Administrator\\\\AppData\\\\Local\\\\${heavyPath}\\\\nebula-hook.exe"`;

  const start = process.hrtime.bigint();
  const matched = matchesFromStart(INVALID_UNCLOSED_DOUBLE_QUOTE_STRING_PATTERN, closedHeavyLine);
  const elapsedMs = Number(process.hrtime.bigint() - start) / 1e6;

  assert.equal(matched, false);
  assert.ok(
    elapsedMs < 50,
    `fixed pattern should finish well under 50ms on backslash-heavy closed strings, took ${elapsedMs.toFixed(2)}ms`,
  );
});

test('legacy overlapping alternation is exponentially slower than the fixed pattern', () => {
  // ~18 backslash pairs is enough to show the bug without multi-second CI hangs.
  const closedBackslashHeavy = `"${'\\\\'.repeat(18)}x"`;

  const legacyStart = process.hrtime.bigint();
  matchesFromStart(LEGACY_CATASTROPHIC_PATTERN, closedBackslashHeavy);
  const legacyMs = Number(process.hrtime.bigint() - legacyStart) / 1e6;

  const fixedStart = process.hrtime.bigint();
  matchesFromStart(INVALID_UNCLOSED_DOUBLE_QUOTE_STRING_PATTERN, closedBackslashHeavy);
  const fixedMs = Number(process.hrtime.bigint() - fixedStart) / 1e6;

  assert.ok(
    fixedMs < 5,
    `fixed pattern must stay near 0ms, took ${fixedMs.toFixed(2)}ms`,
  );
  // Guard against reintroducing /"(\\.|[^"])*$/ — on this input it is already much slower.
  assert.ok(
    legacyMs > 20 && legacyMs > fixedMs * 20,
    `expected legacy pattern to be much slower than fixed (legacy=${legacyMs.toFixed(2)}ms, fixed=${fixedMs.toFixed(2)}ms)`,
  );
});
