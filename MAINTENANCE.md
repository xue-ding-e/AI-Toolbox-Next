# Independent maintenance

- Upstream: https://github.com/coulsontl/ai-toolbox
- Initial upstream baseline commit: `5e18c4a80130e6704fe38c9a4f134f6cc039c1c5`
- Linear baseline commit: `f5a739568cee811bed11ab745bb5a4d16917e9e2`
- `upstream-main` preserves the original upstream commit graph and authorship.
- `main` keeps a functional commit sequence without merge commits. The initial linear reconstruction retains original author/committer identities, dates and messages with an `Original-Commit` trailer. Later upstream synchronization uses cherry-picks: source authors and author dates are retained with a `(cherry picked from commit ...)` reference, while committer metadata records the new integration. Rewritten commit signatures are not represented as original signatures; inspect the upstream objects for original signatures.
- [Commit mapping](docs/upstream-history-map.tsv) records source commits, rewritten commits and integration records. Redundant merges map to the existing linear tip. Mainline integration commits retain the original upstream integration tree. Internal merge integrations apply a parent-relative difference conservatively: the unique original parent already ancestral to the most recent mainline boundary is preferred, otherwise the first parent is used. They are not claimed to be pure conflict-resolution changes.
- The linear baseline tree is identical to the recorded upstream tree. Every original first-parent boundary was checked for tree equality.
- Existing licenses, copyright notices and third-party notices remain authoritative. No upstream author is replaced or claimed as the independent maintainer.
- Historical contribution forks and experimental branches are not automatically incorporated.

## Automation and releases

Original workflow files are retained as documentation under `docs/upstream-workflows/` with `.txt` suffixes; GitHub does not load them as workflows. Repository Actions are configured for three reviewed validation workflows with read-only repository permissions. Structural history and JSON checks run on `main` changes. Full frontend and Rust validation can be dispatched manually on `main`; its initial-validation push trigger is restricted to `validation/cloud-checks`. The two heavy jobs run sequentially and do not publish or deploy.

No upstream tags, releases, secrets, signing keys, R2 storage configuration or publishing credentials are copied. This repository has not published independent binaries. Product identifiers, updater endpoints and release provenance require a separate review before building or distributing independent packages. Existing upstream development and test instructions still apply.

## Validation evidence

