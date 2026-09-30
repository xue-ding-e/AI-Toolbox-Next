import assert from 'node:assert/strict';
import test from 'node:test';

import {
  detectLanguageSync,
  detectSystemLanguage,
  fromStoredLanguage,
  isSupportedLanguage,
  normalizeLanguage,
  resolveLanguagePreference,
  resolveStoredLanguage,
  toStoredLanguage,
} from '../../i18n/language.ts';

/**
 * Run `body` with `navigator.language` pinned, then restore the real global.
 *
 * Pinning is what makes the locale-resolution assertions below deterministic:
 * without it they would only say what the machine running the suite happens to
 * be set to. Node's test runner gives each file its own process, so the
 * override cannot leak into another file.
 */
async function withNavigatorLanguage(
  language: string | undefined,
  body: () => Promise<void> | void,
): Promise<void> {
  const original = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
  Object.defineProperty(globalThis, 'navigator', {
    value: language === undefined ? undefined : { language },
    configurable: true,
  });

  try {
    await body();
  } finally {
    if (original) {
      Object.defineProperty(globalThis, 'navigator', original);
    } else {
      delete (globalThis as { navigator?: unknown }).navigator;
    }
  }
}

test('normalizeLanguage folds every Chinese tag to zh-CN', () => {
  for (const tag of ['zh-CN', 'zh-TW', 'zh-Hans-CN', 'zh', 'ZH-CN', 'zh_CN.UTF-8']) {
    assert.equal(normalizeLanguage(tag), 'zh-CN', `tag: ${tag}`);
  }
});

test('normalizeLanguage resolves non-Chinese and unusable tags to en-US', () => {
  for (const tag of ['en-US', 'en', 'fr-FR', 'ja-JP', 'C', 'POSIX', '']) {
    assert.equal(normalizeLanguage(tag), 'en-US', `tag: ${tag}`);
  }

  // Nothing stored and nothing reported both mean "no answer", which must not
  // turn into a language the user never asked for.
  assert.equal(normalizeLanguage(undefined), 'en-US');
  assert.equal(normalizeLanguage(null), 'en-US');
});

test('isSupportedLanguage accepts only the shipped languages', () => {
  assert.equal(isSupportedLanguage('zh-CN'), true);
  assert.equal(isSupportedLanguage('en-US'), true);

  // A near-miss such as 'zh' or 'zh-TW' would index `antdLocales` to undefined
  // and split antd (English) from i18next (Chinese), so it must be rejected.
  for (const value of ['', 'zh', 'zh-TW', 'en', 'fr-FR', undefined, null, 42, {}]) {
    assert.equal(isSupportedLanguage(value), false, `value: ${String(value)}`);
  }
});

test('detectLanguageSync follows navigator.language', async () => {
  await withNavigatorLanguage('zh-TW', () => {
    assert.equal(detectLanguageSync(), 'zh-CN');
  });

  await withNavigatorLanguage('fr-FR', () => {
    assert.equal(detectLanguageSync(), 'en-US');
  });

  // No navigator at all is the non-browser case; it must still be index-safe.
  await withNavigatorLanguage(undefined, () => {
    assert.equal(detectLanguageSync(), 'en-US');
  });
});

test('an unset stored language follows the system locale, not a fixed language', async () => {
  // A fresh install stores "" — the same sentinel the Rust `AppSettings::default()`
  // uses — and anything unrecognized is equally "never chosen".
  await withNavigatorLanguage('fr-FR', async () => {
    assert.equal(await resolveStoredLanguage(''), 'en-US');
    assert.equal(await resolveStoredLanguage(undefined), 'en-US');
    assert.equal(await resolveStoredLanguage('zh'), 'en-US');
  });

  await withNavigatorLanguage('zh-TW', async () => {
    assert.equal(await resolveStoredLanguage(''), 'zh-CN');
  });
});

test('an explicit stored language survives a disagreeing system locale', async () => {
  await withNavigatorLanguage('fr-FR', async () => {
    assert.equal(await resolveStoredLanguage('zh-CN'), 'zh-CN');
  });

  await withNavigatorLanguage('zh-CN', async () => {
    assert.equal(await resolveStoredLanguage('en-US'), 'en-US');
  });
});

test('the stored setting round-trips through the preference, with system as the empty sentinel', () => {
  // "system" persists as the empty string the backend already writes, so the
  // tray's own resolution sees exactly what it always did.
  assert.equal(toStoredLanguage('system'), '');
  assert.equal(toStoredLanguage('zh-CN'), 'zh-CN');
  assert.equal(toStoredLanguage('en-US'), 'en-US');

  for (const preference of ['system', 'zh-CN', 'en-US'] as const) {
    assert.equal(fromStoredLanguage(toStoredLanguage(preference)), preference);
  }
});

test('anything unrecognized in the setting reads back as system, not as a language', () => {
  for (const stored of ['', undefined, null, 'zh', 'zh-TW', 'en', 'fr-FR', 42, {}]) {
    assert.equal(fromStoredLanguage(stored), 'system', `stored: ${String(stored)}`);
  }

  assert.equal(fromStoredLanguage('zh-CN'), 'zh-CN');
  assert.equal(fromStoredLanguage('en-US'), 'en-US');
});

test('the OS locale query is what decides, when it answers', async () => {
  // The plugin-os path, which the fallback-only assertions above never reach:
  // whatever the query returns must win over `navigator.language` (pinned to a
  // different language here on purpose).
  await withNavigatorLanguage('fr-FR', async () => {
    assert.equal(await detectSystemLanguage(async () => 'zh-CN'), 'zh-CN');
    assert.equal(await detectSystemLanguage(async () => 'zh-TW'), 'zh-CN');
    assert.equal(await detectSystemLanguage(async () => 'en-US'), 'en-US');
    // Linux reports the raw locale name, and Windows a BCP-47 tag.
    assert.equal(await detectSystemLanguage(async () => 'zh_CN.UTF-8'), 'zh-CN');
    assert.equal(await detectSystemLanguage(async () => 'de-DE'), 'en-US');
  });

  // A null locale and a throwing IPC both mean "no answer", and both have to
  // fall back to something index-safe rather than to the raw value.
  await withNavigatorLanguage('zh-TW', async () => {
    assert.equal(await detectSystemLanguage(async () => null), 'zh-CN');
    assert.equal(
      await detectSystemLanguage(async () => {
        throw new Error('plugin:os|locale not allowed');
      }),
      'zh-CN',
    );
  });
});

test('resolving the system preference follows the locale, an explicit one does not', async () => {
  await withNavigatorLanguage('fr-FR', async () => {
    assert.equal(await resolveLanguagePreference('system'), 'en-US');
    assert.equal(await resolveLanguagePreference('zh-CN'), 'zh-CN');
  });

  await withNavigatorLanguage('zh-TW', async () => {
    assert.equal(await resolveLanguagePreference('system'), 'zh-CN');
    assert.equal(await resolveLanguagePreference('en-US'), 'en-US');
  });
});
