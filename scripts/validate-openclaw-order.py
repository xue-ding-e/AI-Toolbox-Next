#!/usr/bin/env python3
"""One bounded, manual validation of the reviewed OpenClaw roundtrip candidate."""
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import time

BASE = "e222d60165c78addeab4a74ad3e2964600ee7f31"
REPOSITORY = "xue-ding-e/AI-Toolbox-Next"
REF = "refs/heads/validation/openclaw-order-roundtrip"
PRODUCT = "tauri/src/coding/open_claw/roundtrip.rs"
BASE_BLOB = "1b6de9287a1633adeab62208e5d41336c3e2c68f"
BASE_SHA256 = "396306f2eda83b09a97dd8a0062625912a76393f988f081b33afc4e8f2c692b2"
CANDIDATE = "scripts/fixtures/openclaw-order-candidate.rs"
CANDIDATE_SHA256 = "e2303c973ae7461ec81362e1bec0a8ebcd1b6c0d9de698013ff4be38060157b5"
PRODUCTION_PATCH = "scripts/fixtures/openclaw-order-production.diff"
PRODUCTION_PATCH_SHA256 = "72532aa510cf5fe5b04db7a8de3f19e698dda888557b4ae6dfcc8eaa8cf312c1"
BASELINE_SHA256 = "e454c6d30fbd6ff2e624c8e33bf7b3e2ddf69e185e050478bdb7370fc515afb4"
HARNESS_FILES = {
    ".github/workflows/maintenance-validation.yml",
    "scripts/validate-openclaw-order.py", CANDIDATE, PRODUCTION_PATCH,
}
PREREQUISITE_HASHES = {
    "tauri/build.rs": "359ec8dc0b6ed902368db3454011b55a62d3d85a86b2ad199c92764fd69e581a",
    "tauri/tauri.conf.json": "a3ff6886316687d5d61d67ff99485f04c8ff9876742fae5f8b9f2a60782a469f",
    "tauri/Cargo.toml": "4858cc9123480a46f7c8f27463cfb2adbaf076ecf459e26277f7a280d796cd6f",
}
LOCK_BLOBS = {
    "tauri/Cargo.lock": "2f2ebbce97a84fd3c43dd82d45fb51958edd39e1",
    "pnpm-lock.yaml": "d6056e1c3282a22654350d59faab37cbb0e5d9c8",
}
OTHER_WORKFLOWS = {
    ".github/workflows/compaction-regression.yml": "78f4d73e6ae94c13c45b086eadbf5e2eb631c69d69cc5ea38004c5ac4677f08f",
    ".github/workflows/maintenance-structure.yml": "f67752389d5f4976cee9334f99e851b3ea598c17914e24ecb3bf516afce95b1a",
}
MODULE = "coding::open_claw::roundtrip::tests::"
RUST_VERSION = "1.99.1"
LOG_SOFT_LIMIT = 32 * 1024 * 1024
COMPILE_BUDGET = 25 * 60
GENERATED = "tauri/gen/schemas"
DIST_BYTES = b"<!doctype html><html><head><title>Synthetic Rust test build</title></head><body></body></html>\n"
TEST_MARKER = b"#[cfg(test)]\nmod tests {"
# Inventory is explicit: no test is accepted by count alone.
INVENTORY = {'prefix': 'coding::open_claw::roundtrip::tests::',
 'red': ['provider_reorder_changes_document_despite_value_equality'],
 'baselineControls': ['scan_health_detects_known_openclaw_issues',
                      'scan_health_is_silent_on_valid_config',
                      'scan_health_from_source_reports_parse_failed',
                      'migrate_legacy_timeout_moves_to_timeout_seconds',
                      'migrate_legacy_timeout_keeps_existing_timeout_seconds',
                      'migrate_legacy_timeout_is_noop_without_agents',
                      'value_to_rt_value_serializes_empty_providers_without_panic',
                      'roundtrip_preserves_top_level_comments_and_adds_section',
                      'save_detects_external_conflict',
                      'noop_save_skips_backup',
                      'remove_root_section_middle_preserves_others',
                      'remove_root_section_last_and_first',
                      'apply_root_section_diff_adds_removes_updates_only_changed',
                      'provider_reorder_conflict_keeps_external_bytes',
                      'provider_reorder_ignores_non_provider_object_key_order',
                      'provider_reorder_still_persists_semantic_non_models_changes',
                      'provider_reorder_still_persists_nested_provider_changes'],
 'green': ['scan_health_detects_known_openclaw_issues',
           'scan_health_is_silent_on_valid_config',
           'scan_health_from_source_reports_parse_failed',
           'migrate_legacy_timeout_moves_to_timeout_seconds',
           'migrate_legacy_timeout_keeps_existing_timeout_seconds',
           'migrate_legacy_timeout_is_noop_without_agents',
           'value_to_rt_value_serializes_empty_providers_without_panic',
           'roundtrip_preserves_top_level_comments_and_adds_section',
           'save_detects_external_conflict',
           'noop_save_skips_backup',
           'remove_root_section_middle_preserves_others',
           'remove_root_section_last_and_first',
           'apply_root_section_diff_adds_removes_updates_only_changed',
           'provider_reorder_changes_document_despite_value_equality',
           'provider_reorder_saves_reloads_and_backs_up_original_bytes',
           'provider_reorder_repeated_order_skips_write_and_backup',
           'provider_reorder_can_move_back_and_reload',
           'provider_reorder_conflict_keeps_external_bytes',
           'provider_reorder_backup_failure_keeps_original_bytes',
           'provider_reorder_ignores_non_provider_object_key_order',
           'provider_reorder_still_persists_semantic_non_models_changes',
           'provider_reorder_still_persists_nested_provider_changes'],
 'redRequiredFailure': 'assertion order actual a,b expected b,a; compilation/setup failure '
                       'excluded'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def sha(path):
    return digest(path.read_bytes())


def rust_tests(data):
    return [name.decode() for name in re.findall(rb"#\[test\]\s+fn ([a-z_0-9]+)\(", data)]


def members(group):
    """Only non-zombie processes can still execute; scan the owned Linux group."""
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


class Validation:
    def __init__(self):
        self.root = Path(os.environ["GITHUB_WORKSPACE"]).resolve()
        self.harness = Path(__file__).resolve().parents[1]
        self.source = self.root / "source"
        self.target = self.source / "tauri/target"
        self.evidence = Path(os.environ["RUNNER_TEMP"]) / "openclaw-order-evidence"
        self.evidence.mkdir(exist_ok=True)
        self.outcome = {"status": "incomplete", "stage": "initialization", "base": BASE,
                        "expectedRed": 1, "expectedBaselineControls": 17, "expectedFixed": 22,
                        "scope": "Synthetic OpenClaw roundtrip library tests only; no application launch"}
        self.save("result.json", self.outcome)
        self.started = time.monotonic()
        self.deadline = self.started
        self.final_deadline = self.started
        self.in_cleanup = False
        self.sequence = 0
        self.processes = []
        self.tests = []
        self.compile_seconds = 0.0
        self.prepared = False
        self.owned_home = False
        self.harness_hashes = {}
        self.locks = {}

    def save(self, name, value):
        (self.evidence / name).write_text(json.dumps(value, indent=2) + "\n")

    def stage(self, label):
        self.outcome["stage"] = label
        self.save("result.json", self.outcome)
        print(label, flush=True)

    def run(self, name, args, seconds, cwd=None, env=None):
        """Every attempted command records exit/reason/survivors, including failure paths."""
        self.sequence += 1
        stem = f"{self.sequence:04d}-{name}"
        log_path = self.evidence / (stem + ".log")
        cwd = cwd or self.source
        process = None
        reason = None
        survivors = None
        cleanup_error = None
        command_error = None
        started = time.monotonic()
        budget_deadline = self.final_deadline if self.in_cleanup else self.deadline
        local_deadline = started + seconds
        deadline = min(local_deadline, budget_deadline)
        with log_path.open("wb") as log:
            try:
                if seconds <= 0 or started >= deadline:
                    reason = "deadline-before-spawn"
                    raise RuntimeError(reason)
                process = subprocess.Popen(args, cwd=cwd, stdout=log, stderr=subprocess.STDOUT,
                                           start_new_session=True, env=env or os.environ.copy())
                while process.poll() is None:
                    if time.monotonic() >= deadline:
                        reason = "overall-deadline" if deadline < local_deadline else "timeout"
                        break
                    if log_path.stat().st_size > LOG_SOFT_LIMIT:
                        reason = "log-soft-limit"
                        break
                    time.sleep(0.1)
                if log_path.stat().st_size > LOG_SOFT_LIMIT:
                    reason = reason or "log-soft-limit"
                survivors = members(process.pid)
                if survivors:
                    reason = reason or "surviving-process-group"
            except BaseException as error:
                command_error = type(error).__name__ + ": " + str(error)
                reason = reason or "spawn-or-interrupt-error"
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
                            while time.monotonic() < until:
                                if process.poll() is not None and not members(process.pid):
                                    break
                                time.sleep(0.1)
                        survivors = members(process.pid)
                    else:
                        survivors = []
                except BaseException as error:
                    cleanup_error = type(error).__name__ + ": " + str(error)
                finally:
                    code = process.poll() if process is not None else None
                    record = {
                        "name": name, "command": args, "cwd": str(cwd),
                        "pid": process.pid if process is not None else None,
                        "exitCode": code, "reason": reason or "exited",
                        "survivors": survivors, "cleanupError": cleanup_error,
                        "spawnOrInterruptError": command_error,
                        "seconds": round(time.monotonic() - started, 3),
                        "timeoutSeconds": seconds, "logBytes": log_path.stat().st_size,
                        "logSoftLimit": LOG_SOFT_LIMIT,
                    }
                    self.save(stem + "-process.json", record)
                    self.processes.append(record)
                    self.save("processes.json", self.processes)
        assert cleanup_error is None and code is not None and survivors == [], record
        assert reason is None and command_error is None, record
        return code, log_path

    def successful(self, name, args, seconds, **kwargs):
        code, log = self.run(name, args, seconds, **kwargs)
        assert code == 0, (name, code, str(log))
        return log

    def git(self, *args, cwd=None):
        return self.successful("git-" + args[0], ["git", *args], 30, cwd=cwd).read_bytes()

    def preflight(self):
        elapsed = time.time() - float(os.environ["VALIDATION_STARTED_EPOCH"])
        assert 0 <= elapsed < 42 * 60, "Invalid or expired workflow clock"
        self.deadline = time.monotonic() + 42 * 60 - elapsed
        self.final_deadline = self.deadline + 60
        self.stage("Verify exact source, four-file harness, and manual branch guards")
        assert os.environ["GITHUB_REPOSITORY"] == REPOSITORY
        assert os.environ["GITHUB_EVENT_NAME"] == "workflow_dispatch"
        assert os.environ["GITHUB_REF"] == REF
        assert self.harness == self.root / "harness"
        assert self.git("rev-parse", "HEAD", cwd=self.harness).decode().strip() == os.environ["GITHUB_SHA"]
        assert self.git("show", "-s", "--format=%P", "HEAD", cwd=self.harness).decode().strip() == BASE
        assert set(self.git("diff", "--name-only", BASE, "HEAD", cwd=self.harness).decode().splitlines()) == HARNESS_FILES
        assert self.git("status", "--porcelain", cwd=self.harness) == b""
        assert self.git("rev-parse", "HEAD").decode().strip() == BASE
        assert self.git("status", "--porcelain") == b""
        assert self.git("ls-files", "--others", "--ignored", "--exclude-standard") == b""
        self.harness_hashes = {path: sha(self.harness / path) for path in sorted(HARNESS_FILES)}
        assert self.harness_hashes[CANDIDATE] == CANDIDATE_SHA256
        assert self.harness_hashes[PRODUCTION_PATCH] == PRODUCTION_PATCH_SHA256
        assert all(not (self.harness / path).is_symlink() for path in HARNESS_FILES)
        assert self.git("rev-parse", BASE + ":" + PRODUCT).decode().strip() == BASE_BLOB
        original = self.git("show", BASE + ":" + PRODUCT)
        assert digest(original) == BASE_SHA256 == sha(self.source / PRODUCT)
        for path, expected in PREREQUISITE_HASHES.items():
            assert sha(self.source / path) == expected
        for path, blob in LOCK_BLOBS.items():
            assert self.git("rev-parse", BASE + ":" + path).decode().strip() == blob
        self.locks = {path: sha(self.source / path) for path in LOCK_BLOBS}
        assert {path: sha(self.harness / path) for path in OTHER_WORKFLOWS} == OTHER_WORKFLOWS
        workflow_files = set(self.git("ls-files", ".github/workflows", cwd=self.harness).decode().splitlines())
        assert workflow_files == set(OTHER_WORKFLOWS) | {".github/workflows/maintenance-validation.yml"}
        # The pinned remaining workflows are manual-only, or main push/PR only.
        # This isolated branch has no push-triggered workflow and no release workflow.
        self.save("harness-identities.json", {"base": BASE, "commit": os.environ["GITHUB_SHA"],
                  "files": self.harness_hashes, "otherWorkflowHashes": OTHER_WORKFLOWS,
                  "singleParent": BASE, "firstPushTriggers": [], "locks": self.locks,
                  "hashChain": "workflow pins driver/candidate/production patch; driver pins source, locks and prerequisites; workflow hash recorded"})
        assert Path(os.environ["CARGO_TARGET_DIR"]).resolve() == self.target.resolve()
        assert os.environ["CARGO_BUILD_JOBS"] == "2"
        assert not self.target.exists(), "No pre-existing target cache is allowed"
        for relative in ("tauri", "tauri/gen", GENERATED, "dist"):
            assert not (self.source / relative).is_symlink()
        assert not (self.source / GENERATED).exists() and not (self.source / "dist").exists()
        candidate = (self.harness / CANDIDATE).read_bytes()
        assert candidate.count(TEST_MARKER) == original.count(TEST_MARKER) == 1
        baseline = original.split(TEST_MARKER)[0] + TEST_MARKER + candidate.split(TEST_MARKER)[1]
        assert digest(baseline) == BASELINE_SHA256
        assert baseline.startswith(original[:-2] + b"\n") and baseline.endswith(b"}\n")
        assert baseline.split(TEST_MARKER)[0] == original.split(TEST_MARKER)[0]
        assert rust_tests(original) == INVENTORY["baselineControls"][:13]
        assert rust_tests(candidate) == rust_tests(baseline) == INVENTORY["green"]
        assert len(set(INVENTORY["green"])) == 22
        assert len(INVENTORY["baselineControls"]) == 17 and len(INVENTORY["red"]) == 1
        assert set(INVENTORY["baselineControls"]).isdisjoint(INVENTORY["red"])
        assert set(INVENTORY["baselineControls"] + INVENTORY["red"]).issubset(INVENTORY["green"])
        assert baseline.count(b"        assert_eq!(actual, keys);") == 1
        self.panic_line = baseline[:baseline.index(b"        assert_eq!(actual, keys);")].count(b"\n") + 1
        assert self.panic_line == 850
        self.candidate, self.baseline = candidate, baseline
        (self.source / PRODUCT).write_bytes(candidate)
        patch = str(self.harness / PRODUCTION_PATCH)
        self.git("apply", "--reverse", "--check", patch)
        self.git("apply", "--reverse", patch)
        assert (self.source / PRODUCT).read_bytes() == baseline
        (self.source / PRODUCT).write_bytes(candidate)
        self.tested_diff = self.git("diff", "--binary", "--", PRODUCT)
        (self.evidence / "tested.diff").write_bytes(self.tested_diff)
        (self.evidence / "production.diff").write_bytes((self.harness / PRODUCTION_PATCH).read_bytes())
        self.prepared = True
        conf = json.loads((self.source / "tauri/tauri.conf.json").read_text())
        assert conf["build"]["frontendDist"] == "../dist"
        (self.source / "dist").mkdir()
        (self.source / "dist/index.html").write_bytes(DIST_BYTES)
        self.save("build-prerequisite.json", {"frontendDist": "../dist", "path": "dist/index.html",
                  "sha256": digest(DIST_BYTES), "reason": "Synthetic Tauri compile input, not a product change or frontend build",
                  "buildRsSha256": PREREQUISITE_HASHES["tauri/build.rs"], "applicationStarted": False})
        self.test_home = Path(os.environ["RUNNER_TEMP"]) / "openclaw-order-test-home"
        self.test_home.mkdir(exist_ok=False)
        self.owned_home = True
        for child in ("config", "data", "cache", "appdata", "localappdata", "tmp"):
            (self.test_home / child).mkdir()
        # A minimal test environment excludes runner tokens and real account locations.
        self.test_env = {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8", "HOME": str(self.test_home),
                         "USERPROFILE": str(self.test_home), "XDG_CONFIG_HOME": str(self.test_home / "config"),
                         "XDG_DATA_HOME": str(self.test_home / "data"), "XDG_CACHE_HOME": str(self.test_home / "cache"),
                         "APPDATA": str(self.test_home / "appdata"), "LOCALAPPDATA": str(self.test_home / "localappdata"),
                         "TMPDIR": str(self.test_home / "tmp"), "RUST_BACKTRACE": "0", "RUST_TEST_THREADS": "1"}
        self.check_source("prepared", baseline=False)

    def check_test_home(self, label):
        # Metadata only: never read unexpected files, and never delete a leak.
        root_info = self.test_home.lstat()
        assert stat.S_ISDIR(root_info.st_mode) and not stat.S_ISLNK(root_info.st_mode)

        def fail_scan(error):
            raise error

        entries = {}
        for root, directories, files in os.walk(self.test_home, followlinks=False, onerror=fail_scan):
            for name in sorted(directories + files):
                path = Path(root) / name
                info = path.lstat()
                entries[path.relative_to(self.test_home).as_posix()] = {
                    "mode": info.st_mode, "size": info.st_size,
                    "mtimeNs": info.st_mtime_ns,
                    "isDirectory": stat.S_ISDIR(info.st_mode),
                    "isSymlink": stat.S_ISLNK(info.st_mode),
                }
        self.save("test-home-" + label + ".json", {"entries": entries, "deleted": False})
        assert not self.test_home.is_symlink()
        assert set(entries) == {"config", "data", "cache", "appdata", "localappdata", "tmp"}, "Test home leak: see metadata evidence"
        assert all(item["isDirectory"] and not item["isSymlink"] for item in entries.values())

    def fast_identities(self, baseline):
        assert sha(self.source / PRODUCT) == (BASELINE_SHA256 if baseline else CANDIDATE_SHA256)
        assert {path: sha(self.source / path) for path in self.locks} == self.locks
        assert {path: sha(self.harness / path) for path in self.harness_hashes} == self.harness_hashes

    def check_source(self, label, baseline):
        self.fast_identities(baseline)
        assert self.git("rev-parse", "HEAD").decode().strip() == BASE
        assert self.git("diff", "--cached", "--name-only") == b""
        assert set(self.git("diff", "--name-only", "HEAD").decode().splitlines()) == {PRODUCT}
        self.git("diff", "--check")
        if not baseline:
            assert self.git("diff", "--binary", "--", PRODUCT) == self.tested_diff
        unknown = self.git("ls-files", "--others", "--exclude-standard").decode().splitlines()
        ignored = self.git("ls-files", "--others", "--ignored", "--exclude-standard").decode().splitlines()
        self.save("untracked-" + label + ".json", {"unknown": unknown, "ignored": ignored})
        assert unknown == []
        assert all(path == "dist/index.html" or path.startswith("tauri/target/") or
                   path.startswith(GENERATED + "/") for path in ignored), ignored
        assert (self.source / "dist/index.html").read_bytes() == DIST_BYTES
        assert not (self.source / "dist").is_symlink()
        assert not (self.source / "dist/index.html").is_symlink()
        generated = {}
        for relative in ("tauri", "tauri/gen", GENERATED):
            assert not (self.source / relative).is_symlink()
        root = self.source / GENERATED
        if root.exists():
            assert root.is_dir()
            for path in sorted(root.rglob("*")):
                assert not path.is_symlink()
                if not path.is_dir():
                    assert path.is_file()
                    generated[path.relative_to(self.source).as_posix()] = sha(path)
        assert self.git("status", "--porcelain", cwd=self.harness) == b""
        self.save("source-" + label + ".json", {"sourceSha256": sha(self.source / PRODUCT),
                  "baselineImplementation": baseline, "locks": self.locks, "generatedSchemas": generated,
                  "syntheticDistSha256": digest(DIST_BYTES), "productionFilesChanged": [PRODUCT]})

    def compile(self, phase):
        self.stage(phase + ": compile actual locked project library with two jobs")
        remaining = COMPILE_BUDGET - self.compile_seconds
        started = time.monotonic()
        try:
            log = self.successful(phase + "-compile", ["cargo", "+" + RUST_VERSION, "test", "--locked",
                    "--jobs", "2", "--manifest-path", "tauri/Cargo.toml", "--lib", "--no-run",
                    "--message-format=json"], remaining)
        finally:
            self.compile_seconds += time.monotonic() - started
            self.save("compile-budget.json", {"totalBudgetSeconds": COMPILE_BUDGET,
                      "consumedSeconds": round(self.compile_seconds, 3),
                      "cacheBoundary": "Fresh target before baseline; only same-job baseline dependencies reused by fixed compile; no external cache"})
        binaries, finished = [], []
        for line in log.read_text(errors="replace").splitlines():
            try:
                item = json.loads(line)
            except json.JSONDecodeError:
                continue
            if item.get("reason") == "build-finished":
                finished.append(item.get("success"))
            if (item.get("reason") == "compiler-artifact" and item.get("profile", {}).get("test")
                    and item.get("target", {}).get("name") == "ai_toolbox_lib" and item.get("executable")):
                assert set(item["target"]["kind"]) == {"staticlib", "cdylib", "rlib"}
                assert Path(item["target"]["src_path"]).resolve() == self.source / "tauri/src/lib.rs"
                assert Path(item["manifest_path"]).resolve() == self.source / "tauri/Cargo.toml"
                binaries.append(item["executable"])
        assert finished == [True] and len(binaries) == 1, (finished, binaries)
        raw_binary = Path(binaries[0])
        binary = raw_binary.resolve(strict=True)
        assert not self.target.is_symlink() and not raw_binary.is_symlink()
        assert binary.is_relative_to(self.target.resolve(strict=True))
        assert binary.is_file() and os.access(binary, os.X_OK)
        identity = sha(binary)
        self.save(phase + "-binary.json", {"path": str(binary), "sha256": identity, "target": "ai_toolbox_lib"})
        return binary, identity

    def test_run(self, label, binary, identity, args, seconds=90):
        assert sha(binary) == identity
        result = self.run(label, [str(binary), *args], seconds, env=self.test_env)
        assert sha(binary) == identity
        return result

    def inventory(self, phase, binary, identity):
        results = {}
        for ignored in (False, True):
            label = phase + ("-ignored-inventory" if ignored else "-inventory")
            args = [MODULE, "--list"] + (["--ignored"] if ignored else [])
            code, log = self.test_run(label, binary, identity, args, 30)
            assert code == 0
            lines = log.read_text().splitlines()
            names = [line[:-6] for line in lines if line.endswith(": test")]
            summaries = [re.fullmatch(r"(\d+) tests?, (\d+) benchmarks?", line) for line in lines]
            summaries = [match for match in summaries if match]
            assert len(summaries) == 1 and tuple(map(int, summaries[0].groups())) == (len(names), 0)
            expected = [] if ignored else [MODULE + name for name in INVENTORY["green"]]
            assert len(names) == len(set(names)) == len(expected) and set(names) == set(expected)
            self.save(label + ".json", {"names": names, "count": len(names), "ignored": ignored})
            results[ignored] = names
        return results

    @staticmethod
    def counts(log, expected_status):
        found = re.findall(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", log, re.MULTILINE)
        assert len(found) == 1 and found[0][0] == expected_status, "Missing or ambiguous libtest summary"
        return tuple(map(int, found[0][1:]))

    def exact(self, phase, short, binary, identity, red=False):
        name = MODULE + short
        self.fast_identities(phase == "baseline")
        code, listing = self.test_run(phase + "-list-" + short, binary, identity, [name, "--exact", "--list"], 30)
        lines = listing.read_text().splitlines()
        assert code == 0 and [line for line in lines if line.endswith(": test")] == [name + ": test"]
        assert "1 test, 0 benchmarks" in lines
        code, log_path = self.test_run(phase + "-" + short, binary, identity,
                                      [name, "--exact", "--test-threads=1", "--nocapture"])
        log = log_path.read_text()
        assert re.search(r"^running 1 test$", log, re.MULTILINE)
        if red:
            assert short == "provider_reorder_changes_document_despite_value_equality"
            assert code == 101, "Timeout, signal, compiler/setup failure is not product RED"
            assert self.counts(log, "FAILED")[:4] == (0, 1, 0, 0)
            assert re.findall(r"^thread '([^']+)'(?: \(\d+\))? panicked at ", log, re.MULTILINE) == [name]
            assert log.count("panicked at") == 1
            assert re.search(rf"panicked at [^\n]*coding/open_claw/roundtrip\.rs:{self.panic_line}:\d+:", log)
            assert "assertion `left == right` failed" in log
            assert re.search(r'^\s*left: \["a", "b"\]$', log, re.MULTILINE)
            assert re.search(r'^\s*right: \["b", "a"\]$', log, re.MULTILINE)
            assert re.findall(r"^    (" + re.escape(MODULE) + r"\S+)$", log, re.MULTILINE) == [name]
        else:
            assert code == 0 and self.counts(log, "ok")[:4] == (1, 0, 0, 0)
            assert "test " + name + " ... ok" in log and "panicked at" not in log
        self.fast_identities(phase == "baseline")
        self.check_test_home(phase + "-" + short)
        self.tests.append({"phase": phase, "name": name, "exitCode": code,
                           "classification": "expected-product-red" if red else "green",
                           "log": log_path.name})
        self.save("tests.json", self.tests)

    def execute(self):
        self.preflight()
        self.stage("Install existing Linux build dependencies and official pinned Rust toolchain")
        self.successful("apt-update", ["sudo", "apt-get", "update"], 180)
        self.successful("apt-install", ["sudo", "apt-get", "install", "-y", "libwebkit2gtk-4.1-dev",
                        "librsvg2-dev", "patchelf", "libssl-dev", "libgtk-3-dev", "libayatana-appindicator3-dev"], 300)
        rust_env = {**os.environ, "RUSTUP_DIST_SERVER": "https://static.rust-lang.org",
                    "RUSTUP_UPDATE_ROOT": "https://static.rust-lang.org/rustup"}
        self.successful("rust-install", ["rustup", "toolchain", "install", RUST_VERSION, "--profile", "minimal"], 240, env=rust_env)
        version = self.successful("rust-version", ["rustc", "+" + RUST_VERSION, "--version"], 15)
        assert version.read_text().startswith("rustc " + RUST_VERSION + " ")
        self.successful("cargo-version", ["cargo", "+" + RUST_VERSION, "--version"], 15)
        self.successful("disk", ["df", "-h"], 15)
        self.check_source("prerequisites", baseline=False)
        (self.source / PRODUCT).write_bytes(self.baseline)
        self.check_source("baseline-before", baseline=True)
        baseline_binary, baseline_hash = self.compile("baseline")
        self.check_source("baseline-compiled", baseline=True)
        baseline_inventory = self.inventory("baseline", baseline_binary, baseline_hash)
        self.stage("Baseline: require one exact order-assertion RED and all 17 controls GREEN")
        self.exact("baseline", INVENTORY["red"][0], baseline_binary, baseline_hash, red=True)
        for name in INVENTORY["baselineControls"]:
            self.exact("baseline", name, baseline_binary, baseline_hash)
        self.check_source("baseline-tested", baseline=True)
        (self.source / PRODUCT).write_bytes(self.candidate)
        self.check_source("fixed-before", baseline=False)
        fixed_binary, fixed_hash = self.compile("fixed")
        assert fixed_hash != baseline_hash, "Changed production code must yield a different executable"
        self.check_source("fixed-compiled", baseline=False)
        assert self.inventory("fixed", fixed_binary, fixed_hash) == baseline_inventory
        self.stage("Fixed: require all 22 exact roundtrip tests GREEN")
        for name in INVENTORY["green"]:
            self.exact("fixed", name, fixed_binary, fixed_hash)
        self.check_source("final", baseline=False)
        assert len(self.tests) == 40
        self.outcome.update({"status": "passed", "stage": "complete", "expectedProductRed": 1,
                             "baselineControlsPassed": 17, "fixedExactPassed": 22, "ignored": 0,
                             "baselineExecutableSha256": baseline_hash, "fixedExecutableSha256": fixed_hash,
                             "candidateAndLocksUnchanged": True, "testedDiffSha256": digest(self.tested_diff)})

    def finish(self):
        self.in_cleanup = True
        errors = []
        if self.owned_home:
            try:
                self.check_test_home("finally")
            except BaseException as error:
                errors.append("test-home: " + type(error).__name__ + ": " + str(error))
        try:
            if self.prepared:
                (self.source / PRODUCT).write_bytes(self.candidate)
                self.check_source("finally", baseline=False)
            assert all(row["survivors"] == [] and row["cleanupError"] is None for row in self.processes)
        except BaseException as error:
            errors.append("source-or-processes: " + type(error).__name__ + ": " + str(error))
        finally:
            if errors:
                self.outcome.update({"status": "failed", "finallyErrors": errors})
            self.outcome["compileSeconds"] = round(self.compile_seconds, 3)
            self.outcome["completedExactTests"] = len(self.tests)
            self.save("result.json", self.outcome)
        return 0 if self.outcome["status"] == "passed" else 1


def main():
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    validation = Validation()
    try:
        validation.execute()
    except BaseException as error:
        validation.outcome.update({"status": "failed", "errorType": type(error).__name__, "error": str(error)})
        print(type(error).__name__ + ": " + str(error), flush=True)
    finally:
        code = validation.finish()
    raise SystemExit(code)


if __name__ == "__main__":
    main()
