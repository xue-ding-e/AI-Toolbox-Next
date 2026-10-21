#!/usr/bin/env bash
set -euo pipefail

# Execute only in a disposable GitHub runner checkout of this exact public base.
expected=b1a2eec2c143021494b2274f2311da9bccab5740
test "${GITHUB_REPOSITORY:?}" = xue-ding-e/AI-Toolbox-Next
test "${GITHUB_REF:?}" = refs/heads/validation/magpie-combined-portability
test "${GITHUB_EVENT_NAME:?}" = workflow_dispatch
test "$(git -C "${GITHUB_WORKSPACE:?}/validation" rev-parse HEAD)" = "${GITHUB_SHA:?}"
source_dir="${GITHUB_WORKSPACE:?}/magpie"
payload="${GITHUB_WORKSPACE}/validation/payload"
cd "$source_dir"
evidence="${RUNNER_TEMP:?}/magpie-evidence"
mkdir -p "$evidence"
finish() {
  result=$?
  git rev-parse HEAD HEAD^{tree} > "$evidence/source-after.sha"
  git hash-object go.mod go.sum > "$evidence/locks-after.sha"
  printf '%s\n' "$result" > "$evidence/harness-exit.code"
  exit "$result"
}
trap finish EXIT
git rev-parse HEAD HEAD^{tree} > "$evidence/source-before.sha"
git hash-object go.mod go.sum > "$evidence/locks-before.sha"
test "$(git rev-parse HEAD)" = "$expected"
test "$(git rev-parse HEAD^{tree})" = bb0cab39c16371ade106c3c5bf5609a14bf4149c
test "$(git hash-object go.mod)" = 750d5010dee53cdc08be244f926bd6faab2c5e7a
test "$(git hash-object go.sum)" = 6befac56cfdc580df33d2249fdc94005c36c77c5
test -z "$(git status --porcelain)"
go version
test "$(go env GOVERSION)" = go1.27.2
export GOPATH="$(go env GOPATH)" GOMODCACHE="$(go env GOMODCACHE)" GOCACHE="$(go env GOCACHE)"
export HOME="$(mktemp -d "${RUNNER_TEMP:?}/magpie-home.XXXXXX")"
export XDG_CONFIG_HOME="$HOME/.config" GOTOOLCHAIN=local
export GOPROXY=https://proxy.golang.org GOSUMDB=sum.golang.org GOFLAGS=-mod=readonly
export GOMAXPROCS=2
runner="${GITHUB_WORKSPACE}/validation/run-bounded.py"
python3 "$runner" download 5m go mod download
git diff --exit-code -- go.mod go.sum
# Runtime tests must not fetch packages or reach a real provider.
export GOPROXY=off GOSUMDB=off
# Apply the independently reviewed features in order. No decoder is included.
for patch in paginated-guard failed-retry seam scan-complete; do
  git apply --check "$payload/$patch.patch"
  git apply "$payload/$patch.patch"
done
cp "$payload/"*_test.go internal/sessions/
git diff --check
test -z "$(gofmt -l internal/sessions/codex_portable.go internal/sessions/codex_portable_{paginated,retry,scan}_test.go)"
echo 'de84fdcd9253b1911cf2141821e266c2fb0eb949b27cf0974b465202f9ea16ac  internal/sessions/codex_portable.go' | sha256sum -c -
sha256sum internal/sessions/codex_portable.go internal/sessions/codex_portable_{paginated,retry,scan}_test.go > "$evidence/combined-files.sha256"
git add -- internal/sessions/codex_portable.go internal/sessions/codex_portable_{paginated,retry,scan}_test.go
git write-tree > "$evidence/combined-tree.sha"
git diff --cached --binary > "$evidence/combined.patch"
python3 "$runner" green 5m go test -p=2 -tags nogui ./internal/sessions -run '^Test(PortCodexRollouts.*|PortLineLeavesOthers)$' -count=1 -timeout=4m -json
python3 "$runner" race 5m go test -p=2 -race -tags nogui ./internal/sessions -run '^Test(PortCodexRollouts.*|PortLineLeavesOthers)$' -count=1 -timeout=4m -json
python3 - "$evidence" <<'CHECK'
import json, pathlib, sys
expected = {
 'TestPortCodexRollouts', 'TestPortCodexRolloutsLeavesAnOpenThread',
 'TestPortLineLeavesOthers', 'TestPortCodexRolloutsDefersPaginatedHistory',
 'TestPortCodexRolloutsLeavesUnchangedPaginatedHistory',
 'TestPortCodexRolloutsRetriesFailedRead', 'TestPortCodexRolloutsIncompleteScan',
 'TestPortCodexRolloutsMissingArchiveCompletes',
}
for phase in ('green', 'race'):
    events = []
    for line in (pathlib.Path(sys.argv[1]) / (phase + '.log')).read_text().splitlines():
        try: events.append(json.loads(line))
        except json.JSONDecodeError: pass
    passed = {e.get('Test') for e in events if e.get('Action') == 'pass'}
    assert expected <= passed, (phase, 'missing executed tests', expected - passed)
    assert not any(e.get('Action') in ('fail', 'skip') for e in events), phase
CHECK
git diff --exit-code -- go.mod go.sum
test "$(git hash-object go.mod)" = 750d5010dee53cdc08be244f926bd6faab2c5e7a
test "$(git hash-object go.sum)" = 6befac56cfdc580df33d2249fdc94005c36c77c5
printf '%s\n' 'Scoped synthetic Magpie portability validation complete; not a full suite or native Codex resume test.'
