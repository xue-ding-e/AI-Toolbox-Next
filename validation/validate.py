import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time


BASE = "861aa4f05c83d0927128292276bd59bfeb54a2da"
REPOSITORY = "xue-ding-e/AI-Toolbox-Next"
REF = "refs/heads/validation/reasoning-item-ids"
PREFIX = "tauri/src/coding/proxy_gateway/transformer/"
PRODUCT_FILES = {PREFIX + name for name in (
    "kernel_tests.rs", "reasoning_item_id_tests.rs", "stream.rs"
)}
STREAM_FILE = PREFIX + "stream.rs"
TEST_FILE = PREFIX + "reasoning_item_id_tests.rs"
GENERATED_DIR = "tauri/gen/schemas"
HARNESS_FILES = {
    ".github/workflows/maintenance-validation.yml",
    "validation/candidate.diff",
    "validation/manifest.json",
    "validation/validate.py",
}
MODULE = "coding::proxy_gateway::transformer::"
TEST_MODULE = MODULE + "kernel::tests::reasoning_item_id_tests::"
RUST_VERSION = "1.99.1"
LOG_LIMIT = 32 * 1024 * 1024
ROOT = Path(os.environ["GITHUB_WORKSPACE"])
HARNESS = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "source"
TARGET = SOURCE / "tauri/target"
EVIDENCE = Path(os.environ["RUNNER_TEMP"]) / "reasoning-item-ids-evidence"
EVIDENCE.mkdir(exist_ok=False)
(EVIDENCE / "result.json").write_text(json.dumps({
    "status": "incomplete", "stage": "preflight", "base": BASE,
}) + "\n")
started = float(os.environ["VALIDATION_STARTED_EPOCH"])
elapsed = time.time() - started
assert 0 <= elapsed < 31 * 60, "Invalid or expired workflow clock"
OVERALL_DEADLINE = time.monotonic() + (31 * 60 - elapsed)
FINAL_DEADLINE = OVERALL_DEADLINE + 120
in_cleanup = False
command_sequence = 0
process_records = []


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(name, value):
    (EVIDENCE / name).write_text(json.dumps(value, indent=2) + "\n")


def members(group):
    # Linux /proc stat: state, ppid, pgrp after the closing command parenthesis.
    # Zombies cannot execute and are waiting for their parent/reaper.
    found = []
    for item in Path("/proc").iterdir():
        if not item.name.isdigit():
            continue
        try:
            fields = (item / "stat").read_text().rsplit(")", 1)[1].split()
            if int(fields[2]) == group and fields[0] != "Z":
                found.append(int(item.name))
        except (FileNotFoundError, ProcessLookupError):
            pass
    return sorted(found)


def interrupted(signum, _frame):
    raise RuntimeError(f"Validator interrupted by signal {signum}")


signal.signal(signal.SIGTERM, interrupted)
signal.signal(signal.SIGINT, interrupted)


def run(name, args, seconds, cwd=SOURCE, extra_env=None):
    global command_sequence
    command_sequence += 1
    stem = f"{command_sequence:04d}-{name}"
    log_path = EVIDENCE / (stem + ".log")
    budget_deadline = FINAL_DEADLINE if in_cleanup else OVERALL_DEADLINE
    assert time.monotonic() < budget_deadline, "Overall deadline exhausted"
    local_deadline = time.monotonic() + seconds
    deadline = min(local_deadline, budget_deadline)
    process = None
    reason = None
    remaining = None
    cleanup_error = None
    spawn_error = None
    start = time.monotonic()
    with log_path.open("wb") as log:
        try:
            process = subprocess.Popen(
                args, cwd=cwd, stdout=log, stderr=subprocess.STDOUT,
                start_new_session=True,
                env={**os.environ, **(extra_env or {})},
            )
            while process.poll() is None:
                if time.monotonic() >= deadline:
                    reason = "overall-deadline" if deadline < local_deadline else "timeout"
                    break
                if log_path.stat().st_size > LOG_LIMIT:
                    reason = "log-limit"
                    break
                time.sleep(0.1)
            if log_path.stat().st_size > LOG_LIMIT:
                reason = reason or "log-limit"
            remaining = members(process.pid)
            if remaining:
                reason = reason or "surviving-descendants"
        except BaseException as error:
            spawn_error = type(error).__name__ + ": " + str(error)
            raise
        finally:
            try:
                if process is not None:
                    for sig in (signal.SIGTERM, signal.SIGKILL):
                        if process.poll() is not None and not members(process.pid):
                            break
                        try:
                            os.killpg(process.pid, sig)
                        except ProcessLookupError:
                            pass
                        until = time.monotonic() + 5
                        while (process.poll() is None or members(process.pid)) and time.monotonic() < until:
                            time.sleep(0.1)
                    remaining = members(process.pid)
            except BaseException as error:
                cleanup_error = type(error).__name__ + ": " + str(error)
            finally:
                code = process.poll() if process is not None else None
                record = {
                    "name": name, "command": args, "cwd": str(cwd),
                    "pid": process.pid if process is not None else None,
                    "exitCode": code, "reason": reason,
                    "survivors": remaining, "cleanupError": cleanup_error,
                    "spawnOrInterruptError": spawn_error,
                    "seconds": round(time.monotonic() - start, 3),
                    "timeoutSeconds": seconds,
                    "logBytes": log_path.stat().st_size, "logSoftLimit": LOG_LIMIT,
                }
                save(stem + "-process.json", record)
                process_records.append(record)
                save("processes.json", process_records)
    assert cleanup_error is None and code is not None and remaining == [], (
        "Owned process exit unconfirmed; outer job timeout is the final safeguard", record
    )
    assert reason is None and spawn_error is None, record
    return code, log_path


