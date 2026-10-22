#!/usr/bin/env bash
set -euo pipefail
base=$(cd "$(dirname "$0")" && pwd)
out="$RUNNER_TEMP/magpie-evidence"
work="${CAPSULE_WORK:?supervisor owns workspace}"
rustup_binary=$(command -v rustup)
export HOME="$work/home" CARGO_HOME="$work/cargo" RUSTUP_HOME="$work/rustup"
export CARGO_TARGET_DIR="$work/target" CARGO_BUILD_JOBS=2
export GOCACHE="$work/go-cache" GOPATH="$work/go-path" GOMAXPROCS=2
export GO111MODULE=off GOTOOLCHAIN=local GOPROXY=off GOSUMDB=off
export CAPSULE_CORPUS="$base/corpus.json"
export PATH="$RUSTUP_HOME/toolchains/1.94.0-x86_64-unknown-linux-gnu/bin:$PATH"
check_copies() {
    cmp "$base/payload/capsule.go" "$work/go/capsule.go"
    cmp "$base/payload/capsule_test.go" "$work/go/capsule_test.go"
    cmp "$base/corpus_test.go" "$work/go/corpus_test.go"
    cmp "$base/oracle/Cargo.toml" "$work/oracle/Cargo.toml"
    cmp "$base/oracle/src/main.rs" "$work/oracle/src/main.rs"
}
if [[ "${1:?fixed phase required}" != prepare ]]; then check_copies; fi
case "${1:?fixed phase required}" in
prepare)
    cd "$base"
    sha256sum -c payload.sha256 > "$out/input-verification.txt"
    cp payload.sha256 corpus.json "$out/"
    mkdir -p "$work/go" "$work/oracle" "$work/home" "$out/formatted"
    cp payload/capsule.go payload/capsule_test.go corpus_test.go "$work/go/"
    cp -R oracle/. "$work/oracle/"
    go version | tee "$out/go-version.txt"
    test "$(go env GOVERSION)" = go1.27.2
    command -v go > "$out/go-path.txt"
    command -v gofmt > "$out/gofmt-path.txt"
    python3 --version > "$out/python-version.txt"
    # Open outside the status-1 exception: log-open failures must remain fatal.
    exec {gofmt_diff_fd}>> "$out/gofmt.diff"
    for source in capsule.go capsule_test.go; do
        gofmt "$work/go/$source" > "$out/formatted/$source"
        # Go 1.27 reports formatting-only differences with status 1; parser or
        # other formatter errors use 2. Formatting is evidence, not a test gate.
        if gofmt -d "$work/go/$source" >&"$gofmt_diff_fd"; then
            :
        else
            format_status=$?
            test "$format_status" -eq 1
        fi
    done
    exec {gofmt_diff_fd}>&-
    # Evidence only: no gofmt -w and no replacement of frozen input bytes.
    sha256sum "$work/go/capsule.go" "$work/go/capsule_test.go" "$work/go/corpus_test.go" \
        "$work/oracle/Cargo.toml" "$work/oracle/src/main.rs" > "$out/tested-source.sha256"
    check_copies
    ;;
rust-toolchain)
    "$rustup_binary" toolchain install 1.94.0 --profile minimal --no-self-update
    rustc -Vv > "$out/rust-version.txt"
    cargo -V > "$out/cargo-version.txt"
    grep -q '^release: 1.94.0$' "$out/rust-version.txt"
    ;;
resolve)
    cargo generate-lockfile --manifest-path "$work/oracle/Cargo.toml"
    cp "$work/oracle/Cargo.lock" "$out/Cargo.lock"
    python3 - "$out/Cargo.lock" <<'PY'
# Cargo-generated lock subset validation, not a general TOML parser.
# Compatible with hosted Ubuntu 22.04's Python 3.10; no tomllib dependency.
import re, sys
text = open(sys.argv[1], encoding='utf-8').read()
pairs = []
for block in text.split('[[package]]')[1:]:
    name = re.findall(r'^name = "([^"\n]+)"$', block, re.M)
    version = re.findall(r'^version = "([^"\n]+)"$', block, re.M)
    assert len(name) == len(version) == 1
    pairs.append((name[0], version[0]))
for name, version in [('serde','1.0.228'),('serde_core','1.0.228'),('serde_derive','1.0.228'),('serde_json','1.0.149')]:
    assert [x for x in pairs if x[0] == name] == [(name, version)]
PY
    ;;
fetch)
    cargo fetch --locked --manifest-path "$work/oracle/Cargo.toml"
    cargo tree --locked --offline -e features --manifest-path "$work/oracle/Cargo.toml" > "$out/cargo-features.txt"
    ;;
green|race)
    cmp "$base/payload/capsule.go" "$work/go/capsule.go"
    cmp "$base/payload/capsule_test.go" "$work/go/capsule_test.go"
    cd "$work/go"
    export CAPSULE_RESULTS="$out/go-corpus-$1.jsonl"
    if [[ "$1" == race ]]; then
        exec go test -race -p 2 -count=1 -timeout=60s -json .
    else
        exec go test -p 2 -count=1 -timeout=60s -json .
    fi
    ;;
oracle)
    exec cargo run --locked --offline --quiet --manifest-path "$work/oracle/Cargo.toml" -- "$base/corpus.json"
    ;;
verify)
    cmp "$out/Cargo.lock" "$work/oracle/Cargo.lock"
    cmp "$base/payload/capsule.go" "$work/go/capsule.go"
    cmp "$base/payload/capsule_test.go" "$work/go/capsule_test.go"
    cd "$base"
    sha256sum -c payload.sha256 > "$out/final-input-verification.txt"
    python3 "$base/check-results.py" > "$out/summary.txt"
    ;;
*) echo 'Unknown validation phase' >&2; exit 2 ;;
esac
