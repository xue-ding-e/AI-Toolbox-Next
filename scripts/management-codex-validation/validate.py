#!/usr/bin/env python3
"""Draft, Ubuntu-only bounded validation for issue #41 PR-B.

Review and supply the pinned candidate.json and candidate.patch before use.
Only the reviewed patch is applied to an ephemeral BASE worktree. No branch
mutation, app install, GUI startup, frontend build, or broad Tauri tests occur.
Every external command in this Python harness runs in an owned session;
non-zombie survivors are failures. Apt setup remains a separate workflow step.
The runner job is the outer containment boundary, not a reusable host daemon runner.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import signal
import subprocess
import sys
import time

BASE = "f81e71ac1f232940e9bbc1039afe0198d06eb103"
REPO = "xue-ding-e/AI-Toolbox-Next"
MANIFEST_SHA256 = "9f6b82d32103a22d02be584b74dfd020ee7b5126431a4d22fcc8603af8d17865"
CANDIDATE = "e1d1e6fe11a556016c59964b95e0c3cd572785ac"
PATCH_SHA256 = "18af77ac89efd05c7c67edbb58fa80249c54854e163eeaf0ad7fcd8ea04bb780"
BRANCH = "refs/heads/validation/management-codex-core"
CONTROL_PATHS = {
    ".github/workflows/maintenance-validation.yml",
    "scripts/management-codex-validation/validate.py",
    "scripts/management-codex-validation/candidate.json",
    "scripts/management-codex-validation/candidate.patch",
}
MERGER_TESTS = sorted([
    "coding::codex::commands::tests::append_toml_configs_keeps_common_root_keys_at_root",
    "coding::codex::commands::tests::append_toml_configs_merges_common_tables_without_overwriting_provider_table",
    "coding::codex::commands::tests::append_toml_configs_keeps_explicit_provider_scalar_overrides",
])
# Fixed from the actual 18-test inventory recorded in core-validation-final.log.
CORE_TESTS = sorted([
    "plan::tests::provider_cannot_hide_invalid_common_master_data",
    "plan::tests::equal_numeric_patch_preserves_original_lexical_spelling",
    "plan::tests::plans_requested_root_values_without_io_or_capacity_claim",
    "plan::tests::absent_context_is_not_invented_from_model_cache",
    "plan::tests::preserves_comments_unknown_sections_and_auth_text",
    "plan::tests::provider_wins_and_shadowing_is_explicit",
    "plan::tests::rejects_non_integer_effective_value_and_allows_equal_limit",
    "plan::tests::rejects_zero_overflow_empty_and_invalid_toml",
    "plan::tests::validates_effective_threshold_even_when_only_context_changes",
    "plan::tests::repeated_patch_is_byte_noop",
    "tests::append_rejects_invalid_toml_when_composing_two_documents",
    "tests::append_preserves_existing_empty_side_fast_path",
    "tests::append_keeps_unmodified_comments_and_provider_precedence",
    "tests::append_toml_configs_keeps_explicit_provider_scalar_overrides",
    "tests::append_toml_configs_keeps_common_root_keys_at_root",
    "tests::parse_empty_document_and_report_context",
    "tests::merge_replaces_non_tables_instead_of_inventing_array_or_inline_merges",
    "tests::append_toml_configs_merges_common_tables_without_overwriting_provider_table",
])
LOG_LIMIT = 16 * 1024 * 1024  # Per command, both streams; exceedance fails closed.
TOTAL_LOG_LIMIT = 64 * 1024 * 1024
RECORDS = []
TOTAL_LOG_BYTES = 0
ACTIVE = None
EVIDENCE = None
PHASE = "unknown"
JOB_DEADLINE = None


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    result = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def require(condition, reason):
    if not condition:
        raise RuntimeError(reason)


def write_json(name, value):
    destination = EVIDENCE / name
    temporary = destination.with_suffix(destination.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    temporary.replace(destination)


def process_table():
    """PID identities include start ticks, preventing PID-reuse cleanup mistakes."""
    result = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            raw = (entry / "stat").read_text()
            fields = raw[raw.rindex(")") + 2:].split()
            result[int(entry.name)] = {
                "state": fields[0], "ppid": int(fields[1]),
                "pgid": int(fields[2]), "sid": int(fields[3]),
                "start": int(fields[19]),
            }
        except (FileNotFoundError, ProcessLookupError):
            pass
    return result


def owned_members(leader, known):
    table = process_table()
    # Same session/group plus previously observed descendants, including those
    # that changed group/session. This does not promise containment of hostile
    # descendants that escape before their first observation; runner job owns
    # that outer boundary. No test in the exact inventory spawns descendants.
    changed = True
    while changed:
        changed = False
        for pid, item in table.items():
            parent = table.get(item["ppid"])
            known_parent = parent and known.get(item["ppid"]) == parent["start"]
            if item["sid"] == leader or item["pgid"] == leader or known_parent:
                if pid not in known:
                    known[pid] = item["start"]
                    changed = True
    return {
        pid: item for pid, item in table.items()
        if known.get(pid) == item["start"] and item["state"] != "Z"
    }


def cleanup(child, known):
    """Bounded TERM then KILL, only verified owned identities; always reap leader."""
    for sig, seconds in [(signal.SIGTERM, 2), (signal.SIGKILL, 3)]:
        members = owned_members(child.pid, known)
        for pid, item in members.items():
            # Recheck identity immediately before signaling this specific PID.
            current = process_table().get(pid)
            if current and current["start"] == item["start"]:
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    pass
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            child.poll()
            if not owned_members(child.pid, known):
                break
            time.sleep(0.05)
    try:
        child.wait(timeout=1)
    except subprocess.TimeoutExpired:
        pass
    return owned_members(child.pid, known)


def interrupted(signum, _frame):
    raise RuntimeError(f"interrupted by signal {signum}")


def run(label, command, timeout, cwd, env, allowed_codes=(0,)):
    """Nonblocking pipe drain plus owned process observation with hard deadlines."""
    global ACTIVE, TOTAL_LOG_BYTES
    require(re.fullmatch(r"[a-z0-9-]+", label), "unsafe log label")
    output_paths = {key: EVIDENCE / f"{PHASE}-{label}.{key}.log" for key in ("stdout", "stderr")}
    require(not any(path.exists() for path in output_paths.values()), "duplicate command label")
    started = time.monotonic()
    requested_timeout = timeout
    if JOB_DEADLINE is not None:
        timeout = min(timeout, JOB_DEADLINE - time.time() - 10)
    require(timeout > 0, "job evidence/cleanup reserve reached")
    record = {"label": label, "argv": command, "timeout_seconds": timeout,
              "requested_timeout_seconds": requested_timeout,
              "exit_code": None, "reason": "not_started", "survivors": {},
              "log_bytes_seen": 0, "logs": {key: path.name for key, path in output_paths.items()}}
    child = None
    known = {}
    selector = selectors.DefaultSelector()
    files = {}
    reason = "exception"
    try:
        files = {key: path.open("wb") for key, path in output_paths.items()}
        child = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                 start_new_session=True)
        ACTIVE = child.pid
        record["pid"] = child.pid
        initial = process_table().get(child.pid)
        if initial:
            known[child.pid] = initial["start"]
        for stream, key in [(child.stdout, "stdout"), (child.stderr, "stderr")]:
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, key)
        deadline = started + timeout
        while True:
            exit_code = child.poll()
            members = owned_members(child.pid, known)
            if exit_code is not None and members:
                reason = "surviving_descendants_after_parent_exit"
                break
            if time.monotonic() >= deadline:
                reason = "timeout"
                break
            for key, _mask in selector.select(timeout=0.05):
                try:
                    data = os.read(key.fileobj.fileno(), 65536)
                except BlockingIOError:
                    continue
                if not data:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                remaining = min(LOG_LIMIT - record["log_bytes_seen"], TOTAL_LOG_LIMIT - TOTAL_LOG_BYTES)
                files[key.data].write(data[:max(0, remaining)])
                record["log_bytes_seen"] += len(data)
                TOTAL_LOG_BYTES += len(data)
            if record["log_bytes_seen"] > LOG_LIMIT or TOTAL_LOG_BYTES > TOTAL_LOG_LIMIT:
                reason = "log_soft_limit_exceeded"
                break
            if exit_code is not None and not selector.get_map():
                reason = "completed" if exit_code in allowed_codes else "nonzero_exit"
                break
    finally:
        try:
            if child is not None:
                try:
                    record["survivors"] = cleanup(child, known)
                    record["exit_code"] = child.poll()
                except BaseException as cleanup_error:
                    reason = "cleanup_failed"
                    record["survivors"] = {"unknown": "cleanup could not be confirmed"}
                    record["cleanup_error"] = f"{type(cleanup_error).__name__}: {cleanup_error}"
        finally:
            ACTIVE = None
            record["reason"] = reason
            record["duration_seconds"] = round(time.monotonic() - started, 3)
            for key in list(selector.get_map().values()):
                key.fileobj.close()
            selector.close()
            for file in files.values():
                file.close()
            RECORDS.append(record)
            write_json(f"{PHASE}-commands.json", RECORDS)
    require(reason == "completed" and not record["survivors"], f"{label}: {reason}; see command evidence")
    return output_paths["stdout"].read_bytes()


def clean_env(test_root=None):
    # Never inherit tokens, proxy credentials, DISPLAY, arbitrary compiler flags,
    # preload settings, or credentials from action context into cargo/tests.
    allowed = ["PATH", "HOME", "RUSTUP_HOME", "CARGO_HOME"]
    env = {key: os.environ[key] for key in allowed if key in os.environ}
    env.update({"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "CARGO_BUILD_JOBS": "2",
                "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0",
                "CARGO_INCREMENTAL": "0", "RUST_BACKTRACE": "1", "CARGO_TERM_COLOR": "never"})
    if test_root:
        for key in ["HOME", "USERPROFILE", "XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME",
                    "AI_TOOLBOX_DATA_DIR", "AI_TOOLBOX_CACHE_DIR", "CODEX_HOME", "CLAUDE_CONFIG_DIR",
                    "TMPDIR", "TMP", "TEMP"]:
            directory = test_root / key
            directory.mkdir()
            env[key] = str(directory)
        for key in ["RUSTUP_HOME", "CARGO_HOME"]:
            env.pop(key, None)
    return env


def source_check(root, manifest):
    """Verify the four-file harness checkout, not an applied product candidate."""
    require(os.environ.get("GITHUB_REPOSITORY") == REPO, "repository guard failed")
    require(os.environ.get("GITHUB_REF") == BRANCH, "branch guard failed")
    require(os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch", "manual-only guard failed")
    sha = os.environ.get("GITHUB_SHA", "")
    require(re.fullmatch(r"[0-9a-f]{40}", sha), "missing validation SHA")
    require(os.environ.get("EXPECTED_SHA") == sha, "dispatch expected SHA does not match checkout event")
    require(digest(json.dumps(manifest, sort_keys=True).encode()) == MANIFEST_SHA256,
            "manifest differs from exact reviewed PR-B manifest; no scope expansion")
    require(manifest.get("source_base") == BASE, "fixed baseline mismatch")
    require(manifest.get("candidate_commit") == CANDIDATE, "candidate provenance mismatch")
    require(manifest.get("patch_sha256") == PATCH_SHA256, "reviewed patch mismatch")
    require(sorted(manifest.get("core_tests", [])) == CORE_TESTS, "core inventory changed")
    require(sorted(manifest.get("desktop_tests", [])) == MERGER_TESTS, "desktop inventory changed")
    env = clean_env()
    git = lambda label, args: run(label, ["git", *args], 15, root, env)
    head = git("head", ["rev-parse", "HEAD"]).decode().strip()
    require(head == sha, "checkout does not match github.sha")
    parents = git("single-parent", ["rev-list", "--parents", "-n", "1", "HEAD"]).decode().split()
    require(parents == [head, BASE], "validation HEAD must have exactly the fixed base as its sole parent")
    require(not git("clean-checkout", ["status", "--porcelain", "--untracked-files=all"]).strip(), "checkout is dirty")
    changed = set(git("changed-paths", ["diff", "--name-only", BASE, "HEAD"]).decode().splitlines())
    require(changed == CONTROL_PATHS, "validation branch must change exactly the four harness files")
    patch_path = root / "scripts/management-codex-validation/candidate.patch"
    require(not patch_path.is_symlink() and file_digest(patch_path) == PATCH_SHA256,
            "stored candidate patch does not match fixed reviewed digest")
    base_lock = git("baseline-lock", ["show", f"{BASE}:tauri/Cargo.lock"])
    base_tree = git("baseline-tree", ["rev-parse", f"{BASE}^{{tree}}"])
    write_json(f"{PHASE}-source.json", {
        "base_commit": BASE, "base_tree": base_tree.decode().strip(),
        "validation_commit": head, "candidate_commit_provenance_only": CANDIDATE,
        "patch_sha256": PATCH_SHA256, "candidate_files_expected": manifest["candidate_files"],
        "base_tauri_lock_sha256": digest(base_lock),
        "harness_sha256": file_digest(Path(__file__)),
        "workflow_sha256": file_digest(root / ".github/workflows/maintenance-validation.yml"),
        "manifest_canonical_sha256": MANIFEST_SHA256,
    })
    return patch_path


def prepare_source(checkout, run_root, manifest, patch_path, env):
    """Apply only the reviewed patch to a new detached temporary BASE worktree.

    Staging is local only, needed for canonical diffs including newly added files.
    No candidate commit object, full fetch, checkout mutation, or product push is
    required. The ephemeral worktree is retained until the runner is discarded;
    neither its contents nor its target directory are uploaded.
    """
    root = run_root / "source"
    run("create-source-worktree", ["git", "worktree", "add", "--detach", str(root), BASE],
        60, checkout, env)
    run("apply-reviewed-patch", ["git", "apply", "--index", "--", str(patch_path)], 30, root, env)
    files = manifest["candidate_files"]
    for path, expected in files.items():
        require(not Path(path).is_absolute() and ".." not in Path(path).parts, "unsafe candidate path")
        resolved = (root / path).resolve(strict=True)
        require(resolved.is_relative_to(root) and resolved.is_file(), "candidate escaped temp source")
        require(file_digest(resolved) == expected, f"candidate hash mismatch: {path}")
    changed = run("patched-paths", ["git", "diff", "--cached", "--name-only", "HEAD"], 15, root, env)
    require(set(changed.decode().splitlines()) == set(files), "patched source paths differ from reviewed ten")
    unstaged = run("patched-unstaged", ["git", "diff", "--name-only"], 15, root, env)
    untracked = run("patched-untracked", ["git", "ls-files", "--others", "--exclude-standard"], 15, root, env)
    require(not unstaged.strip() and not untracked.strip(), "unexpected unstaged/untracked product files")
    patch = run("canonical-patch", ["git", "diff", "--cached", "--binary", "--full-index", "--no-ext-diff",
                "--no-renames", "HEAD", "--", *sorted(files)], 15, root, env)
    require(digest(patch) == PATCH_SHA256, "applied canonical patch hash differs")
    write_json("validation-patched-source.json", {
        "base_commit": BASE, "candidate_commit_provenance_only": CANDIDATE,
        "canonical_patch_sha256": digest(patch), "files": files,
        "tauri_lock_sha256": file_digest(root / "tauri/Cargo.lock"),
        "worktree": str(root), "retention": "ephemeral runner only, not uploaded",
    })
    return root, files


def artifact(output, manifest_path, target_name, target):
    matches = []
    for line in output.decode().splitlines():
        message = json.loads(line)
        if message.get("reason") != "compiler-artifact":
            continue
        if (Path(message["manifest_path"]).resolve() == manifest_path.resolve()
                and message.get("profile", {}).get("test") is True
                and message.get("target", {}).get("name") == target_name
                and set(message.get("target", {}).get("kind", [])) & {"lib", "rlib", "staticlib", "cdylib"}
                and message.get("executable")):
            executable = Path(message["executable"]).resolve(strict=True)
            require(executable.is_relative_to(target) and executable.is_file()
                    and os.access(executable, os.X_OK), "test executable escaped owned target")
            matches.append(executable)
    require(len(matches) == 1, "expected exactly one matching real library test artifact")
    return matches[0]


def inventory(output):
    text = output.decode()
    names = sorted(line[:-6] for line in text.splitlines() if line.endswith(": test"))
    require(len(names) == len(set(names)), "duplicate test inventory")
    require(not any(line.endswith(": benchmark") for line in text.splitlines()), "unexpected benchmark inventory")
    return names


def check_pass(output, count, filtered=None):
    text = output.decode()
    pattern = rf"^test result: ok\. {count} passed; 0 failed; 0 ignored; 0 measured; (\d+) filtered out;"
    matches = re.findall(pattern, text, re.M)
    require(len(matches) == 1, "test summary missing, ignored, or wrong pass count")
    if filtered is not None:
        require(int(matches[0]) == filtered, "unexpected filtered test count")


def validate(checkout, manifest):
    patch_path = source_check(checkout, manifest)
    env = clean_env()
    root = checkout
    version = run("rust-version", ["rustc", "--version"], 15, root, env).decode().strip()
    require(version.startswith("rustc 1.99.1 "), "Rust version is not pinned 1.99.1")
    run("cargo-version", ["cargo", "--version"], 15, root, env)
    run_root = Path(os.environ["RUNNER_TEMP"]).resolve() / ("management-codex-" + os.environ["GITHUB_RUN_ID"] + "-" + os.environ["GITHUB_RUN_ATTEMPT"])
    run_root.mkdir()  # No cache reuse or pre-existing build artifacts.
    root, files = prepare_source(checkout, run_root, manifest, patch_path, env)
    target = run_root / "target"
    target.mkdir()
    env["CARGO_TARGET_DIR"] = str(target)
    dist = root / "dist"
    require(not dist.exists(), "refuse to replace existing frontend build")
    asset_created = False
    selected = {}
    before = {path: digest((root / path).read_bytes()) for path in files}
    try:
        # Tauri build.rs still executes normally. Empty compile-only embedding
        # asset satisfies frontendDist without pnpm, web builds, or UI claims.
        dist.mkdir()
        asset_created = True
        (dist / "index.html").write_bytes(b"")
        write_json("validation-compile-asset.json", {"path": "dist/index.html", "sha256": digest(b""),
                    "purpose": "empty compile-only asset, NOT frontend/UI validation"})
        tauri_manifest = root / "tauri/Cargo.toml"
        compiled = run("tauri-compile", ["cargo", "test", "--locked", "--manifest-path", str(tauri_manifest),
                       "--lib", "--no-run", "--jobs", "2", "--message-format=json"], 1800, root, env)
        binary = artifact(compiled, tauri_manifest, "ai_toolbox_lib", target)
        isolated = run_root / "tauri-test-environment"
        isolated.mkdir()
        test_env = clean_env(isolated)
        listed = run("tauri-list", [str(binary), "append_toml_configs", "--list"], 30, isolated, test_env)
        require(inventory(listed) == MERGER_TESTS, "real Tauri merger inventory differs from exact reviewed three")
        selected["tauri"] = {"binary": str(binary), "sha256": file_digest(binary), "tests": MERGER_TESTS}
        for index, name in enumerate(MERGER_TESTS):
            output = run(f"tauri-case-{index + 1}", [str(binary), name, "--exact", "--test-threads=1"], 90, isolated, test_env)
            check_pass(output, 1)
        core_manifest = root / "crates/ai-toolbox-codex-config/Cargo.toml"
        core_output = run("core-compile", ["cargo", "test", "--locked", "--manifest-path", str(core_manifest),
                          "--lib", "--no-run", "--jobs", "2", "--message-format=json"], 120, root, env)
        core_binary = artifact(core_output, core_manifest, "ai_toolbox_codex_config", target)
        core_root = run_root / "core-test-environment"
        core_root.mkdir()
        core_env = clean_env(core_root)
        core_list = run("core-list", [str(core_binary), "--list"], 30, core_root, core_env)
        require(len(CORE_TESTS) >= 18 and inventory(core_list) == CORE_TESTS, "core exact >=18 inventory mismatch")
        selected["core"] = {"binary": str(core_binary), "sha256": file_digest(core_binary), "tests": CORE_TESTS}
        core_result = run("core-tests", [str(core_binary), "--test-threads=1"], 120, core_root, core_env)
        check_pass(core_result, len(CORE_TESTS), 0)
    finally:
        after = {}
        integrity_errors = []
        for path in files:
            try:
                after[path] = file_digest(root / path)
            except OSError as error:
                after[path] = None
                integrity_errors.append(f"{path}: {type(error).__name__}: {error}")
        # Write the independently known evidence before attempting asset cleanup.
        # These hashes cover candidate files only, not an assertion about the tree.
        write_json("validation-inventory.json", selected)
        write_json("validation-integrity.json", {
            "candidate_files_before": before, "candidate_files_after": after,
            "candidate_files_unchanged": before == after,
            "errors": integrity_errors,
        })
        cleanup_error = None
        try:
            # No recursive cleanup. Preserve unexpected contents for diagnosis.
            if asset_created:
                asset = dist / "index.html"
                if asset.is_file() and not asset.is_symlink() and asset.read_bytes() == b"":
                    asset.unlink()
                dist.rmdir()
        except OSError as error:
            cleanup_error = f"{type(error).__name__}: {error}"
        write_json("validation-asset-cleanup.json", {
            "asset_was_created": asset_created, "dist_exists_after_cleanup": dist.exists(),
            "error": cleanup_error,
        })
        final_tree = {
            "unstaged_paths": None, "untracked_paths": None, "cached_paths": None,
            "expected_cached_paths": sorted(files), "expected_patch_sha256": PATCH_SHA256,
            "errors": [],
        }
        # Observe the complete Git state after cleanup, even if cleanup failed.
        for label, key, command in [
            ("post-unstaged", "unstaged_paths", ["git", "diff", "--name-only"]),
            ("post-untracked", "untracked_paths", ["git", "ls-files", "--others", "--exclude-standard"]),
            ("post-cached", "cached_paths", ["git", "diff", "--cached", "--name-only", "HEAD"]),
        ]:
            try:
                final_tree[key] = sorted(run(label, command, 15, root, env).decode().splitlines())
            except BaseException as error:
                final_tree["errors"].append(f"{label}: {type(error).__name__}: {error}")
        write_json("validation-final-git-state.json", final_tree)
        require(cleanup_error is None, "compile asset cleanup failed; see cleanup and Git-state evidence")
        require(before == after, "validation changed reviewed candidate source/lock files")
        require(not final_tree["errors"] and final_tree["unstaged_paths"] == []
                and final_tree["untracked_paths"] == []
                and final_tree["cached_paths"] == sorted(files),
                "final Git state was not confirmed as exactly the reviewed staged ten-file patch")


def main():
    global EVIDENCE, PHASE, JOB_DEADLINE
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=["preflight", "validation"], required=True)
    parser.add_argument("--manifest", required=True)
    options = parser.parse_args()
    PHASE = options.phase
    require(sys.platform == "linux", "Ubuntu runner required")
    require(os.geteuid() != 0, "source validation, compilation, and tests must remain unprivileged")
    root = Path(os.environ["GITHUB_WORKSPACE"]).resolve()
    EVIDENCE = Path(os.environ["RUNNER_TEMP"]).resolve() / "management-codex-evidence"
    require(not EVIDENCE.is_symlink(), "evidence directory must not be a symlink")
    EVIDENCE.mkdir(exist_ok=True)
    require(not (EVIDENCE / f"{PHASE}-summary.json").exists(), "phase already executed; no blind retry")
    # Reserve at least two minutes for evidence upload plus three for checkout
    # before preflight. Enforce remaining wall time within every command, rather
    # than relying on the 45-minute Actions hard kill to run Python finally.
    clock_path = EVIDENCE / "preflight-budget.json"
    if PHASE == "preflight":
        require(not clock_path.exists(), "budget already initialized")
        write_json(clock_path.name, {"deadline_unix": time.time() + 39 * 60})
    JOB_DEADLINE = json.loads(clock_path.read_text())["deadline_unix"]
    summary = {"phase": PHASE, "ok": False, "error": None}
    for sig in [signal.SIGTERM, signal.SIGINT]:
        signal.signal(sig, interrupted)
    try:
        manifest_path = (root / options.manifest).resolve(strict=True)
        require(manifest_path == root / "scripts/management-codex-validation/candidate.json",
                "manifest must be the fixed harness file")
        manifest = json.loads(manifest_path.read_text())
        if PHASE == "validation":
            validate(root, manifest)
        else:
            source_check(root, manifest)
        summary["ok"] = True
    except BaseException as error:
        summary["error"] = f"{type(error).__name__}: {error}"
    finally:
        summary["commands"] = len(RECORDS)
        summary["log_bytes_seen"] = TOTAL_LOG_BYTES
        write_json(f"{PHASE}-summary.json", summary)
        print(json.dumps(summary, sort_keys=True), flush=True)
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
