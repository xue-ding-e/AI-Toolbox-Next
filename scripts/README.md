# Scripts

## run-web-tests.mjs

Run all frontend test files under `web/test/`.

Default usage:

```bash
pnpm test
```

Notes:

- Frontend tests should live under `web/test/` and mirror the source directory structure.
- The script discovers `.test.ts` and `.spec.ts` files recursively.
- For a single file, prefer Node's built-in runner directly:

```bash
node --test web/test/path/to/file.test.ts
```

## i18n-keys.mjs

Inspect and maintain translation keys without manually reading the full locale JSON files.

Common commands:

```bash
pnpm i18n:check
pnpm i18n:report
pnpm i18n:find-text "更多选项"
pnpm i18n:find-key claudecode.settings
pnpm i18n:prune --prefix codex.settings --write
```

Notes:

- `i18n:check` fails when statically used translation keys are missing from any locale, or locale key sets drift.
- `i18n:find-text` finds keys by translated copy.
- `i18n:find-key` shows values and static usage locations for a key or prefix.
- `i18n:prune` only removes high-confidence unused keys under an explicit `--prefix`; avoid broad cleanup because dynamic keys can be resolved at runtime.

## verify-gateway-request-observations.mjs

Run the real Gateway request-list/detail component against synthetic IPC DTOs:

```bash
node scripts/verify-gateway-request-observations.mjs
```

- Uses the existing Chromium/Vite fixture runner; accepts its `--browser` and `--output` options. Requires the repository's locked frontend dependencies and an already installed Chromium browser; adds no dependency.
- Exactly 12 scenarios (6 cases × Chinese/English) and 92 assertions cover independent declared/forwarded/used flags, filtered tools, rewrite summaries and missing details, legacy records, empty arrays, and literal HTML-like summaries.
- The services alias points to the real gateway API wrappers. A strict synthetic IPC allowlist permits only list/detail reads and the view's idle rebuild-status poll; every other command fails the fixture. No credentials, real request logs, user data, or external service is needed.
- Writes `scenarios.json` and the shared runner's `results.json` into the printed artifact directory. Scenario/assertion counts and unhandled browser exceptions are failure gates.
- This is component DOM regression coverage, not desktop GUI, full-page, backend observation collection, or real JSONL acceptance. Running it is a separate step from syntax checks or TypeScript checks.

## transparent-white.js

Convert white/near-white pixels to transparent in a PNG image.

Default target (in-place):

```bash
node scripts/transparent-white.js
```

Custom input/output/threshold:

```bash
node scripts/transparent-white.js <input> [output] [threshold]
```

- `input`: source PNG path
- `output`: destination PNG path (defaults to input path)
- `threshold`: 0-255, treats any pixel with RGB >= threshold as white (default: 245)

## Regenerate Tauri icons

The main icon source is `public/icon.png`. Regenerate platform icons with:

```bash
pnpm tauri icon "public/icon.png"
```

This updates the generated icons in the Tauri icon output directories.
