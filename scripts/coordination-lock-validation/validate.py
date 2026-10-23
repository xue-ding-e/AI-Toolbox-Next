#!/usr/bin/env python3
"""Ubuntu-only bounded lock resolution for issue #41 coordination hooks; no build/test.

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
import tempfile
import difflib
import stat

BASE = '8a15ef04c88bf4404c482faf7166f6694985e401'
REPO = "xue-ding-e/AI-Toolbox-Next"
MANIFEST_SHA256 = 'a82bc3ec3ec0a7b7a191bc026a3bed3acf6b43403e56c3c91aa4cb418a4a4a05'
CANDIDATE = None
PATCH_SHA256 = 'cf51bb9d40f209bbc120fa5426299c56eb29fd7d54221eaf92c1ce195b66921e'
BRANCH = "refs/heads/validation/coordination-artifact-lock"
CONTROL_PATHS = {
    ".github/workflows/maintenance-validation.yml",
    "scripts/coordination-lock-validation/validate.py",
    "scripts/coordination-lock-validation/candidate.json",
    "scripts/coordination-lock-validation/candidate.patch",
}
INVENTORIES = {'desktop_selected': ['coding::codex::history_sync::tests::forged_sidecar_preserves_database_index_lock_inode_and_backup_set', 'coding::codex::history_sync::tests::restore_latest_restores_database_and_session_metadata', 'coding::ssh::session::tests::matches_excluded_directory_by_name_segment_only', 'coding::ssh::session::tests::reports_directory_upload_paths_relative_to_mapping_root', 'coding::ssh::sync::tests::pattern_file_display_path_preserves_nested_relative_path', 'coding::ssh::sync::tests::pattern_file_display_path_uses_static_glob_root', 'coding::ssh::sync::tests::remote_replace_and_remove_preserve_lease_inodes_in_local_shell_fixture', 'coding::ssh::sync::tests::remote_tree_checks_precede_staging_and_destructive_swap', 'coding::ssh::sync::tests::shell_fixture::proc_stat_parser_distinguishes_live_members_from_zombies', 'coding::wsl::sync::tests::directory_copy_command_uses_temp_dir_before_replacing_target', 'coding::wsl::sync::tests::directory_copy_command_with_excludes_builds_tar_exclude_args', 'coding::wsl::sync::tests::directory_replace_preserves_parent_lease_and_refuses_non_directory_targets', 'coding::wsl::sync::tests::directory_shell_fixture_excludes_all_reserved_source_types_and_preserves_controls', 'coding::wsl::sync::tests::directory_shell_fixture_rejects_target_artifacts_before_staging', 'coding::wsl::sync::tests::normalize_directory_target_path_trims_trailing_slashes', 'coding::wsl::sync::tests::shell_fixture::proc_stat_parser_distinguishes_live_members_from_zombies', 'settings::backup::utils::tests::coordination_source_pruning_keeps_ordinary_dotfiles_and_near_matches', 'settings::backup::utils::tests::custom_directory_entry_excludes_matching_files_and_prunes_directories', 'settings::backup::utils::tests::custom_directory_entry_restores_without_deleting_extra_files', 'settings::backup::utils::tests::custom_file_entry_is_written_with_manifest', 'settings::backup::utils::tests::duplicate_normalized_manifests_block_synthetic_database_sink', 'settings::backup::utils::tests::external_config_restore_path_rejects_existing_symlink_components', 'settings::backup::utils::tests::external_config_restore_path_rejects_traversal', 'settings::backup::utils::tests::forged_archive_and_custom_target_are_rejected_before_any_write', 'settings::backup::utils::tests::legacy_database_snapshot_allows_missing_directory', 'settings::backup::utils::tests::legacy_database_snapshot_skips_system_files', 'settings::backup::utils::tests::ordinary_binary_root_dir_payload_restores_without_metadata_interpretation']}
CORE_TESTS = []
LOG_LIMIT = 16 * 1024 * 1024  # Per command, both streams; exceedance fails closed.
TOTAL_LOG_LIMIT = 64 * 1024 * 1024
RECORDS = []
TOTAL_LOG_BYTES = 0
ACTIVE = None
EVIDENCE = None
PHASE = "unknown"
JOB_DEADLINE = None
CACHE_ROOT = None
CACHE_CHECK_AT = 0
CACHE_OBSERVATION = {}


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
    # that outer boundary. Selected shell fixtures spawn bounded owned shell groups; the tested helper waits/reaps them. No fixture intentionally daemonizes, but this monitor is not hostile process containment.
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


def check_public_cache():
    global CACHE_CHECK_AT, CACHE_OBSERVATION
    if CACHE_ROOT is None or time.monotonic() < CACHE_CHECK_AT:
        return
    CACHE_CHECK_AT = time.monotonic() + 1
    size = 0
    count = 0
    for path in CACHE_ROOT.rglob("*"):
        try:
            info = path.lstat()
        except FileNotFoundError:
            continue
        require(stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode), "unexpected special cache object")
        require(not path.is_relative_to(CACHE_ROOT / "registry/cache") and not path.is_relative_to(CACHE_ROOT / "registry/src"), "source archive download forbidden")
        count += 1
        size += info.st_size if stat.S_ISREG(info.st_mode) else 0
        require(size <= 160 * 1024 * 1024 and count <= 30000, "public index budget exceeded")
    CACHE_OBSERVATION = {"bytes": size, "entries": count, "source_archives_allowed": False}


def lock_records(raw):
    text = raw.decode("utf-8")
    pieces = text.split("[[package]]\n")
    result = {}
    for block in pieces[1:]:
        def field(name, required=True):
            values = re.findall(r"^" + name + r" = (\".*\")$", block, re.M)
            require(len(values) == 1 or (not required and len(values) == 0), "unexpected lock field")
            return json.loads(values[0]) if values else None
        key = (field("name"), field("version"), field("source", False))
        require(key not in result, "duplicate lock package")
        if key[2]:
            require(key[2] == "registry+https://github.com/rust-lang/crates.io-index", "nonofficial registry/git source")
            require(re.fullmatch(r"[0-9a-f]{64}", field("checksum")), "invalid checksum")
        result[key] = block
    require(result, "empty lock package inventory")
    return result


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
            check_public_cache()
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
            "manifest differs from exact reviewed coordination-hooks manifest; no scope expansion")
    require(manifest.get("source_base") == BASE, "fixed baseline mismatch")
    require(manifest.get("candidate_commit") == CANDIDATE, "candidate provenance mismatch")
    require(manifest.get("patch_sha256") == PATCH_SHA256, "reviewed patch mismatch")
    require(sorted(manifest.get("core_tests", [])) == CORE_TESTS, "core inventory changed")
    require(manifest.get("inventories") == INVENTORIES, "exact inventories changed")
    require(len(manifest["candidate_files"]) == 12 and len(manifest["preserved_files"]) == 9, "manifest scope changed")
    env = clean_env(EVIDENCE / (PHASE + "-git-home"))
    git = lambda label, args: run(label, ["git", *args], 15, root, env)
    head = git("head", ["rev-parse", "HEAD"]).decode().strip()
    require(head == sha, "checkout does not match github.sha")
    parents = git("single-parent", ["rev-list", "--parents", "-n", "1", "HEAD"]).decode().split()
    require(parents == [head, BASE], "validation must be a single-parent harness on exact base")
    require(not git("clean-checkout", ["status", "--porcelain", "--untracked-files=all"]).strip(), "checkout is dirty")
    changed = set(git("changed-paths", ["diff", "--no-renames", "--name-only", BASE, "HEAD"]).decode().splitlines())
    require(changed == CONTROL_PATHS, "validation branch must change exactly the four harness files")
    patch_path = root / "scripts/coordination-lock-validation/candidate.patch"
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
    require(set(changed.decode().splitlines()) == set(files), "patched source paths differ from reviewed twelve")
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
    global CACHE_ROOT, CACHE_CHECK_AT
    patch_path = source_check(checkout, manifest)
    run_root = Path(os.environ["RUNNER_TEMP"]).resolve() / ("coordination-lock-" + os.environ["GITHUB_RUN_ID"])
    run_root.mkdir()
    env = clean_env(run_root / "environment")
    cargo_home = run_root / "cargo-home"
    cargo_home.mkdir(mode=0o700)
    env["CARGO_HOME"] = str(cargo_home)
    env["CARGO_HTTP_TIMEOUT"] = "20"
    env["CARGO_NET_RETRY"] = "0"
    env["CARGO_REGISTRIES_CRATES_IO_PROTOCOL"] = "sparse"
    CACHE_ROOT = cargo_home
    root, files = prepare_source(checkout, run_root, manifest, patch_path, env)
    version = run("rust-version", ["rustc", "--version"], 10, root, env).decode().strip()
    require(version.startswith("rustc 1.99.1 "), "wrong Rust")
    cargo_manifest = root / "tauri/Cargo.toml"
    lock = root / "tauri/Cargo.lock"
    before = lock.read_bytes()
    before_records = lock_records(before)
    (EVIDENCE / "validation-before.lock").write_bytes(before)
    error_text = None
    try:
        # --no-deps is informational only; it does not resolve/validate Cargo.lock.
        run("manifest-metadata", ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version=1", "--manifest-path", str(cargo_manifest)], 10, root, env)
        run("resolve-local-package", ["cargo", "update", "--manifest-path", str(cargo_manifest), "-p", "ai-toolbox-management"], 65, root, env)
        after = lock.read_bytes()
        after_records = lock_records(after)
        (EVIDENCE / "validation-after.lock").write_bytes(after)
        diff = list(difflib.unified_diff(before.decode().splitlines(True), after.decode().splitlines(True), fromfile="candidate/Cargo.lock", tofile="cargo-generated/Cargo.lock"))
        (EVIDENCE / "validation-lock.diff").write_text("".join(diff))
        require(set(before_records) == set(after_records), "package identity/version/source inventory drift")
        changed = [key for key in before_records if before_records[key] != after_records[key]]
        registry_same = all(before_records[key] == after_records[key] for key in before_records if key[2])
        write_json("validation-package-diff.json", {"changed_records": changed, "registry_records_exact": registry_same,
                   "registry_count": sum(bool(key[2]) for key in before_records), "before_sha256": digest(before), "after_sha256": digest(after)})
        require(registry_same, "registry record drift is not approved")
        require(changed == [("ai-toolbox-management", "0.1.0", None)], "only the management path dependency closure may change")
        path_key = ("ai-toolbox-management", "0.1.0", None)
        expected = before.decode().replace(before_records[path_key], after_records[path_key], 1)
        require(expected.encode() == after, "lock header/order/nonmanagement bytes changed")
        run("locked-offline-resolution", ["cargo", "update", "--locked", "--offline", "--manifest-path", str(cargo_manifest), "-p", "ai-toolbox-management"], 15, root, env)
        require(lock.read_bytes() == after, "locked offline resolution changed lock bytes")
        require(all(file_digest(root / path) == expected for path, expected in {**files, **manifest["preserved_files"]}.items() if path != "tauri/Cargo.lock"), "source outside lock changed")
        status = run("final-unstaged", ["git", "diff", "--name-only"], 10, root, env).decode().splitlines()
        require(status == ["tauri/Cargo.lock"], "unexpected source change")
        require(not run("final-untracked", ["git", "ls-files", "--others", "--exclude-standard"], 10, root, env).strip(), "unexpected untracked source")
    except BaseException as error:
        error_text = str(error)
        raise
    finally:
        CACHE_CHECK_AT = 0
        try:
            check_public_cache()
        finally:
            write_json("validation-lock-result.json", {"error": error_text, "cache": CACHE_OBSERVATION,
                       "compiled": False, "tests_run": 0, "no_deps_is_lock_proof": False,
                       "current_lock_sha256": file_digest(lock), "all_recorded_survivors_empty": all(not record["survivors"] for record in RECORDS)})


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
    EVIDENCE = Path(os.environ["RUNNER_TEMP"]).resolve() / "coordination-lock-evidence"
    require(not EVIDENCE.is_symlink(), "evidence directory must not be a symlink")
    EVIDENCE.mkdir(exist_ok=True)
    require(not (EVIDENCE / f"{PHASE}-summary.json").exists(), "phase already executed; no blind retry")
    # This one invocation gets 110 seconds including cleanup, within a two-minute
    # step. Checkout/toolchain/upload each have separate one-minute ceilings;
    # the outer job limit is five minutes. No build or product test is invoked.
    clock_path = EVIDENCE / "preflight-budget.json"
    if PHASE in ["preflight", "validation"]:
        require(not clock_path.exists(), "budget already initialized")
        write_json(clock_path.name, {"deadline_unix": time.time() + 110})
    JOB_DEADLINE = json.loads(clock_path.read_text())["deadline_unix"]
    if PHASE == "validation":
        JOB_DEADLINE = min(JOB_DEADLINE, time.time() + 110)
    summary = {"phase": PHASE, "ok": False, "error": None}
    for sig in [signal.SIGTERM, signal.SIGINT]:
        signal.signal(sig, interrupted)
    try:
        manifest_path = (root / options.manifest).resolve(strict=True)
        require(manifest_path == root / "scripts/coordination-lock-validation/candidate.json",
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