def successful(name, args, seconds, **kwargs):
    code, log = run(name, args, seconds, **kwargs)
    assert code == 0, (name, code, str(log))
    return log


def git(*args, cwd=SOURCE):
    log = successful("git-" + args[0].replace("/", "-"), ["git", *args], 20, cwd=cwd)
    return log.read_bytes()


manifest = json.loads((HARNESS / "validation/manifest.json").read_text())
assert manifest["base"] == BASE
assert manifest["repository"] == REPOSITORY and manifest["ref"] == REF
assert set(manifest["validationFiles"]) == HARNESS_FILES
assert len(manifest["validationFiles"]) == 4
assert set(manifest["productFiles"]) == PRODUCT_FILES
assert len(manifest["productFiles"]) == 3
assert manifest["generatedDirectories"] == [GENERATED_DIR]
assert set(manifest["identities"]) == PRODUCT_FILES
assert manifest["toolchain"] == RUST_VERSION
assert os.environ["GITHUB_REPOSITORY"] == REPOSITORY
assert os.environ["GITHUB_EVENT_NAME"] == "workflow_dispatch"
assert os.environ["GITHUB_REF"] == REF
assert git("rev-parse", "HEAD").decode().strip() == BASE
assert git("rev-parse", "HEAD", cwd=HARNESS).decode().strip() == os.environ["GITHUB_SHA"]
assert git("show", "-s", "--format=%P", "HEAD", cwd=HARNESS).decode().strip() == BASE, (
    "Validation commit must have exactly one parent, the fixed product base"
)
assert set(git("diff", "--name-only", BASE, "HEAD", cwd=HARNESS).decode().splitlines()) == HARNESS_FILES
assert git("status", "--porcelain", cwd=HARNESS) == b""
assert git("status", "--porcelain") == b""
assert HARNESS.resolve() == (ROOT / "harness").resolve()
assert Path(os.environ["CARGO_TARGET_DIR"]).resolve() == TARGET.resolve()
assert os.environ["CARGO_BUILD_JOBS"] == "2"
assert not TARGET.exists(), "Fresh controlled build directory required"
for path in (SOURCE / "tauri", SOURCE / "tauri/gen", SOURCE / GENERATED_DIR):
    assert not path.is_symlink(), "Generated schema parent chain must not contain symlinks"
assert not (SOURCE / GENERATED_DIR).exists(), "Generated schemas must be absent before validation"
patch = HARNESS / "validation/candidate.diff"
assert sha(patch) == manifest["patchSha256"]
assert sha(Path(__file__)) == manifest["validatorSha256"]
harness_hashes = {path: sha(HARNESS / path) for path in sorted(HARNESS_FILES)}
locks = {path: sha(SOURCE / path) for path in ("pnpm-lock.yaml", "tauri/Cargo.lock")}
assert locks == manifest["locks"]
save("harness-identities.json", {
    "base": BASE, "validationCommit": os.environ["GITHUB_SHA"], "files": harness_hashes,
    "hashChain": "workflow pins patch/manifest/validator; manifest pins patch/validator/product/locks; workflow own hash recorded",
})
for path, identity in manifest["identities"].items():
    if identity["baseBlob"] is None:
        assert not (SOURCE / path).exists()
        assert git("ls-tree", BASE, "--", path) == b""
    else:
        assert git("rev-parse", BASE + ":" + path).decode().strip() == identity["baseBlob"]
        assert sha(SOURCE / path) == identity["baseSha256"]
