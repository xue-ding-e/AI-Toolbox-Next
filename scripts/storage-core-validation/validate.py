#!/usr/bin/env python3
"""Draft, Ubuntu-only bounded validation for issue #41 PR-C.

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

BASE = "e1d1e6fe11a556016c59964b95e0c3cd572785ac"
REPO = "xue-ding-e/AI-Toolbox-Next"
MANIFEST_SHA256 = "2f0d1634bda117c4e6e69aa034582a229264e904aa67e22368f56b9e11a2e7d3"
CANDIDATE = "233a77ad8afd2f5ce3588050e8e5c3f06b61a93e"
PATCH_SHA256 = "e4b1e79944ed93f57f5abf4acf6c9af92af140bc6dd0d82dd28a882138d5e990"
BRANCH = "refs/heads/validation/shared-storage-core"
CONTROL_PATHS = {
    ".github/workflows/maintenance-validation.yml",
    "scripts/storage-core-validation/validate.py",
    "scripts/storage-core-validation/candidate.json",
    "scripts/storage-core-validation/candidate.patch",
}
INVENTORIES = {'core_unit': ['model_pricing_seed::tests::bundled_model_pricing_includes_gpt_5_6_and_grok_4_5', 'model_pricing_seed::tests::bundled_model_pricing_json_is_valid', 'model_pricing_seed::tests::seed_inserts_missing_rows_and_preserves_existing_rows', 'model_pricing_seed::tests::seed_rejects_duplicate_model_ids'], 'core_sqlite': ['standalone_file_open_rejects_future_schema_without_modifying_bytes', 'standalone_state_initializes_schema_jsonb_and_bundled_pricing'], 'desktop_sqlite': ['already_migrated_v18_with_latency_samples_preserves_existing_measurements', 'already_migrated_v18_without_latency_samples_is_repaired_with_backup', 'backup_api_creates_readable_sqlite_file', 'create_generates_clean_id_and_count_delete_work', 'delete_all_clears_only_target_table', 'future_backup_schema_message_is_user_facing_and_detectable', 'invalid_identifier_and_json_path_are_rejected', 'jsonb_probe_and_schema_migration_create_all_tables', 'list_orders_by_json_integer_then_column', 'patch_preserves_unknown_fields_and_supports_nested_paths', 'patch_where_bool_updates_all_matching_records', 'put_derives_columns_from_numeric_json_timestamps_without_changing_json_type', 'put_get_stores_jsonb_blob_and_injects_metadata', 'put_keeps_column_and_json_timestamps_aligned_on_update', 'query_by_bool_string_number_and_max_work', 'query_by_field_supports_array_and_object_values', 'schema_migration_rejects_future_user_version', 'session_usage_migrations_preserve_rollups_and_ledger_on_repeat', 'sqlite_state_limits_lock_scope_with_closure_api', 'sqlite_state_open_creates_pre_migration_backup_before_schema_upgrade', 'sqlite_state_open_rejects_future_user_version_before_file_initialization', 'transaction_rolls_back_on_error', 'update_applied_status_switches_records_inside_one_transaction', 'update_hook_records_changed_table_names', 'v17_migration_preserves_existing_request_logs_and_nullable_effort', 'v20_usage_metadata_upgrade_preserves_logs_rollups_and_import_ledger', 'v21_websocket_upgrade_defaults_old_logs_to_http_without_changing_usage', 'v27_migration_preserves_existing_request_logs_and_nullable_service_tier', 'v28_migration_preserves_existing_request_logs_and_nullable_compaction_mode', 'v29_adds_the_credits_columns_in_the_same_step_as_the_project_dimension', 'v29_migration_rebuilds_rollups_with_the_project_column'], 'desktop_reexports': ['desktop_reexports_share_storage_types_and_connection']}
CORE_TESTS = ['model_pricing_seed::tests::bundled_model_pricing_includes_gpt_5_6_and_grok_4_5', 'model_pricing_seed::tests::bundled_model_pricing_json_is_valid', 'model_pricing_seed::tests::seed_inserts_missing_rows_and_preserves_existing_rows', 'model_pricing_seed::tests::seed_rejects_duplicate_model_ids', 'standalone_file_open_rejects_future_schema_without_modifying_bytes', 'standalone_state_initializes_schema_jsonb_and_bundled_pricing']
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
                    fd = os.pidfd_open(pid)
                    try:
                        verified = process_table().get(pid)
                        if verified and verified["start"] == item["start"]:
                            signal.pidfd_send_signal(fd, sig)
                    finally:
                        os.close(fd)
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
    env["RUSTUP_HOME"] = os.environ.get("RUSTUP_HOME", str(Path(os.environ["HOME"]) / ".rustup"))
    if test_root:
        for key in ["HOME", "USERPROFILE", "XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME",
                    "AI_TOOLBOX_DATA_DIR", "AI_TOOLBOX_CACHE_DIR", "CODEX_HOME", "CLAUDE_CONFIG_DIR",
                    "TMPDIR", "TMP", "TEMP"]:
            directory = test_root / key
            directory.mkdir(parents=True)
            env[key] = str(directory)
        env["GIT_CONFIG_NOSYSTEM"] = "1"
        env["GIT_CONFIG_GLOBAL"] = "/dev/null"
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
            "manifest differs from exact reviewed PR-C manifest; no scope expansion")
    require(manifest.get("source_base") == BASE, "fixed baseline mismatch")
    require(manifest.get("candidate_commit") == CANDIDATE, "candidate provenance mismatch")
    require(manifest.get("patch_sha256") == PATCH_SHA256, "reviewed patch mismatch")
    require(sorted(manifest.get("core_tests", [])) == CORE_TESTS, "core inventory changed")
    require(manifest.get("inventories") == INVENTORIES, "exact inventories changed")
    require(len(manifest["candidate_files"]) == 29 and len(manifest["preserved_files"]) == 7, "manifest scope changed")
    env = clean_env(EVIDENCE / (PHASE + "-git-home"))
    git = lambda label, args: run(label, ["git", *args], 15, root, env)
    head = git("head", ["rev-parse", "HEAD"]).decode().strip()
    require(head == sha, "checkout does not match github.sha")
    parents = git("single-parent", ["rev-list", "--parents", "-n", "1", "HEAD"]).decode().split()
    require(parents == [head, BASE], "validation HEAD must have exactly the fixed base as its sole parent")
    require(not git("clean-checkout", ["status", "--porcelain", "--untracked-files=all"]).strip(), "checkout is dirty")
    changed = set(git("changed-paths", ["diff", "--no-renames", "--name-only", BASE, "HEAD"]).decode().splitlines())
    require(changed == CONTROL_PATHS, "validation branch must change exactly the four harness files")
    patch_path = root / "scripts/storage-core-validation/candidate.patch"
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
    for path, expected in {**files, **manifest["preserved_files"]}.items():
        require(not Path(path).is_absolute() and ".." not in Path(path).parts, "unsafe candidate path")
        if expected is None:
            require(not (root / path).exists() and not (root / path).is_symlink(), "deleted file remains")
            continue
        require(not (root / path).is_symlink(), "candidate symlink refused")
        resolved = (root / path).resolve(strict=True)
        require(resolved.is_relative_to(root) and resolved.is_file(), "candidate escaped temp source")
        require(file_digest(resolved) == expected, f"candidate hash mismatch: {path}")
    changed = run("patched-paths", ["git", "diff", "--cached", "--no-renames", "--name-only", "HEAD"], 15, root, env)
    require(set(changed.decode().splitlines()) == set(files), "patched source paths differ from reviewed twenty-nine")
    status = run("patched-status", ["git", "diff", "--cached", "--name-status", "--no-renames", "HEAD"], 15, root, env)
    require(dict((line.split("\t")[1], line.split("\t")[0]) for line in status.decode().splitlines()) == manifest["candidate_statuses"], "patched statuses differ")
    unstaged = run("patched-unstaged", ["git", "diff", "--no-renames", "--name-only"], 15, root, env)
    untracked = run("patched-untracked", ["git", "ls-files", "--others", "--exclude-standard"], 15, root, env)
    require(not unstaged.strip() and not untracked.strip(), "unexpected unstaged/untracked product files")
    patch = run("canonical-patch", ["git", "diff", "--cached", "--binary", "--full-index", "--no-ext-diff",
                "--no-renames", "HEAD", "--", *sorted(files)], 15, root, env)
    require(digest(patch) == PATCH_SHA256, "applied canonical patch hash differs")
    write_json("validation-patched-source.json", {
        "base_commit": BASE, "candidate_commit_provenance_only": CANDIDATE,
        "canonical_patch_sha256": digest(patch), "files": files,
        "tauri_lock_sha256": file_digest(root / "tauri/Cargo.lock"),
        "storage_lock_sha256": file_digest(root / "crates/ai-toolbox-storage/Cargo.lock"),
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
                and set(message.get("target", {}).get("kind", [])) & {"lib", "rlib", "staticlib", "cdylib", "test"}
                and message.get("executable")):
            executable = Path(message["executable"]).resolve(strict=True)
            require(executable.is_relative_to(target) and executable.is_file()
                    and os.access(executable, os.X_OK), "test executable escaped owned target")
            matches.append(executable)
    require(len(matches) == 1, "expected exactly one matching real test artifact")
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
    run_root = Path(os.environ["RUNNER_TEMP"]).resolve() / ("storage-core-" + os.environ["GITHUB_RUN_ID"] + "-" + os.environ["GITHUB_RUN_ATTEMPT"])
    require(not run_root.is_symlink(), "runtime symlink refused")
    run_root.mkdir()
    env = clean_env(run_root / "build-environment")
    private_cargo = run_root / "cargo-home"
    private_cargo.mkdir()
    env["CARGO_HOME"] = str(private_cargo)
    root = checkout
    version = run("rust-version", ["rustc", "--version"], 15, root, env).decode().strip()
    require(version.startswith("rustc 1.99.1 "), "Rust version is not pinned 1.99.1")
    run("cargo-version", ["cargo", "--version"], 15, root, env)
    root, files = prepare_source(checkout, run_root, manifest, patch_path, env)
    target = run_root / "target"
    target.mkdir()
    env["CARGO_TARGET_DIR"] = str(target)
    dist = root / "dist"
    require(not dist.exists(), "refuse to replace existing frontend build")
    asset_created = False
    selected = {}
    tracked = {**files, **manifest["preserved_files"]}
    before = {path: file_digest(root / path) if expected is not None else None for path, expected in tracked.items()}
    primary_error = None
    try:
        # Tauri build.rs still executes normally. Empty compile-only embedding
        # asset satisfies frontendDist without pnpm, web builds, or UI claims.
        dist.mkdir()
        asset_created = True
        (dist / "index.html").write_bytes(b"")
        write_json("validation-compile-asset.json", {"path": "dist/index.html", "sha256": digest(b""),
                    "purpose": "empty compile-only asset, NOT frontend/UI validation"})
        groups = [
            ("core", root / "crates/ai-toolbox-storage/Cargo.toml", ["--lib", "--test", "sqlite_jsonb"],
             [("core_unit", "ai_toolbox_storage"), ("core_sqlite", "sqlite_jsonb")], 240),
            ("desktop", root / "tauri/Cargo.toml", ["--test", "sqlite_jsonb", "--test", "storage_reexports"],
             [("desktop_sqlite", "sqlite_jsonb"), ("desktop_reexports", "storage_reexports")], 1050),
        ]
        for group, cargo_manifest, selectors_, suites, seconds in groups:
            compiled = run(group + "-compile", ["cargo", "test", "--locked", "--manifest-path", str(cargo_manifest),
                           *selectors_, "--no-run", "--jobs", "2", "--message-format=json"], seconds, root, env)
            for suite, target_name in suites:
                label = suite.replace("_", "-")
                binary = artifact(compiled, cargo_manifest, target_name, target)
                isolated = run_root / (label + "-environment")
                isolated.mkdir()
                test_env = clean_env(isolated)
                for key in ["CARGO_HOME", "RUSTUP_HOME"]:
                    test_env.pop(key, None)
                names = INVENTORIES[suite]
                listed = run(label + "-list", [str(binary), "--list", "--test-threads=1"], 15, isolated, test_env)
                require(inventory(listed) == names, "exact inventory mismatch: " + suite)
                selected[suite] = {"binary": str(binary), "sha256": file_digest(binary), "tests": names}
                output = run(label + "-tests", [str(binary), "--test-threads=1"], 90, isolated, test_env)
                check_pass(output, len(names), 0)
        require(sum(len(selected[k]["tests"]) for k in ["core_unit", "core_sqlite"]) == 6,
                "exact six shared storage tests required")
    except BaseException as error:
        primary_error = f"{type(error).__name__}: {error}"
        raise
    finally:
        after = {}
        integrity_errors = []
        for path, expected in tracked.items():
            try:
                if expected is None:
                    require(not (root / path).exists() and not (root / path).is_symlink(), "deleted file recreated")
                    after[path] = None
                else:
                    after[path] = file_digest(root / path)
            except BaseException as error:
                after[path] = None
                integrity_errors.append(f"{path}: {type(error).__name__}: {error}")
        # Write the independently known evidence before attempting asset cleanup.
        # These hashes cover candidate files only, not an assertion about the tree.
        write_json("validation-inventory.json", selected)
        write_json("validation-integrity.json", {
            "candidate_files_before": before, "candidate_files_after": after,
            "candidate_files_unchanged": not integrity_errors and before == after,
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
            ("post-unstaged", "unstaged_paths", ["git", "diff", "--no-renames", "--name-only"]),
            ("post-untracked", "untracked_paths", ["git", "ls-files", "--others", "--exclude-standard"]),
            ("post-cached", "cached_paths", ["git", "diff", "--cached", "--no-renames", "--name-only", "HEAD"]),
        ]:
            try:
                final_tree[key] = sorted(run(label, command, 15, root, env).decode().splitlines())
            except BaseException as error:
                final_tree["errors"].append(f"{label}: {type(error).__name__}: {error}")
        write_json("validation-final-git-state.json", final_tree)
        integrity_ok = (not integrity_errors and before == after)
        tree_ok = (not final_tree["errors"] and final_tree["unstaged_paths"] == []
                   and final_tree["untracked_paths"] == [] and final_tree["cached_paths"] == sorted(files))
        write_json("validation-final-checks.json", {
            "primary_error": primary_error, "asset_cleanup_error": cleanup_error,
            "integrity_ok": integrity_ok, "tree_ok": tree_ok,
        })
        if primary_error is None:
            require(cleanup_error is None and integrity_ok and tree_ok,
                    "final integrity/cleanup/tree checks failed; see evidence")



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
    EVIDENCE = Path(os.environ["RUNNER_TEMP"]).resolve() / "storage-core-evidence"
    require(not EVIDENCE.is_symlink(), "evidence directory must not be a symlink")
    EVIDENCE.mkdir(exist_ok=True)
    require(not (EVIDENCE / f"{PHASE}-summary.json").exists(), "phase already executed; no blind retry")
    # Reserve three minutes for evidence upload plus two for checkout
    # before preflight. Enforce remaining wall time within every command, rather
    # than relying on the 45-minute Actions hard kill to run Python finally.
    clock_path = EVIDENCE / "preflight-budget.json"
    if PHASE == "preflight":
        require(not clock_path.exists(), "budget already initialized")
        write_json(clock_path.name, {"deadline_unix": time.time() + 37 * 60})
    JOB_DEADLINE = json.loads(clock_path.read_text())["deadline_unix"]
    if PHASE == "validation":
        JOB_DEADLINE = min(JOB_DEADLINE, time.time() + 24 * 60)
    summary = {"phase": PHASE, "ok": False, "error": None}
    for sig in [signal.SIGTERM, signal.SIGINT]:
        signal.signal(sig, interrupted)
    try:
        manifest_path = (root / options.manifest).resolve(strict=True)
        require(manifest_path == root / "scripts/storage-core-validation/candidate.json",
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