The initial full cloud run on `f660c307122e86c769c5d024d4c57802f3e4b2f4` passed: [validation run](https://github.com/xue-ding-e/AI-Toolbox-Next/actions/runs/38911133637). It used Node.js 22.23.3, pnpm 9.15.0 and Rust 1.99.1 on Ubuntu 22.04.

- TypeScript check passed.
- Frontend: 966 tests passed, zero failed/skipped; 307 separate browser-fixture checks and the structural layout guards passed.
- Rust: 3,118 tests passed, zero failed; 13 upstream tests remain ignored because they require real credentials, local databases/session files, installed CLIs or WSL environments. This count includes 11 passing doctests.
- These results do not verify native desktop GUI behavior, platform-specific packaging/signing, live credentials or independent update channels. See the [validation backlog](https://github.com/xue-ding-e/AI-Toolbox-Next/issues/1).

## Upstream synchronization prepared on 2026-10-18

- Latest synchronized source: `019372a8e64215fb7089a8ed28206da9f934c43c`.
- Previous mirrored source: `74d0ee923de0cc15b5be534e7f044b762de2a646`.
- Candidate starts from existing maintenance `main` at `4cf5ceda436b3c86ac506a71dd83cff307268a37`; existing history is preserved and only single-parent commits are appended.
- The 28 source commits comprise 23 separately replayed functional/documentation commits, two patch-equivalent WebSocket commits already present in this repository, and three merges. The merges are audited rather than inserted into development history. Source authors and source commit references are retained.
- PR #428's test/fix patches are already represented by `3a753736` / `153fc0e7`; stable patch IDs match official `bff3a99b` / `3efe8494` respectively.
- Merge `4adf486b` adds no independent integration change. Merge `4dd72321` combines service-tier and pricing/collector documentation; its exact resolved module-documentation blob is used during `140a6601` replay. Merge `46a3f990` reconciles heatmap/range naming, bilingual pricing text and formatter tests; its exact four resolved blobs are used during `98567a97` replay. The intervening source commits do not change those four paths.
- At the `46a3f990` integration boundary and final source tip, all upstream application and test files match the official tree exactly. Remaining differences are only the existing Next README prefix, maintenance/history records, validation workflows, and the inert copy of the original release workflow.
- [Synchronization evidence](docs/history-verification/upstream-sync-20261018.json) records every source commit and its candidate mapping. The initial baseline evidence above remains historical evidence, not a claim that this new candidate passed those old test runs.
- This record describes a local review candidate; it does not assert that remote `main` or `upstream-main` has moved. Publishing requires a fresh remote-tip check and review/validation of the final candidate. No binary release or deployment is part of this synchronization.

## DSH synchronization and targeted validation, 2026-10-18

The later synchronized source is `f3eae1e3fdbe7b2d1f53c74c8113c20df794e009`, one ordinary upstream commit after `019372a8`. It is replayed as `28e1a61777576f3d50b0261b863ff8cf97ac74ce` on maintenance base `fcbab9f1f8b270890610c8c6f080d2edb1d13a08`. All nine DSH files match the source commit exactly. The independently integrated image-history race fix, Next attribution, license/NOTICE files and release-workflow isolation are retained.

[Targeted Rust validation](https://github.com/xue-ding-e/AI-Toolbox-Next/actions/runs/39127462408) ran on exact candidate `799d39f1665253914e16f3a40be239ad95a399e0`, Ubuntu 22.04 with rustc/cargo 1.99.1. All six added session regressions were verified in the test listing before execution. Session parsing: 21 passed, 0 failed, 2 ignored; DSH configuration/catalog: 36 passed, 0 failed, 0 ignored. `real_artifact_smoke` and `real_sessions_root_smoke` remain ignored. The existing full-frontend and full-Rust jobs were deliberately skipped; this is not full-suite validation. Local focused Node checks passed 26/26 and TypeScript passed. Browser/native checks, real DSH artifacts, and a direct new input/compat form-save roundtrip remain unverified.

The dedicated CI job is manual and restricted to `maintenance/sync-dsh-021-20261018`; it shares the existing serial concurrency group, uses read-only repository permissions, and neither uploads artifacts nor publishes/deploys. The evidence record is [dsh-sync-20261018.json](docs/history-verification/dsh-sync-20261018.json).

Open boundary for follow-up: the session-list parser detects `turn/start` only within its first 40 lines. A first turn later than that could be mistaken for a blank session. This is a static boundary concern, not a reproduced regression in actual DSH 0.2.1 sessions. Verify a valid delayed-first-turn fixture and intended list semantics before a separate behavioral fix; the ignored smoke test repeats the same bounded check and cannot establish this edge case.

## OpenCode 2.0.32 synchronization and credential regression, 2026-10-19

Source `fd007e1a7fc54e8442d8ef9dd086d06c8c7aa0be` adds two ordinary commits after `f3eae1e3`: OpenCode 2.0.32 compatibility and moving the tray usage line below Open Main Window. They are replayed separately as `289e17bd` and `85bb777b`, preserving upstream authors and author dates on maintenance base `ab8d0065`. The source version fields advance to 1.2.0; this is a source synchronization, not a binary release. Existing Next attribution, AGPL/NOTICE, image/MCP/Skills corrections, and release-workflow isolation are retained.

Independent review prompted an actual synthetic SQLite/file regression: malformed legacy auth.json prevented valid V2 database credentials from being read. [RED run](https://github.com/xue-ding-e/AI-Toolbox-Next/actions/runs/39181124959/job/117833893916) on `3780bfb9` compiled successfully and passed 29 migration tests; free_models passed nine and failed the new malformed-JSON test. Later modules did not run after that failure. The independent fix, tested as `4a5c23a7` and consolidated with its same-feature regressions as `d309e32d`, uses nonempty database entries when legacy JSON cannot be read, retains the legacy error without database entries, and preserves the documented database-priority union for valid JSON. It does not infer CLI generation from a migration marker or change that priority contract. Tests use temporary synthetic files and SQLite, not user credentials.

[GREEN run](https://github.com/xue-ding-e/AI-Toolbox-Next/actions/runs/39181880993/job/117836179532) on exact `4a5c23a7904b1b0c22140e2c6dfc85d250b12323` passed 193 targeted Rust tests, zero failed/ignored: migration 29 (all names first checked with --list), free_models 12, commands 12, markdown_agents 6, tray_support 4, MCP configuration 40, runtime locations 19, backup utilities 61, tray tests 10. Ubuntu 22.04 used rustc/cargo 1.99.1. The existing full-Rust, frontend and DSH jobs were deliberately skipped. The manual job is restricted to the synchronization branch, read-only, serial, time-bounded and does not publish or upload artifacts.

Local frontend checks passed 93 focused OpenCode tests, all 1,055 Node tests, TypeScript and three structural guards on the unchanged frontend tree. These results do not verify native GUI, real credentials, live config/database migration, WSL or packaging. The default config.json rename followed by migration parse failure remains an untested boundary, not a reproduced defect. Earlier backup GUI confirmation/password/cancel checks and DSH live-session boundaries remain open. [Evidence and mappings](docs/history-verification/opencode-sync-20261019.json) distinguish exact tested commits from historical runs; integration requires a fresh remote-tip check and final independent review.
