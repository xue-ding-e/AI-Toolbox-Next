# Draft only: bounded capsule model interoperability validation

No external write, branch creation, dispatch, toolchain installation or Go/Rust test
execution has occurred. Local supervisor cleanup smoke was executed separately. Root admission and the global heavy-test window are
required. Proposed branch: `validation/magpie-capsule-model`, in the existing
AI-Toolbox-Next validation repository; not a Magpie initialization route.

Only the frozen new decoder, its synthetic tests, this small harness and an
independent three-field serde model belong in the validation payload. No
initialization bundle, Magpie history/tree, AGPL consumer source, user data,
or credential is included. The registered workflow would be replaced in full
on this independent validation branch, with exactly one manual guarded job.
Other existing workflows and first-push publication isolation must be rechecked
against the chosen exact branch parent before any future external write.

Inputs are frozen copies, not changed by gofmt. The job saves gofmt diff and
formatted copies for later review, but tests the original bytes and compares
them again after execution. Manifest includes every executed harness source,
the corpus and the original candidate files. The workflow separately pins that
manifest hash. There are no Magpie production dependency downloads.

Go 1.27.2 performs std-only original tests plus a harness-only corpus test,
followed by race. Rust 1.94.0 is a fixed official release, confirmed from
rust-lang/rust tag 1.94.0 src/version (blob
8db4a57b3d020eb5dec61024fd5968aadd00622f). The Rust model uses exact serde
1.0.228 and serde_json 1.0.149 with preserve_order. It is not the actual
AI-Toolbox consuming module and does not validate AES or product wiring.
The fresh small Cargo.lock is generated once under a bounded phase, exact
serde family versions are checked, then fetch/build use --locked and the build
uses --offline. Lock bytes and cargo feature tree are retained, not represented
as a pre-reviewed complete transitive dependency lock. Version validation uses
stdlib regex on Cargo-generated package records, compatible with Python 3.10.
The model features match the product manifest at AI commit 8575ade, not a
verified full-app feature-unification graph; the actual model feature tree is
retained. The i64 numeric path does not use parse_any_number, so the reviewed
arbitrary_precision/float_roundtrip options do not change these integer
classification expectations. Go tests use GOPROXY=off
but that is not an assertion that the entire runner is network-isolated.

Shared corpus: 40 purely synthetic payloads, 36 expected agreements and four
explicit conservative Go restrictions (positional array, ignored lone surrogate,
ignored invalid raw UTF-8, and total JSON depth 10001). Total depth 10000
is accepted by both. Numeric controls include -0, i64/u64 bounds,
1.0/1e0, null and missing. Whitespace is compared as original decoded bytes;
the Rust model additionally records its later trim result. A successful run
must have all original 4 Go tests, all 46 payload subtests and 8 malformed
envelope subtests, the added corpus test with all 40 subtests, no skips, and
the same result under race. Oracle results must cover exactly the same 40 IDs
and match declared classifications and summaries, not merely exit zero.

Resource ceiling: single Ubuntu job, 30 minutes total; suite work deadline 23
minutes followed by at most roughly 9 seconds of cleanup grace, with a 25-minute
step fallback. Phase budgets 3/2/3/4/4/4 minutes for Rust toolchain, resolve, fetch,
Go, race and model. Two build workers/GOMAXPROCS=2, isolated HOME/cache/targets,
16 MiB phase logs and owned-process-group cleanup. Artifact upload uses
if:always as best effort; platform timeout or upload errors can still prevent
artifact retention. Prepare and verify each have a separate 60-second limit.
Expected warm network runtime is several minutes, not a measured promise.
A single supervisor owns all phases, each with one process group; there are
no nested independent supervisors. It waits for the entire owned group rather
than only its leader, then kills/reaps with a short grace period and records
non-zombie survivors. Local synthetic smoke covered a deadline, external TERM,
and leader exit with a TERM-ignoring descendant holding stdout; all recorded
PIDs were absent afterward. This does not prove cleanup of a future CI run,
which must provide its own phase cleanup records. Only groups created by this
supervisor are targeted; this is not a general process escape sandbox.

Before admission: independent harness review; exact base/branch/complete
workflow and source hashes; verify no pending/queued/running job in the shared
concurrency group. Do not retry a denied upload or ambiguous dispatch. After
completion: read original artifact, verify manifest and command exits, compare
all expected cases, disclose compilation/dependency failures separately from
behavior failures. This is not a full history migration or native resume test.