original_stream = git("show", BASE + ":" + STREAM_FILE)
assert hashlib.sha256(original_stream).hexdigest() == manifest["identities"][STREAM_FILE]["baseSha256"]
git("apply", "--check", "--index", str(patch))
git("apply", "--index", str(patch))
fixed_stream = (SOURCE / STREAM_FILE).read_bytes()
candidate_index_diff = git("diff", "--cached", "--binary", "HEAD")
(EVIDENCE / "candidate.diff").write_bytes(patch.read_bytes())
(EVIDENCE / "tested.diff").write_bytes(candidate_index_diff)


def generated_schema_snapshot():
    # Tauri's existing build script writes only this explicitly ignored schema tree.
    # It is build evidence, never an additional product-file change allowance.
    root = SOURCE / GENERATED_DIR
    for path in (SOURCE / "tauri", SOURCE / "tauri/gen", root):
        assert not path.is_symlink(), "Generated schema parent chain must not contain symlinks"
    if not root.exists():
        return {"exists": False, "files": {}, "directories": []}
    assert root.is_dir(), "Generated schema path must be a directory"
    files = {}
    directories = [GENERATED_DIR]
    for path in sorted(root.rglob("*")):
        assert not path.is_symlink(), "Generated schemas must not contain symlinks"
        relative = path.relative_to(SOURCE).as_posix()
        if path.is_dir():
            directories.append(relative)
        else:
            assert path.is_file(), "Generated schemas must contain only regular files/directories"
            files[relative] = sha(path)
    return {"exists": True, "files": files, "directories": directories}


def check_source(label, baseline=False):
    assert git("rev-parse", "HEAD").decode().strip() == BASE
    assert set(git("diff", "--cached", "--name-only", "HEAD").decode().splitlines()) == PRODUCT_FILES
    assert set(git("diff", "--name-only").decode().splitlines()) == ({STREAM_FILE} if baseline else set())
    assert set(git("diff", "HEAD", "--name-only").decode().splitlines()) == (
        PRODUCT_FILES - {STREAM_FILE} if baseline else PRODUCT_FILES
    )
    git("diff", "--cached", "--check")
    assert git("diff", "--cached", "--binary", "HEAD") == candidate_index_diff
    files = {}
    for path, identity in manifest["identities"].items():
        actual = SOURCE / path
        assert actual.is_file() and not actual.is_symlink()
        files[path] = sha(actual)
        expected = identity["baseSha256"] if baseline and path == STREAM_FILE else identity["sha256"]
        assert files[path] == expected, (label, path)
    assert {path: sha(SOURCE / path) for path in locks} == locks
    unknown = git("ls-files", "--others", "--exclude-standard").decode().splitlines()
    # Enumerate files: --directory can also report the unignored tauri/gen/ parent.
    ignored = git("ls-files", "--others", "--ignored", "--exclude-standard").decode().splitlines()
    save("untracked-" + label + ".json", {"unknown": unknown, "ignored": ignored})
    assert unknown == []
    allowed_ignored = ("tauri/target/", GENERATED_DIR + "/")
    assert all(any(path == prefix or path.startswith(prefix) for prefix in allowed_ignored) for path in ignored), ignored
    generated = generated_schema_snapshot()
    save("generated-schemas-" + label + ".json", generated)
    assert {path: sha(HARNESS / path) for path in HARNESS_FILES} == harness_hashes
    assert git("status", "--porcelain", cwd=HARNESS) == b""
    save("source-identities-" + label + ".json", {
        "base": BASE, "baselineImplementation": baseline, "files": files,
        "locks": locks, "patchSha256": sha(patch), "generatedSchemas": generated,
    })


