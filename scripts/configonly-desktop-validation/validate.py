#!/usr/bin/env python3
"""Draft, Ubuntu-only bounded desktop compatibility gate for issue #41.

Sealed input pending parent harness review; do not dispatch without admission.
Only the reviewed patch is applied to an ephemeral fixed-base worktree. This
compiles the actual desktop library and runs four old pure-memory controls;
it does not rerun core tests, launch the GUI, or prove ConfigOnly admission.
Every external command uses the previously reviewed owned-process supervisor.
Apt setup has its separate root timeout/job boundary, not pidfd proof here.
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

BASE = "044e45d373fc078c3affa1628b5cb4286bb36d77"
BASE_TREE = "f4d64278fbbef295971acf6fea69f0da8c2beecd"
REPO = "xue-ding-e/AI-Toolbox-Next"
BRANCH = "refs/heads/validation/codex-configonly-desktop"
MANIFEST_SHA256 = "a0cfa99914e59ea3a2fbe54fd1b8881269f40ba6c4d7e89e20b1446527ed3df6"
PATCH_SHA256 = "60b43deccade14346d6ce0ac8b7f88414d812d781cba13a118ecc6164575234a"
CORE_TESTED_PATCH_SHA256 = "3d5d3343a19359621752fa619826799fca5e90a63354b2337d943a37a1a4e994"
CONTROL_PATHS = {
    ".github/workflows/maintenance-validation.yml",
    "scripts/configonly-desktop-validation/validate.py",
    "scripts/configonly-desktop-validation/candidate.json",
    "scripts/configonly-desktop-validation/candidate.patch",
}
CANDIDATE_PATHS = {
    "crates/ai-toolbox-management/AGENTS.md",
    "crates/ai-toolbox-management/Cargo.lock",
    "crates/ai-toolbox-management/Cargo.toml",
    "crates/ai-toolbox-management/src/lib.rs",
    "crates/ai-toolbox-management/src/owner.rs",
    "crates/ai-toolbox-management/src/codex_master.rs",
    "crates/ai-toolbox-management/src/codex_service.rs",
    "crates/ai-toolbox-management/src/config_file.rs",
    "tauri/Cargo.lock",
}
NEW_PATHS = {
    "crates/ai-toolbox-management/src/codex_master.rs",
    "crates/ai-toolbox-management/src/codex_service.rs",
    "crates/ai-toolbox-management/src/config_file.rs",
}
SELECTED_TESTS = sorted([
    "coding::codex::commands::tests::append_toml_configs_keeps_common_root_keys_at_root",
    "coding::codex::commands::tests::append_toml_configs_merges_common_tables_without_overwriting_provider_table",
    "coding::codex::commands::tests::append_toml_configs_keeps_explicit_provider_scalar_overrides",
    "coding::codex::commands::tests::codex_model_catalog_dedupes_identical_rows_keeps_distinct_display_names",
])
LOG_LIMIT = 16 * 1024 * 1024
TOTAL_LOG_LIMIT = 64 * 1024 * 1024
RECORDS = []
TOTAL_LOG_BYTES = 0
ACTIVE = None
EVIDENCE = None
PHASE = "unknown"
JOB_DEADLINE = None
FINALIZING = False
FINALIZE_RESERVE = 60


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
    # that outer boundary. The selected controls are pure-memory tests and intentionally spawn no children; this monitor is not hostile process containment.
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
        timeout = min(timeout, JOB_DEADLINE - time.monotonic() - (10 if FINALIZING else FINALIZE_RESERVE))
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


def clean_env(root):
    # Fresh runner-owned HOME/profile/cache/temp roots; never inherit tokens,
    # proxy credentials, DISPLAY, cargo config, or arbitrary compiler flags.
    root.mkdir(mode=0o700)
    env = {"PATH": os.environ["PATH"], "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8",
           "RUSTUP_TOOLCHAIN": "1.99.1", "CARGO_BUILD_JOBS": "2",
           "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0",
           "CARGO_INCREMENTAL": "0", "RUST_BACKTRACE": "0",
           "CARGO_TERM_COLOR": "never", "CARGO_HTTP_TIMEOUT": "30",
           "CARGO_NET_RETRY": "1", "GIT_CONFIG_NOSYSTEM": "1",
           "GIT_CONFIG_GLOBAL": "/dev/null"}
    # Only the official toolchain installed in this job is shared with cargo.
    env["RUSTUP_HOME"] = os.environ.get("RUSTUP_HOME", str(Path(os.environ["HOME"]) / ".rustup"))
    for key in ["HOME", "USERPROFILE", "CARGO_HOME", "CARGO_TARGET_DIR",
                "XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME",
                "AI_TOOLBOX_DATA_DIR", "AI_TOOLBOX_CACHE_DIR", "CODEX_HOME",
                "CLAUDE_CONFIG_DIR", "TMPDIR", "TMP", "TEMP"]:
        directory = root / key
        directory.mkdir(mode=0o700)
        env[key] = str(directory)
    return env


def source_check(root, manifest, manifest_path):
    """Refuse any unsealed draft before toolchain, apt, build, or test setup."""
    require(re.fullmatch(r"[0-9a-f]{64}", MANIFEST_SHA256 or ""), "unsealed manifest draft")
    require(re.fullmatch(r"[0-9a-f]{64}", PATCH_SHA256 or ""), "unsealed patch draft")
    require(file_digest(manifest_path) == MANIFEST_SHA256, "manifest bytes changed")
    require(manifest.get("state") == "sealed-input-pending-dispatch-review", "candidate not sealed")
    require(os.environ.get("GITHUB_REPOSITORY") == REPO, "repository guard failed")
    require(os.environ.get("GITHUB_REF") == BRANCH, "branch guard failed")
    require(os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch", "manual-only guard failed")
    sha = os.environ.get("GITHUB_SHA", "")
    require(re.fullmatch(r"[0-9a-f]{40}", sha), "missing validation SHA")
    require(os.environ.get("EXPECTED_SHA") == sha, "dispatch SHA differs")
    require(manifest.get("source_base") == BASE and manifest.get("base_tree") == BASE_TREE,
            "fixed source base differs")
    require(manifest.get("patch_sha256") == PATCH_SHA256, "reviewed patch mismatch")
    require(manifest.get("core_tested_patch_sha256") == CORE_TESTED_PATCH_SHA256,
            "core-tested provenance differs")
    require(manifest.get("desktop_selected") == SELECTED_TESTS
            and manifest.get("core_tests") == [], "test scope differs")
    require(set(manifest.get("candidate_files", {})) == CANDIDATE_PATHS, "candidate scope differs")
    require(manifest.get("candidate_statuses") == {
        path: "A" if path in NEW_PATHS else "M" for path in CANDIDATE_PATHS}, "candidate statuses differ")
    for path, expected in {**manifest["candidate_files"], **manifest["preserved_files"]}.items():
        require(not Path(path).is_absolute() and ".." not in Path(path).parts, "unsafe manifest path")
        require(re.fullmatch(r"[0-9a-f]{64}", expected or ""), "unsealed file hash")
    env = clean_env(EVIDENCE / (PHASE + "-git-environment"))
    git = lambda label, args: run(label, ["git", *args], 15, root, env)
    head = git("head", ["rev-parse", "HEAD"]).decode().strip()
    require(head == sha, "checkout differs from github.sha")
    parents = git("single-parent", ["rev-list", "--parents", "-n", "1", "HEAD"]).decode().split()
    require(parents == [head, BASE], "harness must have exactly the fixed base as parent")
    require(not git("clean-checkout", ["status", "--porcelain", "--untracked-files=all"]).strip(), "dirty checkout")
    changed = set(git("changed-paths", ["diff", "--no-renames", "--name-only", BASE, "HEAD"]).decode().splitlines())
    require(changed == CONTROL_PATHS, "harness must change exactly the registered four paths")
    require(git("base-tree", ["rev-parse", f"{BASE}^{{tree}}"]).decode().strip() == BASE_TREE,
            "base tree differs")
    patch = root / "scripts/configonly-desktop-validation/candidate.patch"
    require(not patch.is_symlink() and file_digest(patch) == PATCH_SHA256, "stored patch differs")
    write_json(f"{PHASE}-source.json", {
        "base": BASE, "base_tree": BASE_TREE, "validation_commit": head,
        "patch_sha256": PATCH_SHA256, "core_tested_patch_sha256": CORE_TESTED_PATCH_SHA256,
        "manifest_sha256": MANIFEST_SHA256, "harness_sha256": file_digest(Path(__file__)),
        "workflow_sha256": file_digest(root / ".github/workflows/maintenance-validation.yml"),
        "candidate_files": manifest["candidate_files"], "selected_tests": SELECTED_TESTS,
    })
    return patch


def verify_hashes(root, files):
    actual = {}
    for path, expected in files.items():
        file = root / path
        require(not file.is_symlink() and file.resolve(strict=True).is_relative_to(root),
                "source file escaped owned worktree")
        actual[path] = file_digest(file)
        require(actual[path] == expected, "source file hash differs: " + path)
    return actual


def snapshot_source(root, env, label):
    """Hash every tracked source/build input, not just nine changed files."""
    entries = run(label + "-paths", ["git", "ls-files", "--stage", "-z"], 30, root, env)
    files = {}
    for entry in entries.split(b"\0"):
        if not entry:
            continue
        meta, raw = entry.split(b"\t", 1)
        mode, _oid, stage = meta.decode().split()
        require(stage == "0", "unmerged source entry")
        path = os.fsdecode(raw)
        file = root / path
        require(mode in {"100644", "100755", "120000"}, "unsupported source entry")
        if mode == "120000":
            require(file.is_symlink(), "tracked symlink type changed")
            value = digest(os.fsencode(os.readlink(file)))
        else:
            require(not file.is_symlink() and file.is_file(), "tracked source type changed")
            value = file_digest(file)
        files[path] = {"mode": mode, "sha256": value}
    require(files, "empty source inventory")
    return files


def audit_minimal_lock(before, after):
    """Only four existing normal edges in the local management package may change."""
    marker = "[[package]]"
    old, new = before.split(marker), after.split(marker)
    require(len(old) == len(new) and old[0] == new[0], "lock package count/header changed")
    changes = []
    registry_count = 0
    for old_block, new_block in zip(old[1:], new[1:]):
        if 'source = "registry+' in old_block:
            registry_count += 1
            require(old_block == new_block, "pre-existing registry package changed")
        if old_block != new_block:
            changes.append((old_block, new_block))
    require(len(changes) == 1 and registry_count == 865, "unexpected lock change scope")
    old_block, new_block = changes[0]
    require(re.search(r'^name = "ai-toolbox-management"$', old_block, re.M), "changed non-management lock block")
    added = [' "ai-toolbox-codex-config",\n', ' "ai-toolbox-storage",\n',
             ' "rusqlite",\n', ' "sha2 0.10.9",\n']
    stripped = new_block
    for line in added:
        require(line not in old_block and stripped.count(line) == 1, "normal edge is not a unique addition")
        stripped = stripped.replace(line, "", 1)
    require(stripped == old_block, "lock differs beyond four reviewed normal edges")
    return {"unchanged_registry_blocks": registry_count,
            "added_normal_edges": [line.strip().strip(',').strip('"') for line in added],
            "before_sha256": digest(before.encode()), "after_sha256": digest(after.encode())}


def prepare_source(checkout, run_root, manifest, patch, env):
    # Local staging is confined to a new detached runner-owned worktree. It is
    # for canonical diff/tree evidence only, never a product or branch commit.
    root = run_root / "source"
    run("create-source-worktree", ["git", "worktree", "add", "--detach", str(root), BASE], 60, checkout, env)
    run("apply-reviewed-patch", ["git", "apply", "--index", "--whitespace=error", "--", str(patch)], 30, root, env)
    verify_hashes(root, {**manifest["candidate_files"], **manifest["preserved_files"]})
    changed = run("patched-status", ["git", "diff", "--cached", "--name-status", "--no-renames", BASE], 15, root, env)
    require(dict((line.split("\t")[1], line.split("\t")[0]) for line in changed.decode().splitlines())
            == manifest["candidate_statuses"], "patched status scope differs")
    canonical = run("canonical-patch", ["git", "diff", "--cached", "--binary", "--full-index", "--no-ext-diff", "--no-renames", BASE], 30, root, env)
    require(digest(canonical) == PATCH_SHA256, "applied canonical patch differs")
    require(not run("initial-unstaged", ["git", "diff", "--name-only"], 15, root, env).strip(), "unstaged source")
    require(not run("initial-untracked", ["git", "ls-files", "--others", "--exclude-standard"], 15, root, env).strip(), "untracked source")
    baseline_lock = run("base-lock", ["git", "show", f"{BASE}:tauri/Cargo.lock"], 15, root, env).decode()
    lock_audit = audit_minimal_lock(baseline_lock, (root / "tauri/Cargo.lock").read_text())
    source = (root / "tauri/src/coding/codex/commands.rs").read_text()
    for name in SELECTED_TESTS:
        require(len(re.findall(r"#\[test\]\s*fn\s+" + re.escape(name.rsplit("::", 1)[1]) + r"\s*\(", source)) == 1,
                "selected old source test absent, ignored, or duplicated")
    tree = run("input-tree", ["git", "write-tree"], 15, root, env).decode().strip()
    require(tree == manifest["candidate_tree"], "complete candidate tree differs")
    write_json("validation-patched-source.json", {"tree": tree, "patch_sha256": PATCH_SHA256,
                "candidate_files": manifest["candidate_files"], "lock_audit": lock_audit})
    return root, tree


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


def cleanup_empty_environment(root, env):
    """Remove only the exact empty directories created for a pure-memory case."""
    require(ACTIVE is None and all(not record["survivors"] for record in RECORDS),
            "process exit not confirmed; preserve test environment")
    owned = {Path(value) for key, value in env.items() if key not in {"PATH", "RUSTUP_HOME"}
             and isinstance(value, str) and value.startswith(str(root) + "/")}
    require(set(root.iterdir()) == owned, "unexpected test environment entries; preserve")
    for path in owned:
        require(not path.is_symlink() and path.is_dir() and not any(path.iterdir()),
                "pure-memory control wrote files; preserve environment")
    for path in sorted(owned):
        path.rmdir()
    root.rmdir()


def validate(checkout, manifest, manifest_path):
    global FINALIZING
    patch = source_check(checkout, manifest, manifest_path)
    run_root = Path(os.environ["RUNNER_TEMP"]).resolve() / (
        "configonly-desktop-" + os.environ["GITHUB_RUN_ID"] + "-" + os.environ["GITHUB_RUN_ATTEMPT"])
    require(not run_root.exists() and not run_root.is_symlink(), "runtime root already exists")
    run_root.mkdir(mode=0o700)
    env = clean_env(run_root / "build-environment")
    version = run("rust-version", ["rustc", "--version"], 15, checkout, env).decode().strip()
    require(version.startswith("rustc 1.99.1 "), "Rust version is not pinned 1.99.1")
    cargo_version = run("cargo-version", ["cargo", "--version"], 15, checkout, env).decode().strip()
    require(cargo_version.startswith("cargo 1.99.1 "), "Cargo version is not pinned 1.99.1")
    root, input_tree = prepare_source(checkout, run_root, manifest, patch, env)
    before = snapshot_source(root, env, "input-source")
    write_json("validation-input-files.json", before)
    target = Path(env["CARGO_TARGET_DIR"])
    dist = root / "dist"
    require(not dist.exists() and not dist.is_symlink(), "refuse existing frontend build")
    primary_error = None
    asset_identity = None
    dist_identity = None
    selected = {}
    try:
        # Real Tauri build.rs runs normally. This empty compile-only embedding
        # asset avoids pnpm/frontend work; it is NOT a frontend or GUI proof.
        dist.mkdir(mode=0o700)
        dist_stat = dist.stat()
        dist_identity = (dist_stat.st_dev, dist_stat.st_ino)
        asset = dist / "index.html"
        with asset.open("xb"):
            pass
        asset_stat = asset.stat()
        asset_identity = (asset_stat.st_dev, asset_stat.st_ino)
        write_json("validation-compile-asset.json", {"path": "dist/index.html", "sha256": digest(b""),
                    "purpose": "empty compile-only asset, NOT frontend/UI validation"})
        cargo_manifest = root / "tauri/Cargo.toml"
        # Locked failure is terminal. No update, unlocked retry, or registry
        # substitution is available anywhere in this harness.
        compiled = run("desktop-compile", ["cargo", "test", "--locked", "--manifest-path", str(cargo_manifest),
                       "--lib", "--no-run", "--jobs", "2", "--message-format=json"], 1050, root, env)
        require(any(json.loads(line).get("reason") == "build-finished"
                    and json.loads(line).get("success") is True
                    for line in compiled.decode().splitlines()), "missing successful build-finished")
        binary = artifact(compiled, cargo_manifest, "ai_toolbox_lib", target)
        list_root = run_root / "list-environment"
        list_env = clean_env(list_root)
        list_env["CARGO_NET_OFFLINE"] = "true"
        listed = run("desktop-list", [str(binary), "--list", "--test-threads=1"], 30, list_root, list_env)
        all_names = inventory(listed)
        require(set(SELECTED_TESTS).issubset(all_names), "selected old tests absent from desktop binary")
        cleanup_empty_environment(list_root, list_env)
        selected = {"binary": str(binary), "sha256": file_digest(binary),
                    "selected": SELECTED_TESTS, "whole_binary_inventory": all_names,
                    "whole_binary_inventory_count": len(all_names), "passed": []}
        write_json("validation-inventory.json", selected)
        for index, name in enumerate(SELECTED_TESTS):
            label = f"desktop-case-{index+1:02d}"
            isolated = run_root / (label + "-environment")
            test_env = clean_env(isolated)
            test_env["CARGO_NET_OFFLINE"] = "true"
            case_error = None
            cleanup_error = None
            try:
                actual = run(label + "-list", [str(binary), "--exact", name, "--list"], 15, isolated, test_env)
                require(inventory(actual) == [name], "exact case inventory differs")
                output = run(label + "-test", [str(binary), "--exact", name, "--test-threads=1", "--show-output"],
                             30, isolated, test_env)
                check_pass(output, 1, len(all_names)-1)
                selected["passed"].append(name)
            except BaseException as error:
                case_error = error
                raise
            finally:
                try:
                    cleanup_empty_environment(isolated, test_env)
                except BaseException as error:
                    cleanup_error = f"{type(error).__name__}: {error}"
                write_json("validation-" + label + "-cleanup.json", {
                    "environment_exists": isolated.exists(), "error": cleanup_error,
                    "policy": "only empty exact owned directories; unexpected contents retained"})
                if case_error is None:
                    require(cleanup_error is None, "pure-memory case environment cleanup failed")
        require(len(selected["passed"]) == 4, "four old desktop controls required")
    except BaseException as error:
        primary_error = f"{type(error).__name__}: {error}"
        raise
    finally:
        FINALIZING = True
        integrity_errors = []
        after = None
        cleanup_error = None
        final_tree = None
        final_patch_sha256 = None
        final_unstaged = None
        final_untracked = None
        write_json("validation-inventory.json", selected)
        try:
            after = snapshot_source(root, env, "final-source")
            write_json("validation-final-files.json", after)
        except BaseException as error:
            integrity_errors.append(f"source snapshot: {type(error).__name__}: {error}")
        try:
            # Do not recurse or erase an unknown replacement. The ephemeral
            # runner is the outer boundary, not hostile same-UID containment.
            require(ACTIVE is None and all(not record["survivors"] for record in RECORDS),
                    "process exit not confirmed; preserve compile asset")
            if dist_identity is not None:
                observed = dist.lstat()
                require(not dist.is_symlink() and (observed.st_dev, observed.st_ino) == dist_identity,
                        "compile asset directory replaced; preserve")
                if asset_identity is not None:
                    asset = dist / "index.html"
                    observed = asset.lstat()
                    require(not asset.is_symlink() and (observed.st_dev, observed.st_ino) == asset_identity
                            and asset.is_file() and asset.read_bytes() == b"", "compile asset changed; preserve")
                    asset.unlink()
                dist.rmdir()
        except BaseException as error:
            cleanup_error = f"{type(error).__name__}: {error}"
        write_json("validation-asset-cleanup.json", {"asset_was_created": asset_identity is not None,
                    "dist_exists_after_cleanup": dist.exists(), "error": cleanup_error})
        try:
            final_tree = run("final-tree", ["git", "write-tree"], 15, root, env).decode().strip()
            final_patch_sha256 = digest(run("final-patch", ["git", "diff", "--cached", "--binary", "--full-index",
                                              "--no-ext-diff", "--no-renames", BASE], 30, root, env))
            final_unstaged = run("final-unstaged", ["git", "diff", "--no-renames", "--name-only"], 15, root, env).decode().splitlines()
            final_untracked = run("final-untracked", ["git", "ls-files", "--others", "--exclude-standard"], 15, root, env).decode().splitlines()
        except BaseException as error:
            integrity_errors.append(f"final git state: {type(error).__name__}: {error}")
        integrity_ok = not integrity_errors and before == after
        tree_ok = (final_tree == input_tree and final_patch_sha256 == PATCH_SHA256
                   and final_unstaged == [] and final_untracked == [])
        lock_paths = [path for path in before if path.endswith("Cargo.lock")]
        locks_unchanged = after is not None and all(before[path] == after.get(path) for path in lock_paths)
        write_json("validation-final-checks.json", {
            "primary_error": primary_error, "integrity_errors": integrity_errors,
            "complete_source_unchanged": integrity_ok, "source_file_count": len(before),
            "lock_hashes_before": {path: before[path] for path in lock_paths},
            "lock_hashes_after": {path: after.get(path) for path in lock_paths} if after is not None else None,
            "all_locks_unchanged": locks_unchanged, "input_tree": input_tree, "final_tree": final_tree,
            "final_patch_sha256": final_patch_sha256, "final_unstaged": final_unstaged,
            "final_untracked": final_untracked, "tree_ok": tree_ok,
            "asset_cleanup_error": cleanup_error,
        })
        if primary_error is None:
            require(integrity_ok and tree_ok and locks_unchanged and cleanup_error is None,
                    "final source/lock/cleanup checks failed; see evidence")


def main():
    global EVIDENCE, PHASE, JOB_DEADLINE
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=["preflight", "validation"], required=True)
    parser.add_argument("--manifest", required=True)
    options = parser.parse_args()
    PHASE = options.phase
    require(sys.platform == "linux", "Ubuntu runner required")
    require(os.geteuid() != 0, "preflight/build/test must remain unprivileged")
    root = Path(os.environ["GITHUB_WORKSPACE"]).resolve()
    EVIDENCE = Path(os.environ["RUNNER_TEMP"]).resolve() / "configonly-desktop-evidence"
    require(not EVIDENCE.is_symlink(), "evidence symlink refused")
    EVIDENCE.mkdir(mode=0o700, exist_ok=True)
    require(not (EVIDENCE / f"{PHASE}-summary.json").exists(), "phase already ran; no blind retry")
    # Same runner monotonic clock is shared by both phases. Setup consumes the
    # original 37-minute allowance; validation never restarts that allowance.
    clock_path = EVIDENCE / "preflight-budget.json"
    if PHASE == "preflight":
        require(not clock_path.exists(), "budget already initialized")
        write_json(clock_path.name, {"deadline_monotonic": time.monotonic() + 37 * 60,
                                     "deadline_unix_advisory": time.time() + 37 * 60})
    JOB_DEADLINE = json.loads(clock_path.read_text())["deadline_monotonic"]
    if PHASE == "validation":
        JOB_DEADLINE = min(JOB_DEADLINE, time.monotonic() + 24 * 60)
    summary = {"phase": PHASE, "ok": False, "error": None,
               "core_tests_rerun": 0, "selected_old_desktop_controls": 4,
               "scope": "desktop compilation plus old pure-memory controls; not GUI/service admission"}
    for sig in [signal.SIGTERM, signal.SIGINT]:
        signal.signal(sig, interrupted)
    try:
        manifest_path = (root / options.manifest).resolve(strict=True)
        require(manifest_path == root / "scripts/configonly-desktop-validation/candidate.json",
                "manifest must be the fixed harness path")
        manifest = json.loads(manifest_path.read_text())
        if PHASE == "validation":
            validate(root, manifest, manifest_path)
        else:
            source_check(root, manifest, manifest_path)
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
