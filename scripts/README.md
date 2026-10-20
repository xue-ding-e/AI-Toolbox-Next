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

## verify-gateway-request-export.mjs

Tests-first coverage of the real Gateway request-detail export lifecycle:

```bash
node scripts/verify-gateway-request-export.mjs
# Diagnose one failure independently; the default still requires all five cases.
GATEWAY_REQUEST_EXPORT_CASE=picker-rejected node scripts/verify-gateway-request-export.mjs
GATEWAY_REQUEST_EXPORT_CASE=picker-pending node scripts/verify-gateway-request-export.mjs
```

- Uses the existing Chromium/Vite runner and its `--browser` / `--output` options. No dependency, production handler, or shared-runner change is needed.
- Five cases / 46 assertions: exact successful trace/path and picker options, cancel without export, rejected picker with a rendered error and restored button, single picker during pending repeated clicks, and a disabled button during pending export. Individual cases have fixed counts too; unknown or empty selections fail.
- Keeps the real `GatewayRequestsView`, `proxyGatewayApi` wrappers and `@tauri-apps/plugin-dialog` wrapper. The locked dialog version is 2.6.0; its [official save implementation](https://github.com/tauri-apps/plugins-workspace/blob/dialog-v2.6.0/plugins/dialog/guest-js/index.ts#L372-L377) invokes `plugin:dialog|save` with `{ options }`.
- Synthetic IPC allows only request list/detail, an idle rebuild-status poll, save-dialog results, and export completion. It records the path as a string and never opens a native picker or writes a file. The fixture reuses only synthetic DTO seeds from the observation tests; it does not run those tests.
- Runs fail fast. Each started case writes its synthetic state and assertion progress to `<case>.json`, including failures, alongside the shared runner's diagnostics. No rejected promise is suppressed and no browser exception is marked expected; the shared runner's global exception gate remains in force.
- This is a tests-first candidate, not a reproduction report. At baseline `36209322660b223ee6b9c9eeba67149a197cc002`, the source awaits `save()` outside its `try` and sets export busy afterward. The candidate must be executed before reporting a result or proposing a production change. A synthetic pending promise cannot establish whether the native picker permits re-entry; DOM-rendered alert checks also do not establish native-window visibility. No desktop/full-page or real export acceptance is claimed.

## verify-gateway-request-export-ownership.mjs

Additional detail-session ownership coverage, separate from the five tests-first export cases:

```bash
node scripts/verify-gateway-request-export-ownership.mjs
GATEWAY_REQUEST_EXPORT_OWNERSHIP_CASE=picker-switch node scripts/verify-gateway-request-export-ownership.mjs
GATEWAY_REQUEST_EXPORT_OWNERSHIP_CASE=export-close-reopen node scripts/verify-gateway-request-export-ownership.mjs
GATEWAY_REQUEST_EXPORT_OWNERSHIP_CASE=export-unmount node scripts/verify-gateway-request-export-ownership.mjs
```

- Three cases / 69 assertions, exactly 23 per case. Unknown or empty selections fail; each started case saves synthetic diagnostics, and all selected cases must complete. The original five-case fixture and its 46 assertions are unchanged.
- `picker-switch` keeps the first picker pending, invokes the real second-row action, then starts the replacement detail's export. The first picker later returns its own chosen path and the original export succeeds without publishing a notice or releasing the replacement export's busy state.
- `export-close-reopen` closes the real dialog and reopens the same trace while its first export is pending. A late rejection must not affect the new detail session. `export-unmount` removes and remounts the real view using host controls, then rejects the old export while the new view's export is pending.
- Every case checks exact picker filenames and export trace/path pairs, continued completion of the detached operation, no duplicate work from pending clicks, and the replacement export's eventual success and restored button. The lock is per detail session: switching, closing or unmounting detaches old UI ownership without cancelling an already authorized export.
- Uses the same dependencies and unchanged shared runner as the original export fixture. The only service alias is the real `proxyGatewayApi`; the actual dialog wrapper is retained. The strict IPC allowlist contains list/detail, idle rebuild progress, save, and export only. All paths and DTOs are fixed synthetic values, with no native picker, filesystem export, credentials, real request logs or external service.
- These neighboring cases were added with the ownership guard after the initial lifecycle RED run; they have no independent pre-fix RED evidence. Do not present them as the original tests-first reproduction or as complete race coverage. The switch uses a programmatic click on the real row, which does not prove modal hit testing or native-dialog behavior. Global browser exceptions remain failures, including rejected promises; none are consumed or marked expected.

## verify-migrated-provider-cards.mjs

Mount the real Claude Desktop, Gemini CLI, Grok and OmO Native provider mapping components:

```bash
node scripts/verify-migrated-provider-cards.mjs
```

- Uses the existing Chromium/Vite runner (`--browser`, `--output`) and locked dependencies; no dependency or production change. The 11 named scenarios and 86 assertions have per-scenario and total count gates.
- Six synthetic provider shapes × two layouts produce exactly 12 surfaces in each theme. Existing role/tag/footer/account/model smoke is retained; Grok official/custom compact cards and OmO default/ordinary list cards complete the formerly partial matrix.
- Light/dark checks target the actual `.ant-card` or `[data-provider-card-clickable]` painted surface. Each must have a box, an opaque background matching its selected/container token, appropriate light/dark channels, and a changed background across themes. Unaccented list cards use `colorBgContainer` read by `theme.useToken()` inside the actual `ConfigProvider`; compact cards use the App.css container token, and accented cards use its selected token. Do not assume the two container colors are identical. A transparent outer wrapper cannot pass as a dark card.
- OmO compact menus use real pointer hover/click to open, without fixture style overrides. Closing a menu has zero callbacks; ordinary delete dispatches its exact provider once; clicking a protected delete has zero callbacks and retains the reason. This menu dispatch does not itself open a confirmation. The separate real detail sidebar does: both cancelling and confirming its `Modal.confirm` are checked.
- Grok list and compact-detail modes each exercise ordinary refresh and Token refresh. Two synthetic account IDs detect row/action mixups; callbacks record exact provider/account IDs and drive the existing loading props. Pending repeat clicks must not duplicate the callback, and clearing the supplied prop restores both rows. The fixture adds no callback guard of its own.
- No actual account, credential, backend request or filesystem deletion is involved. Import-time platform shims remain; all IPC calls fail closed. Shared browser exceptions are never suppressed. `scenarios.json`, light/dark screenshots and the runner diagnostics are written to the printed artifact directory.
- These are strengthened component DOM acceptance checks, not evidence of a production bug or native/full-page GUI coverage. Syntax/static checks alone do not establish that the browser scenarios passed.

## verify-gemini-account-pending.mjs

Separate tests-first characterization of concurrent Gemini account actions:

```bash
node scripts/verify-gemini-account-pending.mjs
```

- One scenario / six assertions mounts the real `GeminiCliProviderCard` list layout with both existing pending props: a saved account refreshing while the virtual local account is saving.
- The real account fold is opened and both target actions must remain loading. Each receives two actual DOM clicks; neither may dispatch another callback. Callbacks only record fixed synthetic provider/account IDs and deliberately contain no pending guard.
- Before/after loading and callback evidence are collected before assertions, so a failed loading assertion still leaves the repeated-click result in `dual-pending.json`. Shared browser exceptions remain failures; all backend IPC fails closed.
- This candidate does not modify production pending semantics and has not established a reproduction until executed. It is separate from the migrated-card 11/86 suite, and does not claim Gemini page, real-account, file-save or native GUI acceptance.

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