def compile_binary(phase, seconds):
    log = successful(phase + "-compile", [
        "cargo", "+" + RUST_VERSION, "test", "--locked", "--jobs", "2",
        "--manifest-path", "tauri/Cargo.toml", "--no-default-features", "--lib",
        "--no-run", "--message-format=json",
    ], seconds)
    binaries = []
    finished = []
    for line in log.read_text().splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if record.get("reason") == "build-finished":
            finished.append(record.get("success"))
        if record.get("reason") == "compiler-artifact" and record.get("profile", {}).get("test") and record.get("target", {}).get("name") == "ai_toolbox_lib" and record.get("executable"):
            assert set(record["target"]["kind"]) == {"staticlib", "cdylib", "rlib"}
            assert Path(record["target"]["src_path"]).resolve() == (SOURCE / "tauri/src/lib.rs").resolve()
            assert Path(record["manifest_path"]).resolve() == (SOURCE / "tauri/Cargo.toml").resolve()
            binaries.append(record["executable"])
    assert finished == [True] and len(binaries) == 1, (phase, finished, binaries)
    binary = Path(binaries[0]).resolve(strict=True)
    assert binary.is_relative_to(TARGET.resolve(strict=True))
    assert binary.is_file() and os.access(binary, os.X_OK)
    digest = sha(binary)
    save(phase + "-binary.json", {"path": str(binary), "sha256": digest, "target": "ai_toolbox_lib"})
    return binary, digest


test_home = Path(os.environ["RUNNER_TEMP"]) / "reasoning-item-ids-test-home"
test_home.mkdir(exist_ok=False)
test_env = {
    "HOME": str(test_home), "USERPROFILE": str(test_home),
    "XDG_CONFIG_HOME": str(test_home / "config"),
    "XDG_DATA_HOME": str(test_home / "data"),
    "XDG_CACHE_HOME": str(test_home / "cache"),
    "APPDATA": str(test_home / "appdata"), "LOCALAPPDATA": str(test_home / "localappdata"),
    "RUST_BACKTRACE": "0", "RUST_TEST_THREADS": "1",
}


def test_run(name, binary, digest, args, seconds):
    assert sha(binary) == digest, "Test executable changed before use"
    result = run(name, [str(binary), *args], seconds, extra_env=test_env)
    assert sha(binary) == digest, "Test executable changed during execution"
    return result


def list_tests(label, binary, digest, ignored=False):
    args = [MODULE, "--list"] + (["--ignored"] if ignored else [])
    code, log_path = test_run(label, binary, digest, args, 20)
    assert code == 0
    lines = log_path.read_text().splitlines()
    names = [line[:-6] for line in lines if line.endswith(": test")]
    summaries = [re.fullmatch(r"(\d+) tests?, (\d+) benchmarks?", line) for line in lines]
    summaries = [match for match in summaries if match]
    assert len(summaries) == 1
    assert int(summaries[0][1]) == len(names) and int(summaries[0][2]) == 0
    assert len(names) == len(set(names)) and all(name.startswith(MODULE) for name in names)
    if ignored:
        assert names == [], "No ignored transformer tests are permitted"
    else:
        assert len(names) > 8 and set(manifest["rustTests"]).issubset(names)
    save(label + ".json", {"tests": names, "count": len(names), "ignoredOnly": ignored})
    return names


def result_counts(log, status):
    matches = re.findall(
        r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;",
        log, re.MULTILINE,
    )
    assert len(matches) == 1 and matches[0][0] == status, "Missing or ambiguous libtest result"
    return tuple(map(int, matches[0][1:]))


def exact_test(phase, name, binary, digest, red=None):
    short = name.removeprefix(TEST_MODULE)
    code, listing = test_run(phase + "-list-" + short, binary, digest, ["--exact", name, "--list"], 15)
    assert code == 0
    lines = listing.read_text().splitlines()
    assert [line for line in lines if line.endswith(": test")] == [name + ": test"]
    assert "1 test, 0 benchmarks" in lines
    code, path = test_run(phase + "-" + short, binary, digest, [
        "--exact", name, "--test-threads=1", "--nocapture",
    ], 45)
    log = path.read_text()
    assert re.search(r"^running 1 test$", log, re.MULTILINE)
    if red is None:
        assert code == 0 and result_counts(log, "ok")[:4] == (1, 0, 0, 0)
        assert "test " + name + " ... ok" in log and "panicked at" not in log
    else:
        assert code == 101, "A setup failure, timeout or signal is not product RED"
        assert result_counts(log, "FAILED")[:4] == (0, 1, 0, 0)
        assert re.findall(r"^thread '([^']+)'(?: \(\d+\))? panicked at ", log, re.MULTILINE) == [name]
        assert log.count("panicked at") == 1
        location = rf"panicked at [^\n]*reasoning_item_id_tests\.rs:{red['panicLine']}:\d+:"
        assert re.search(location, log), "RED must be the reviewed assertion, not another panic"
        assert red["message"] in log
        if red["kind"] == "collision":
            assert "assertion `left != right` failed" in log
            assert re.search(r'left: "reasoning_0"', log)
            assert re.search(r'right: "reasoning_0"', log)
        assert re.search(r"^    " + re.escape(name) + r"$", log, re.MULTILINE)
    check_source(phase + "-" + short, baseline=phase == "baseline")
    return {"name": name, "exitCode": code, "classification": "expected-product-red" if red else "green"}


