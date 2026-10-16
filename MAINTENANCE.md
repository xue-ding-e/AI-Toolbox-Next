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

Original workflow files are retained as documentation under `docs/upstream-workflows/` with `.txt` suffixes; GitHub does not load them as workflows. The only active workflow definition performs structural history and JSON validation on `main` changes. Repository Actions remain disabled until separately enabled after review.

No upstream tags, releases, secrets, signing keys, R2 storage configuration or publishing credentials are copied. This repository has not published independent binaries. Product identifiers, updater endpoints and release provenance require a separate review before building or distributing independent packages. Existing upstream development and test instructions still apply.
