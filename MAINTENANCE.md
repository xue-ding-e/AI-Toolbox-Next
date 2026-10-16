# Independent maintenance

- Upstream: https://github.com/coulsontl/ai-toolbox
- Recorded upstream commit: `5e18c4a80130e6704fe38c9a4f134f6cc039c1c5`
- Linear baseline commit: `f5a739568cee811bed11ab745bb5a4d16917e9e2`
- `upstream-main` preserves the original upstream commit graph and authorship.
- `main` reconstructs the functional commit sequence without merge commits. Original author/committer identities, dates and messages are retained with an `Original-Commit` trailer. Rewritten commit signatures are not represented as original signatures; inspect `upstream-main` for the original objects.
- [Commit mapping](docs/upstream-history-map.tsv) records source commits, rewritten commits and integration records. Redundant merges map to the existing linear tip. Mainline integration commits retain the original upstream integration tree. Internal merge integrations apply a parent-relative difference conservatively: the unique original parent already ancestral to the most recent mainline boundary is preferred, otherwise the first parent is used. They are not claimed to be pure conflict-resolution changes.
- The linear baseline tree is identical to the recorded upstream tree. Every original first-parent boundary was checked for tree equality.
- Existing licenses, copyright notices and third-party notices remain authoritative. No upstream author is replaced or claimed as the independent maintainer.
- Historical contribution forks and experimental branches are not automatically incorporated.

## Automation and releases

Original workflow files are retained as documentation under `docs/upstream-workflows/` with `.txt` suffixes; GitHub does not load them as workflows. Repository Actions are enabled for two reviewed, read-only validation workflows. Structural history and JSON checks run on `main` changes. Full frontend and Rust validation can be dispatched manually on `main`; its initial-validation push trigger is restricted to `validation/cloud-checks`. The two heavy jobs run sequentially and do not publish or deploy.

No upstream tags, releases, secrets, signing keys, R2 storage configuration or publishing credentials are copied. This repository has not published independent binaries. Product identifiers, updater endpoints and release provenance require a separate review before building or distributing independent packages. Existing upstream development and test instructions still apply.

## Validation evidence

The initial full cloud run on `f660c307122e86c769c5d024d4c57802f3e4b2f4` passed: [validation run](https://github.com/xue-ding-e/AI-Toolbox-Next/actions/runs/38911133637). It used Node.js 22.23.3, pnpm 9.15.0 and Rust 1.99.1 on Ubuntu 22.04.

- TypeScript check passed.
- Frontend: 966 tests passed, zero failed/skipped; 307 separate browser-fixture checks and the structural layout guards passed.
- Rust: 3,118 tests passed, zero failed; 13 upstream tests remain ignored because they require real credentials, local databases/session files, installed CLIs or WSL environments. This count includes 11 passing doctests.
- These results do not verify native desktop GUI behavior, platform-specific packaging/signing, live credentials or independent update channels. See the [validation backlog](https://github.com/xue-ding-e/AI-Toolbox-Next/issues/1).