outcome = {"status": "incomplete", "base": BASE, "syntheticProtocolFixturesOnly": True}
try:
    check_source("initial")
    successful("apt-update", ["sudo", "apt-get", "update"], 180)
    successful("apt-install", [
        "sudo", "apt-get", "install", "-y", "libwebkit2gtk-4.1-dev", "librsvg2-dev",
        "patchelf", "libssl-dev", "libgtk-3-dev", "libayatana-appindicator3-dev",
    ], 300)
    successful("rust-install", ["rustup", "toolchain", "install", RUST_VERSION, "--profile", "minimal"], 240)
    rustc_log = successful("rust-version", ["rustc", "+" + RUST_VERSION, "--version"], 15)
    assert rustc_log.read_text().startswith("rustc " + RUST_VERSION + " ")
    successful("cargo-version", ["cargo", "+" + RUST_VERSION, "--version"], 15)
    check_source("prerequisites")
    (SOURCE / STREAM_FILE).write_bytes(original_stream)
    check_source("baseline-before", baseline=True)
    baseline_binary, baseline_hash = compile_binary("baseline", 900)
    check_source("baseline-compiled", baseline=True)
    baseline_inventory = list_tests("baseline-inventory", baseline_binary, baseline_hash)
    list_tests("baseline-ignored", baseline_binary, baseline_hash, ignored=True)
    assert len(manifest["redTests"]) == 6 and len(manifest["controlTests"]) == 2
    assert len(manifest["rustTests"]) == len(set(manifest["rustTests"])) == 8
    assert set(manifest["redTests"]) | set(manifest["controlTests"]) == set(manifest["rustTests"])
    assert set(manifest["redTests"]).isdisjoint(manifest["controlTests"])
    baseline_results = [
        exact_test("baseline", name, baseline_binary, baseline_hash, red=red)
        for name, red in manifest["redTests"].items()
    ]
    baseline_results.extend(
        exact_test("baseline", name, baseline_binary, baseline_hash)
        for name in manifest["controlTests"]
    )
    save("baseline-results.json", baseline_results)
    (SOURCE / STREAM_FILE).write_bytes(fixed_stream)
    check_source("fixed-before")
    fixed_binary, fixed_hash = compile_binary("fixed", 480)
    assert fixed_hash != baseline_hash, "Implementation change must produce a different executable"
    check_source("fixed-compiled")
    fixed_inventory = list_tests("fixed-inventory", fixed_binary, fixed_hash)
    assert fixed_inventory == baseline_inventory, "Only implementation may differ between phases"
    list_tests("fixed-ignored", fixed_binary, fixed_hash, ignored=True)
    fixed_results = [exact_test("fixed", name, fixed_binary, fixed_hash) for name in manifest["rustTests"]]
    save("fixed-results.json", fixed_results)
    code, full_log = test_run("transformer-full", fixed_binary, fixed_hash, [MODULE, "--test-threads=1"], 300)
    log = full_log.read_text()
    assert code == 0 and result_counts(log, "ok")[:4] == (len(fixed_inventory), 0, 0, 0)
    passed = re.findall(r"^test (\S+) \.\.\. ok$", log, re.MULTILINE)
    assert len(passed) == len(set(passed)) == len(fixed_inventory)
    assert set(passed) == set(fixed_inventory) and "panicked at" not in log
    check_source("final")
    outcome.update({
        "status": "passed", "expectedProductRed": 6, "baselineControlsPassed": 2,
        "fixedExactPassed": 8, "transformerPassed": len(passed), "ignored": 0,
        "baselineExecutableSha256": baseline_hash, "fixedExecutableSha256": fixed_hash,
        "candidateAndLocksUnchanged": True,
    })
except BaseException as error:
    outcome.update({"status": "failed", "errorType": type(error).__name__, "error": str(error)})
    raise
finally:
    in_cleanup = True
    (SOURCE / STREAM_FILE).write_bytes(fixed_stream)
    try:
        check_source("finally")
        assert all(row["survivors"] == [] and row["cleanupError"] is None for row in process_records)
    except BaseException as error:
        outcome.update({"status": "failed", "finallyError": type(error).__name__ + ": " + str(error)})
        raise
    finally:
        save("result.json", outcome)
