#!/usr/bin/env bash
set -euo pipefail

# Execute only in a disposable GitHub runner checkout of this exact public base.
expected=f5b0157420a4491f6e707b49a8af8e87ea8050a1
test "${GITHUB_REPOSITORY:?}" = xue-ding-e/AI-Toolbox-Next
test "${GITHUB_REF:?}" = refs/heads/validation/magpie-portability
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
test "$(git rev-parse HEAD^{tree})" = 95e7f2606dd18e92ba1e8aae5b57e68836e7011c
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
cp "$payload/codex_portable_paginated_test.go" internal/sessions/

set +e
python3 "$runner" red 7m go test -p=2 -tags nogui ./internal/sessions -run '^TestPortCodexRolloutsDefersPaginatedHistory$' -count=1 -timeout=5m -json
red_status=$?
set -e
tail -n 40 "$RUNNER_TEMP/magpie-evidence/red.log"
test "$red_status" -ne 0
python3 - "$RUNNER_TEMP/magpie-evidence/red.log" <<'PY'
import json, sys
events = []
for line in open(sys.argv[1]):
    try:
        events.append(json.loads(line))
    except json.JSONDecodeError:
        pass
assert any(e.get("Action") == "fail" and e.get("Test") == "TestPortCodexRolloutsDefersPaginatedHistory" for e in events), "Not a behavioral RED (build/download failures do not count)"
assert any("paginated history must be deferred, not published with stale byte references" in e.get("Output", "") for e in events), "Missing expected user-visible failure"
PY

git apply --check "$payload/paginated-guard.patch"
git apply "$payload/paginated-guard.patch"
cp "$payload/toolbox.go" "$payload/toolbox_test.go" internal/codexsummary/
git diff --check
test -z "$(gofmt -l internal/sessions/codex_portable.go internal/sessions/codex_portable_paginated_test.go internal/codexsummary/toolbox.go internal/codexsummary/toolbox_test.go)"
python3 "$runner" green 5m go test -p=2 -tags nogui ./internal/sessions ./internal/codexsummary -run '^Test(PortCodexRollouts.*|PortLineLeavesOthers|Message|ToolboxSummary.*)$' -count=1 -timeout=5m -v
python3 "$runner" race 7m go test -p=2 -race -tags nogui ./internal/sessions ./internal/codexsummary -run '^Test(PortCodexRollouts.*|PortLineLeavesOthers|Message|ToolboxSummary.*)$' -count=1 -timeout=5m
git diff --exit-code -- go.mod go.sum
test "$(git hash-object go.mod)" = 750d5010dee53cdc08be244f926bd6faab2c5e7a
test "$(git hash-object go.sum)" = 6befac56cfdc580df33d2249fdc94005c36c77c5
printf '%s\n' 'Scoped synthetic Magpie portability validation complete; not a full suite or native Codex resume test.'
