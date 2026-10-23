#!/usr/bin/env python3
"""Prepared bounded typed-owner / CLI integration gate. Not dispatch authorization.

Only a fixed 13-file candidate is applied to a detached runner-owned worktree.
Only three new test modules may be formatted, and only the automation lock may
be resolved. Actual locked core and desktop test builds validate the resulting
bytes. Source, dependencies, compiler outputs and evidence stay on runner disk;
only synthetic runtime fixtures use an existing private leaf under /dev/shm.
No GUI, real profile, credential, remote service, main/release mutation or broad
suite run is permitted. Source/lock changes require review before product use.
"""
import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time

BASE = '21130fe41b88e2cb1374de155e27f45fe58707c3'
BASE_TREE = 'a3c2b1f9506594f17bb4351bac47b2b7204c2aef'
INPUT_TREE = '12c8aef128a58b708d67aae5c125f11bdf522d16'
MANIFEST_SHA256 = '36b78fa2b7de570d4d10a062820df1f568e511336eb2e4efce394a37a3808485'
PATCH_SHA256 = '634799d1b7746b34b0e1d96222275d8f47272dafedcddbff1b42724ad0b7508e'
REPO = "xue-ding-e/AI-Toolbox-Next"
BRANCH = "refs/heads/validation/typed-owner-cli"
HARNESS_DIR = "scripts/typed-owner-cli-validation"
CONTROL_PATHS = {".github/workflows/maintenance-validation.yml",
                 *(HARNESS_DIR + "/" + name for name in ("validate.py", "candidate.json", "candidate.patch"))}
LOG_LIMIT = 16 * 1024 * 1024
TOTAL_LOG_LIMIT = 96 * 1024 * 1024
RECORDS = []
TOTAL_LOG_BYTES = 0
ACTIVE = None
EVIDENCE = None
PHASE = "unknown"
STAGE = "guard"
JOB_DEADLINE = None
FINALIZING = False
FINALIZE_RESERVE = 90
FIXTURE = None
FIXTURE_ID = None
LAST_SCAN = 0
FIXTURE_BUDGET = 32 * 1024 * 1024
FIXTURE_FILE_LIMIT = 8 * 1024 * 1024
CACHE_ROOT = None
TARGET_ROOT = None
RUN_ROOT = None
CACHE_CHECK_AT = 0
CACHE_OBSERVATION = {}
ARCHIVES_ALLOWED = False
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
ALLOWED_REMOVALS = {
    ("tempfile", "3.24.0", REGISTRY), ("rustix", "1.1.3", REGISTRY),
    ("linux-raw-sys", "0.11.0", REGISTRY), ("fastrand", "2.3.0", REGISTRY),
    ("errno", "0.3.14", REGISTRY),
}
FORMAT_FILES = ('crates/ai-toolbox-management/src/codex_service/attach_tests.rs', 'crates/ai-toolbox-storage/src/sqlite_state/lifetime_tests.rs', 'crates/ai-toolbox-automation/tests/owner_console.rs')
MUTABLE_LOCK = 'crates/ai-toolbox-automation/Cargo.lock'
CANDIDATE_PATHS = {'crates/ai-toolbox-automation/src/lib.rs', 'crates/ai-toolbox-automation/Cargo.toml', 'crates/ai-toolbox-automation/tests/owner_console.rs', 'crates/ai-toolbox-storage/AGENTS.md', 'crates/ai-toolbox-management/src/codex_service.rs', 'crates/ai-toolbox-storage/src/sqlite_state.rs', 'crates/ai-toolbox-automation/src/cli.rs', 'crates/ai-toolbox-management/src/owner.rs', 'crates/ai-toolbox-management/AGENTS.md', 'crates/ai-toolbox-management/src/codex_service/attach_tests.rs', 'crates/ai-toolbox-automation/tests/console.rs', 'crates/ai-toolbox-automation/AGENTS.md', 'crates/ai-toolbox-storage/src/sqlite_state/lifetime_tests.rs'}
EXPECTED_COUNTS = {'abrupt_actor_exits': 1, 'automation_ordinary': 14, 'desktop_controls': 4, 'management_lib': 61, 'management_ordinary': 72, 'normal_actor_passes': 12, 'ordinary_total': 108, 'owner_ignored_actor_entry': 1, 'owner_parents': 11, 'storage_ordinary': 22}
DESKTOP_SELECTED = {'desktop_reexports': ['desktop_reexports_share_storage_types_and_connection'], 'desktop_sqlite': ['sqlite_state_limits_lock_scope_with_closure_api', 'transaction_rolls_back_on_error', 'update_hook_records_changed_table_names']}

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
    # that outer boundary. Owner parents spawn bounded actors in the inherited
    # session (no setsid/daemon); the separately bounded shell helper starts its
    # own group and waits/reaps it. These known fixtures are not a guarantee
    # against arbitrary unseen escaping descendants.
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


def observe_exit(child):
    """Observe without reaping, retaining the leader PID/session generation.

    A zombie leader cannot have its numeric PID reused. Keep it until the final
    ownership scan has finished, so matching sid/pgid cannot admit an unrelated
    new session merely because the old leader PID was reallocated.
    """
    result = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    if result is None:
        return None
    if result.si_code == os.CLD_EXITED:
        return result.si_status
    require(result.si_code in {os.CLD_KILLED, os.CLD_DUMPED}, "unexpected child wait status")
    return -result.si_status


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
            observe_exit(child)
            if not owned_members(child.pid, known):
                break
            time.sleep(0.05)
    # This final ownership scan occurs while leader PID is still reserved.
    # No sid/pgid scan is permitted after the unique final reap below.
    remaining = owned_members(child.pid, known)
    try:
        child.wait(timeout=1)
    except subprocess.TimeoutExpired:
        remaining["leader_not_reaped"] = {"pid": child.pid}
    return remaining


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
            check_disk_budgets()
            if FIXTURE is not None:
                check_fixture_budget()
            exit_code = observe_exit(child)
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
                    record["exit_code"] = child.returncode
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


def check_fixture_budget():
    global LAST_SCAN
    if time.monotonic() - LAST_SCAN < .5:
        return
    LAST_SCAN = time.monotonic()
    root = FIXTURE.lstat()
    require((root.st_dev, root.st_ino) == FIXTURE_ID
            and root.st_uid == os.geteuid() and stat.S_IMODE(root.st_mode) == 0o700,
            "fixture root identity/mode changed")
    total = 0
    count = 0
    def scan_error(error):
        if isinstance(error, FileNotFoundError) and Path(error.filename) != FIXTURE:
            return
        raise error
    for current, dirs, files in os.walk(FIXTURE, followlinks=False, onerror=scan_error):
        for name in dirs + files:
            path = Path(current) / name
            try:
                item = path.lstat()
            except FileNotFoundError:
                continue
            count += 1
            if stat.S_ISREG(item.st_mode):
                require(item.st_size <= FIXTURE_FILE_LIMIT, "fixture file exceeds per-file limit")
                total += item.st_size
            require(count <= 512 and total <= FIXTURE_BUDGET,
                    "synthetic SQLite/file fixture storage budget exceeded")
    space = os.statvfs(FIXTURE)
    require(space.f_bavail * space.f_frsize >= 16 * 1024 * 1024,
            "runtime tmpfs low space")


def clean_env(root):
    # Fresh runner-owned HOME/profile/cache/temp roots; never inherit tokens,
    # proxy credentials, DISPLAY, cargo config, or arbitrary compiler flags.
    root.mkdir(mode=0o700)
    env = {"PATH": os.environ["PATH"], "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8",
           "RUSTUP_TOOLCHAIN": "1.99.1", "CARGO_BUILD_JOBS": "2",
           "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0",
           "CARGO_INCREMENTAL": "0", "RUST_BACKTRACE": "0",
           "CARGO_TERM_COLOR": "never", "CARGO_HTTP_TIMEOUT": "20",
           "CARGO_NET_RETRY": "0", "GIT_CONFIG_NOSYSTEM": "1",
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


def source_hash(path):
    return digest(os.fsencode(os.readlink(path))) if path.is_symlink() else file_digest(path)


def scan_disk(root, byte_limit, entry_limit, index_only=False, allowed_symlinks=()):
    total = count = 0
    for current, directories, files in os.walk(root, followlinks=False):
        for name in directories + files:
            path = Path(current) / name
            try:
                info = path.lstat()
            except FileNotFoundError:
                continue
            require(stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode)
                    or (stat.S_ISLNK(info.st_mode) and path in allowed_symlinks),
                    "unexpected special cache/target object")
            if index_only:
                require(not path.is_relative_to(root / "registry/cache")
                        and not path.is_relative_to(root / "registry/src"),
                        "archive/source download forbidden during lock resolution")
            count += 1
            total += info.st_size if stat.S_ISREG(info.st_mode) else 0
            require(total <= byte_limit and count <= entry_limit, "disk/cache budget exceeded")
    return {"bytes": total, "entries": count}


def check_disk_budgets(force=False):
    global CACHE_CHECK_AT, CACHE_OBSERVATION
    if CACHE_ROOT is None or (not force and time.monotonic() < CACHE_CHECK_AT):
        return
    CACHE_CHECK_AT = time.monotonic() + (5 if ARCHIVES_ALLOWED else 0.5)
    cache = scan_disk(CACHE_ROOT, 2 * 1024**3 if ARCHIVES_ALLOWED else 160 * 1024**2,
                      200000 if ARCHIVES_ALLOWED else 30000, not ARCHIVES_ALLOWED)
    target = scan_disk(TARGET_ROOT, 12 * 1024**3, 200000)
    # Source, cache, target and isolated homes remain in this single owned root.
    require(cache["bytes"] + target["bytes"] <= 14 * 1024**3, "combined cache/target cap exceeded")
    combined = scan_disk(RUN_ROOT, 16 * 1024**3, 450000, allowed_symlinks={
        RUN_ROOT / "source/.claude/skills", RUN_ROOT / "source/public/tray-icon.png"})
    evidence = scan_disk(EVIDENCE, 16 * 1024**3, 50000)
    require(combined["bytes"] + evidence["bytes"] <= 16 * 1024**3,
            "whole owned source/home/cache/target/evidence disk cap exceeded")
    space = os.statvfs(RUN_ROOT)
    require(space.f_bavail * space.f_frsize >= 2 * 1024**3, "runner disk free-space reserve reached")
    for leaf in (CACHE_ROOT / "registry/index").glob("*"):
        require(leaf.name.startswith("index.crates.io-") and not leaf.is_symlink(),
                "unreviewed registry cache identity")
        config = leaf / "config.json"
        if config.exists():
            value = json.loads(config.read_text())
            require(value.get("dl") in {"https://static.crates.io/crates",
                                        "https://static.crates.io/crates/{crate}/{crate}-{version}.crate"}
                    and value.get("api", "https://crates.io") == "https://crates.io",
                    "nonofficial registry archive/API URL")
    CACHE_OBSERVATION = {"cache": cache, "target": target, "combined_run_root": combined, "evidence": evidence,
                         "whole_owned_disk_bytes": combined["bytes"] + evidence["bytes"],
                         "source_archives_allowed": ARCHIVES_ALLOWED,
                         "sampled_not_a_filesystem_quota": True}


def source_check(checkout, manifest, manifest_path):
    require(file_digest(manifest_path) == MANIFEST_SHA256, "sealed manifest bytes differ")
    require(manifest["schema_version"] == 1
            and manifest["state"] == "sealed-input-pending-harness-review", "wrong manifest state")
    require(os.environ.get("GITHUB_REPOSITORY") == REPO, "wrong repository")
    require(os.environ.get("GITHUB_REF") == BRANCH, "wrong validation ref")
    require(os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch", "manual only")
    head = os.environ.get("GITHUB_SHA", "")
    require(re.fullmatch(r"[0-9a-f]{40}", head) and os.environ.get("EXPECTED_SHA") == head,
            "exact reviewed expected SHA required")
    require(manifest["source_base"] == BASE and manifest["base_tree"] == BASE_TREE
            and manifest["input_tree"] == INPUT_TREE and manifest["patch_sha256"] == PATCH_SHA256,
            "candidate provenance differs")
    require(set(manifest["candidate_files"]) == CANDIDATE_PATHS
            and len(CANDIDATE_PATHS) == 13 and tuple(manifest["format_files"]) == FORMAT_FILES
            and manifest["mutable_lock"] == MUTABLE_LOCK and manifest["counts"] == EXPECTED_COUNTS
            and manifest["desktop_selected"] == DESKTOP_SELECTED, "reviewed scope differs")
    require(not (CANDIDATE_PATHS & set(manifest["preserved_files"])), "overlapping source maps")
    for path, checksum in {**manifest["candidate_files"], **manifest["preserved_files"]}.items():
        require(not Path(path).is_absolute() and ".." not in Path(path).parts
                and re.fullmatch(r"[0-9a-f]{64}", checksum), "unsafe source map")
    env = clean_env(EVIDENCE / (PHASE + "-git-environment"))
    def git(label, args):
        return run(label, ["git", *args], 15, checkout, env)
    require(git("head", ["rev-parse", "HEAD"]).decode().strip() == head, "checkout SHA differs")
    require(git("single-parent", ["rev-list", "--parents", "-n", "1", "HEAD"]).decode().split()
            == [head, BASE], "one harness commit directly above fixed base required")
    require(git("base-tree", ["rev-parse", BASE + "^{tree}"]).decode().strip() == BASE_TREE,
            "base tree differs")
    require(not git("clean-checkout", ["status", "--porcelain", "--untracked-files=all"]).strip(),
            "dirty harness checkout")
    require(set(git("harness-scope", ["diff", "--no-renames", "--name-only", BASE, "HEAD"])
                .decode().splitlines()) == CONTROL_PATHS, "must change only four harness files")
    patch = manifest_path.with_name("candidate.patch")
    require(not patch.is_symlink() and file_digest(patch) == PATCH_SHA256, "candidate patch differs")
    write_json(PHASE + "-source.json", {
        "base": BASE, "base_tree": BASE_TREE, "input_tree": INPUT_TREE,
        "validation_commit": head, "manifest_sha256": MANIFEST_SHA256,
        "patch_sha256": PATCH_SHA256, "driver_sha256": file_digest(Path(__file__)),
        "workflow_sha256": file_digest(checkout / ".github/workflows/maintenance-validation.yml"),
        "candidate_files": manifest["candidate_files"], "counts": EXPECTED_COUNTS,
    })
    return patch


def verify_source_inventory(source, manifest):
    actual = {}
    for path, spec in manifest["source_inventory"].items():
        text = (source / path).read_text()
        names = re.findall(r"#\[test\](?:\s*#\[[^\n]+\])*\s*fn\s+(\w+)", text)
        excluded = spec.get("linux_excluded", [])
        for name in excluded:
            require(re.search(r'#\[cfg\(not\(target_os = "linux"\)\)\]\s*#\[test\]\s*fn\s+'
                              + re.escape(name) + r"\s*\(", text), "non-Linux exclusion changed")
        names = [spec["prefix"] + name for name in names if name not in excluded]
        require(len(names) == spec["count"], "source count differs: " + path)
        actual.setdefault(spec["group"], []).extend(names)
        ignored = re.findall(r'#\[ignore[^\n]*\]\s*fn\s+(\w+)', text)
        require(ignored == (["fixture_process"] if spec["group"] == "owner" else []),
                "unexpected ignored source case")
    actual = {key: sorted(values) for key, values in actual.items()}
    require(actual == manifest["inventories"], "exact source inventories differ")
    console = (source / "crates/ai-toolbox-automation/tests/owner_console.rs").read_text()
    require('const CHILD_LIMIT: Duration = Duration::from_secs(13);' in console
            and 'const CLEANUP_LIMIT: Duration = Duration::from_secs(2);' in console,
            "owner console child/cleanup limits differ")
    for group, names in DESKTOP_SELECTED.items():
        require(set(names) <= set(actual[group]), "selected desktop control absent")
    write_json("validation-source-inventories.json", {"inventories": actual,
               "owner_console_fault_scope": "Hello-stage faults; happy-path two-stage protocol only"})


def verify_hashes(source, manifest):
    expected = {**manifest["candidate_files"], **manifest["preserved_files"]}
    for path, checksum in expected.items():
        file = source / path
        if path in manifest["preserved_symlinks"]:
            require(file.is_symlink() and os.readlink(file) == manifest["preserved_symlinks"][path],
                    "preserved symlink differs")
        else:
            require(not file.is_symlink() and file.is_file()
                    and file.resolve(strict=True).is_relative_to(source), "source escaped owned worktree")
        require(source_hash(file) == checksum, "input source differs: " + path)


def lock_records(raw):
    text = raw.decode("utf-8")
    parts = text.split("[[package]]\n")
    require(re.search(r"(?m)^version = 4$", parts[0]) is not None, "lock version differs")
    result = {}
    for block in parts[1:]:
        def field(name, optional=False):
            values = re.findall(r'^' + name + r' = (".*")$', block, re.M)
            require(len(values) == 1 or (optional and not values), "unexpected lock field " + name)
            return json.loads(values[0]) if values else None
        name, version, source = field("name"), field("version"), field("source", True)
        checksum = field("checksum", True)
        key = (name, version, source)
        require(key not in result, "duplicate package identity")
        if source:
            require(source == REGISTRY and re.fullmatch(r"[0-9a-f]{64}", checksum or ""),
                    "nonofficial registry or missing checksum")
        else:
            require(checksum is None and name in {"ai-toolbox-automation", "ai-toolbox-management",
                                                   "ai-toolbox-storage", "ai-toolbox-codex-config", "ai-toolbox"},
                    "unreviewed local package")
        dependencies = []
        match = re.search(r"(?ms)^dependencies = \[\n(.*?)^\]\n?", block)
        if match:
            for line in match.group(1).splitlines():
                require(re.fullmatch(r' "[^"\n]+",', line), "unparsed dependency edge")
                dependencies.append(json.loads(line.strip().rstrip(',')))
        result[key] = {"name": name, "version": version, "source": source, "checksum": checksum,
                       "dependencies": dependencies, "raw": block}
    require(result, "empty lock inventory")
    return result


def manifest_dependency_names(path, include_dev):
    section = ""
    names = set()
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if line.startswith("["):
            section = line
        elif line and not line.startswith("#"):
            normal = section == "[dependencies]" or section.endswith(".dependencies]")
            dev = section == "[dev-dependencies]" or section.endswith(".dev-dependencies]")
            if normal or (include_dev and dev):
                match = re.match(r"^([A-Za-z0-9_-]+)\s*=", line)
                require(match is not None, "unparsed source manifest dependency")
                names.add(match.group(1))
    return names


def resolve_edge(edge, records):
    parts = edge.split(" ", 2)
    matches = [key for key in records if key[0] == parts[0]
               and (len(parts) < 2 or key[1] == parts[1])
               and (len(parts) < 3 or key[2] == parts[2].strip("()"))]
    require(len(matches) == 1, "ambiguous or unknown seed dependency edge: " + edge)
    return matches[0]


def seed_reachability(source, records):
    management = source / "crates/ai-toolbox-management/Cargo.toml"
    automation = source / "crates/ai-toolbox-automation/Cargo.toml"
    normal = manifest_dependency_names(management, False)
    all_management = manifest_dependency_names(management, True)
    automation_roots = manifest_dependency_names(automation, True)
    require(all_management - normal == {"tempfile"}, "management dev-only edge changed")
    key = ("ai-toolbox-management", "0.1.0", None)
    require({edge.split(" ", 1)[0] for edge in records[key]["dependencies"]} == all_management,
            "seed management record differs from manifest dependencies")
    require(automation_roots == {"ai-toolbox-management", "tokio", "libc", "serde_json"},
            "automation direct dependency set changed")
    pending = [resolve_edge(name, records) for name in sorted(automation_roots)]
    reached = set()
    while pending:
        current = pending.pop()
        if current in reached:
            continue
        reached.add(current)
        edges = records[current]["dependencies"]
        if current == key:
            edges = [edge for edge in edges if edge.split(" ", 1)[0] in normal]
        pending.extend(resolve_edge(edge, records) for edge in edges)
    unreachable = {key for key in records if key[2] and key not in reached}
    require(unreachable == ALLOWED_REMOVALS, "seed graph no longer proves exact five-package removal bound")
    write_json("validation-seed-reachability.json", {
        "source_manifests": {str(p.relative_to(source)): file_digest(p) for p in (management, automation)},
        "management_normal_roots": sorted(normal), "management_excluded_dev_edge": "tempfile",
        "automation_roots_including_dev": sorted(automation_roots),
        "unreachable_registry_records": [records[key] for key in sorted(unreachable)],
        "method": "Traverse actual seed edges, using manifest-derived normal management dependencies; no guessed edges",
    })


def resolve_automation_lock(checkout, source, env):
    global STAGE
    STAGE = "lock_resolution"
    management_raw = run("baseline-management-lock", ["git", "show", BASE + ":crates/ai-toolbox-management/Cargo.lock"],
                         15, checkout, env)
    automation_raw = run("baseline-automation-lock", ["git", "show", BASE + ":" + MUTABLE_LOCK], 15, checkout, env)
    management = lock_records(management_raw)
    automation = lock_records(automation_raw)
    auto_key = ("ai-toolbox-automation", "0.1.0", None)
    require(set(automation) == {auto_key} and automation[auto_key]["dependencies"] == [],
            "baseline automation lock is not the original single-package source")
    require(auto_key not in management, "automation record already present in seed")
    # Both components are exact bytes from the fixed main baseline. No package
    # dependency line is manufactured here; Cargo alone resolves new edges.
    seed = management_raw.rstrip(b"\n") + b"\n\n[[package]]\n" + automation[auto_key]["raw"].encode()
    seed_records = lock_records(seed)
    require(set(seed_records) == set(management) | {auto_key}, "seed composition differs")
    seed_reachability(source, seed_records)
    lock = source / MUTABLE_LOCK
    lock.write_bytes(seed)
    (EVIDENCE / "validation-seed.lock").write_bytes(seed)
    write_json("validation-seed-provenance.json", {
        "base": BASE, "management_lock_sha256": digest(management_raw),
        "automation_original_lock_sha256": digest(automation_raw), "seed_sha256": digest(seed),
        "seed_is_resolved": False, "no_manual_dependency_edges": True,
    })
    error = None
    audit = {"ok": False, "full_record_and_edge_review_required": True}
    try:
        run("resolve-automation-only", ["cargo", "update", "-p", "ai-toolbox-automation",
             "--manifest-path", str(source / "crates/ai-toolbox-automation/Cargo.toml")],
            65, source, env)
        check_disk_budgets(force=True)
        resolved = lock.read_bytes()
        actual = lock_records(resolved)
        removed = set(seed_records) - set(actual)
        added = set(actual) - set(seed_records)
        changed = [key for key in set(actual) & set(seed_records)
                   if actual[key]["raw"] != seed_records[key]["raw"]]
        audit.update(removed=[seed_records[key] for key in sorted(removed)],
                     added=[actual[key] for key in sorted(added)],
                     changed_records=[{"identity": key, "before": seed_records[key], "after": actual[key]}
                                      for key in sorted(changed)],
                     resolved_sha256=digest(resolved), seed_sha256=digest(seed),
                     surviving_registry_identities_and_checksums=[
                         {k: v for k, v in actual[key].items() if k not in {"raw", "dependencies"}}
                         for key in sorted(actual) if key[2]])
        (EVIDENCE / "validation-resolved.lock").write_bytes(resolved)
        (EVIDENCE / "validation-lock.diff").write_text("".join(difflib.unified_diff(
            seed.decode().splitlines(True), resolved.decode().splitlines(True),
            fromfile="actual-baseline-seed/Cargo.lock", tofile="cargo-resolved/Cargo.lock")))
        write_json("validation-lock-records-before.json", list(seed_records.values()))
        write_json("validation-lock-records-after.json", list(actual.values()))
        require(not added, "new/changed identity or version/source is not approved")
        require(removed <= ALLOWED_REMOVALS, "removed package outside exact five-package whitelist")
        require(all(actual[key]["checksum"] == seed_records[key]["checksum"] for key in actual),
                "surviving package checksum changed")
        require({key for key in actual if key[2] is None} == {
            ("ai-toolbox-automation", "0.1.0", None), ("ai-toolbox-management", "0.1.0", None),
            ("ai-toolbox-storage", "0.1.0", None), ("ai-toolbox-codex-config", "0.1.0", None)},
            "local package closure changed")
        for key, record in actual.items():
            for edge in record["dependencies"]:
                resolve_edge(edge, actual)
        auto_dependencies = {edge.split(" ", 1)[0] for edge in actual[auto_key]["dependencies"]}
        require(auto_dependencies == manifest_dependency_names(source / "crates/ai-toolbox-automation/Cargo.toml", True),
                "resolved automation root edges differ from actual source manifest")
        audit.update(ok=True, compile_has_not_yet_run=True, unknown_result_is_pass=False,
                     registry_identity_version_source_checksum_preserved=True,
                     zero_dependency_edge_drift_claimed=False)
    except BaseException as exc:
        error = f"{type(exc).__name__}: {exc}"
        raise
    finally:
        # Retain failed/unresolved bytes too. They are not usable tested output.
        (EVIDENCE / "validation-current-automation.lock").write_bytes(lock.read_bytes())
        audit.update(error=error, cache=CACHE_OBSERVATION)
        write_json("validation-lock-audit.json", audit)
    return audit
def prepare_source(checkout, manifest, patch, env):
    global STAGE
    STAGE = "source_preparation"
    source = RUN_ROOT / "source"
    run("create-source", ["git", "worktree", "add", "--detach", str(source), BASE], 30, checkout, env)
    run("apply-candidate", ["git", "apply", "--index", "--whitespace=error", "--", str(patch)], 30, source, env)
    verify_hashes(source, manifest)
    status = run("input-status", ["git", "diff", "--cached", "--name-status", "--no-renames", BASE], 15, source, env)
    require({line.split("\t")[1]: line.split("\t")[0] for line in status.decode().splitlines()}
            == manifest["candidate_statuses"], "input status scope differs")
    require(run("input-tree", ["git", "write-tree"], 15, source, env).decode().strip() == INPUT_TREE,
            "complete input tree differs")
    canonical = run("input-patch", ["git", "diff", "--cached", "--binary", "--full-index",
                    "--no-ext-diff", "--no-renames", BASE], 30, source, env)
    require(digest(canonical) == PATCH_SHA256, "canonical input patch differs")
    require(not run("input-working-diff", ["git", "diff", "--name-only"], 15, source, env).strip(),
            "input unstaged changes")
    require(not run("input-untracked", ["git", "ls-files", "--others", "--exclude-standard"],
                    15, source, env).strip(), "untracked input")
    for ancestor in [source, *source.parents]:
        require(not (ancestor / ".cargo/config").exists()
                and not (ancestor / ".cargo/config.toml").exists(),
                "inherited ancestor cargo configuration forbidden")
    for candidate in source.rglob(".cargo"):
        require(not (candidate / "config").exists() and not (candidate / "config.toml").exists(),
                "candidate cargo configuration forbidden")
    verify_source_inventory(source, manifest)
    before = snapshot_source(source, env, "input-all")
    require(set(before) == set(manifest["candidate_files"]) | set(manifest["preserved_files"]),
            "all-input file map differs")
    write_json("validation-input-all-filehashes.json", before)
    return source, before


def format_and_seal(source, manifest, before, env):
    global STAGE
    STAGE = "format_and_seal"
    for path in FORMAT_FILES:
        text = (source / path).read_text()
        require(not re.search(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*;", text)
                and not re.search(r"\binclude!\s*\(", text), "formatter could traverse an external module")
    config = RUN_ROOT / "rustfmt.toml"
    config.write_text('edition = "2021"\nreorder_imports = false\nreorder_modules = false\n')
    run("format-three-new-tests", ["rustfmt", "--edition", "2021", "--config-path", str(config),
                                  *[str(source / path) for path in FORMAT_FILES]], 30, source, env)
    allowed = set(FORMAT_FILES) | {MUTABLE_LOCK}
    unstaged = set(run("allowed-generated-paths", ["git", "diff", "--name-only"], 15, source, env).decode().splitlines())
    require(unstaged <= allowed and MUTABLE_LOCK in unstaged, "unexpected generated source scope")
    observed = snapshot_source(source, env, "formatted-all")
    require(set(observed) == set(before) and all(observed[path] == before[path] for path in before if path not in allowed),
            "non-format candidate or preserved source changed")
    run("stage-tested-bytes", ["git", "add", "--", *FORMAT_FILES, MUTABLE_LOCK], 15, source, env)
    tested_tree = run("tested-tree", ["git", "write-tree"], 15, source, env).decode().strip()
    tested_patch = run("tested-patch", ["git", "diff", "--cached", "--binary", "--full-index",
                        "--no-ext-diff", "--no-renames", BASE], 30, source, env)
    output_paths = set(run("tested-paths", ["git", "diff", "--cached", "--name-only", BASE],
                           15, source, env).decode().splitlines())
    require(output_paths == CANDIDATE_PATHS | {MUTABLE_LOCK}, "tested patch must contain exactly 14 product paths")
    (EVIDENCE / "validation-tested.patch").write_bytes(tested_patch)
    write_json("validation-tested-all-filehashes.json", observed)
    for index, path in enumerate(FORMAT_FILES):
        shutil.copyfile(source / path, EVIDENCE / f"validation-formatted-{index + 1}-{Path(path).name}")
    shutil.copyfile(source / MUTABLE_LOCK, EVIDENCE / "validation-tested-automation.lock")
    write_json("validation-tested-source.json", {
        "base": BASE, "input_tree": INPUT_TREE, "tested_tree": tested_tree,
        "input_patch_sha256": PATCH_SHA256, "tested_patch_sha256": digest(tested_patch),
        "mutable_output_paths": sorted(allowed), "actual_changed_input_bytes": sorted(unstaged),
        "tested_file_count": len(observed), "compiled": False,
        "must_adopt_these_tested_bytes_not_original_input": True,
        "parent_format_and_lock_edge_review_required_before_product_commit": True,
    })
    verify_source_inventory(source, manifest)
    return observed, tested_tree, digest(tested_patch)


def create_runtime_fixture(source, env):
    global FIXTURE, FIXTURE_ID
    parent = Path("/dev/shm")
    require(parent.is_dir() and not parent.is_symlink(), "existing tmpfs unavailable")
    magic = run("runtime-filesystem", ["stat", "-f", "-c", "%t", str(parent)], 5, source, env).decode().strip()
    require(magic == "1021994", "runtime root must be TMPFS_MAGIC, never inferred from overlay")
    space = os.statvfs(parent)
    require(space.f_bavail * space.f_frsize >= FIXTURE_BUDGET + 16 * 1024**2, "runtime tmpfs space insufficient")
    FIXTURE = Path(tempfile.mkdtemp(prefix="at-", dir=parent))
    identity = FIXTURE.lstat()
    FIXTURE_ID = (identity.st_dev, identity.st_ino)
    require(identity.st_uid == os.geteuid() and stat.S_IMODE(identity.st_mode) == 0o700,
            "private fixture ownership/mode differs")
    write_json("validation-runtime.json", {
        "root": str(FIXTURE), "magic": magic, "mode": "0700", "identity": list(FIXTURE_ID),
        "sampled_aggregate_bytes": FIXTURE_BUDGET, "hard_per_file_bytes": FIXTURE_FILE_LIMIT,
        "entry_limit": 512, "no_mount_or_existing_directory_mode_change": True,
    })
    test_env = env.copy()
    test_env.update(TMPDIR=str(FIXTURE), TMP=str(FIXTURE), TEMP=str(FIXTURE),
                    SQLITE_TMPDIR=str(FIXTURE), CARGO_NET_OFFLINE="true")
    return test_env


TEST_LAUNCH = [sys.executable, "-c",
    "import os,resource,sys; resource.setrlimit(resource.RLIMIT_FSIZE,(8388608,8388608)); os.execv(sys.argv[1],sys.argv[1:])"]


def compile_targets(label, manifest_path, arguments, expected, source, env, timeout):
    global STAGE
    STAGE = "compile"
    output = run(label + "-compile", ["cargo", "test", "--locked", "--manifest-path", str(manifest_path),
                 *arguments, "--no-run", "--jobs", "2", "--message-format=json"], timeout, source, env)
    messages = [json.loads(line) for line in output.decode().splitlines()]
    require(sum(message.get("reason") == "build-finished" and message.get("success") is True
                for message in messages) == 1, "missing unique successful compiler completion")
    bins = {key: artifact(output, manifest_path, target_name, TARGET_ROOT) for key, target_name in expected.items()}
    if label == "automation":
        # The CLI used by env!(CARGO_BIN_EXE_ai-toolbox-cli) is a real Cargo
        # artifact. Do not execute an arbitrary guessed target/debug path.
        matches = []
        for item in messages:
            if (item.get("reason") == "compiler-artifact" and item.get("executable")
                    and Path(item["manifest_path"]).resolve() == manifest_path.resolve()
                    and item.get("target", {}).get("name") == "ai-toolbox-cli"
                    and item.get("target", {}).get("kind") == ["bin"]
                    and item.get("profile", {}).get("test") is False):
                executable = Path(item["executable"]).resolve(strict=True)
                require(executable.is_relative_to(TARGET_ROOT) and executable.is_file()
                        and os.access(executable, os.X_OK), "CLI escaped target")
                matches.append(executable)
        require(len(matches) == 1, "unique actual CLI artifact absent")
        write_json("validation-console-binary.json", {"path": str(matches[0]), "sha256": file_digest(matches[0]),
                   "execution": "Only selected integration tests invoke the Cargo-provided CLI path"})
    write_json("validation-" + label + "-binaries.json", {
        key: {"path": str(value), "sha256": file_digest(value)} for key, value in bins.items()})
    return bins


def check_binary_inventory(group, binary, manifest, source, test_env):
    global STAGE
    STAGE = "inventory_guard"
    listed = inventory(run(group.replace("_", "-") + "-list", TEST_LAUNCH + [str(binary), "--list", "--test-threads=1"],
                           15, source, test_env))
    require(listed == manifest["inventories"][group], "exact compiled inventory differs: " + group)
    ignored = inventory(run(group.replace("_", "-") + "-ignored-list",
                            TEST_LAUNCH + [str(binary), "--ignored", "--list", "--test-threads=1"],
                            15, source, test_env))
    require(ignored == (["fixture_process"] if group == "owner" else []), "unexpected ignored compiled test")
    write_json("validation-" + group.replace("_", "-") + "-inventory.json", {
        "names": listed, "ignored": ignored, "exact_match": True})
    return listed


def execute_group(group, binary, count, timeout, source, test_env):
    global STAGE
    STAGE = "tests"
    output = run(group.replace("_", "-") + "-tests",
                 TEST_LAUNCH + [str(binary), "--test-threads=1", "--show-output"], timeout, source, test_env)
    if group == "owner":
        text = output.decode()
        require(len(re.findall(r"^test result: ok\. 11 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;",
                               text, re.M)) == 1, "owner parent summary differs")
        require(len(re.findall(r"^verified actor execution: ", text, re.M)) == 12,
                "expected twelve normal actor completions")
        require(re.findall(r"^verified abrupt actor execution: (.+)$", text, re.M) == ["stale"],
                "expected one deliberate abrupt actor exit")
    else:
        check_pass(output, count, 0)
    check_fixture_budget()
    require(not list(FIXTURE.iterdir()), "test group leaked fixture objects: " + group)


def validate(checkout, manifest, manifest_path):
    global RUN_ROOT, CACHE_ROOT, TARGET_ROOT, ARCHIVES_ALLOWED, STAGE, FINALIZING, FIXTURE
    patch = source_check(checkout, manifest, manifest_path)
    RUN_ROOT = Path(os.environ["RUNNER_TEMP"]).resolve() / (
        "typed-owner-cli-" + os.environ["GITHUB_RUN_ID"] + "-" + os.environ["GITHUB_RUN_ATTEMPT"])
    require(not RUN_ROOT.exists() and not RUN_ROOT.is_symlink(), "run root already exists")
    RUN_ROOT.mkdir(mode=0o700)
    env = clean_env(RUN_ROOT / "environment")
    env.update(CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse",
               CARGO_REGISTRIES_CRATES_IO_INDEX="sparse+https://index.crates.io/")
    CACHE_ROOT = Path(env["CARGO_HOME"])
    TARGET_ROOT = Path(env["CARGO_TARGET_DIR"]).resolve()
    (CACHE_ROOT / "config.toml").write_text('[source.crates-io]\nregistry = "sparse+https://index.crates.io/"\n')
    source = before = tested = tested_tree = tested_patch_sha = None
    dist = dist_identity = asset_identity = None
    result = {"ok": False, "product_ready": False, "error": None, "passed_groups": {},
              "desktop_passed": [], "lock_and_format_review_required": True,
              "scope": "108 ordinary core cases + 13 separate actors; desktop lib compile-only + four selected controls"}
    try:
        STAGE = "toolchain_guard"
        require(run("rust-version", ["rustc", "--version"], 10, checkout, env).decode().startswith("rustc 1.99.1 "),
                "unexpected rustc version")
        require(run("cargo-version", ["cargo", "--version"], 10, checkout, env).decode().startswith("cargo 1.99.1 "),
                "unexpected cargo version")
        run("rustfmt-version", ["rustfmt", "--version"], 10, checkout, env)
        source, before = prepare_source(checkout, manifest, patch, env)
        result["lock_audit"] = resolve_automation_lock(checkout, source, env)
        # Audit immutable independent locks as well, before any compile or crate
        # archive becomes permitted. Their bytes are fixed by the full manifest.
        result["immutable_lock_audits"] = {}
        for path in ("crates/ai-toolbox-management/Cargo.lock", "crates/ai-toolbox-storage/Cargo.lock", "tauri/Cargo.lock"):
            records = lock_records((source / path).read_bytes())
            result["immutable_lock_audits"][path] = {"sha256": file_digest(source / path), "records": len(records)}
        tested, tested_tree, tested_patch_sha = format_and_seal(source, manifest, before, env)
        result.update(tested_tree=tested_tree, tested_patch_sha256=tested_patch_sha)
        check_disk_budgets(force=True)
        ARCHIVES_ALLOWED = True
        # Archive downloads start only after the strict registry drift gate.
        core_specs = [
            ("management", "crates/ai-toolbox-management/Cargo.toml", ["--lib", "--test", "owner_process"],
             {"management_lib": "ai_toolbox_management", "owner": "owner_process"}),
            ("storage", "crates/ai-toolbox-storage/Cargo.toml", ["--lib", "--test", "sqlite_jsonb", "--test", "codex_context_journal"],
             {"storage_lib": "ai_toolbox_storage", "storage_sqlite": "sqlite_jsonb", "storage_journal": "codex_context_journal"}),
            ("automation", "crates/ai-toolbox-automation/Cargo.toml", ["--lib", "--test", "console", "--test", "owner_console"],
             {"automation_lib": "ai_toolbox_automation", "console": "console", "owner_console": "owner_console"}),
        ]
        bins = {}
        for label, path, args, expected in core_specs:
            bins.update(compile_targets(label, source / path, args, expected, source, env, 240))
        test_env = create_runtime_fixture(source, env)
        for group, binary in bins.items():
            check_binary_inventory(group, binary, manifest, source, test_env)
        groups = [("management_lib", 61, 150), ("owner", 11, 180), ("storage_lib", 8, 60),
                  ("storage_sqlite", 4, 60), ("storage_journal", 10, 60),
                  ("automation_lib", 2, 30), ("console", 6, 90), ("owner_console", 6, 120)]
        for group, count, limit in groups:
            execute_group(group, bins[group], count, limit, source, test_env)
            result["passed_groups"][group] = count
            write_json("validation-progress.json", result)
        require(sum(result["passed_groups"].values()) == 108, "ordinary total differs")
        result["actors"] = {"normal": 12, "abrupt": 1, "ignored_entry_not_counted_as_pass": 1}
        # The same fixed candidate is then compiled as real desktop inputs.
        # No frontend build or GUI startup: an empty owned embedding asset only.
        STAGE = "desktop_asset"
        dist = source / "dist"
        require(not dist.exists() and not dist.is_symlink(), "refuse existing dist")
        dist.mkdir(mode=0o700)
        observed = dist.lstat()
        dist_identity = (observed.st_dev, observed.st_ino)
        asset = dist / "index.html"
        with asset.open("xb"):
            pass
        observed = asset.lstat()
        asset_identity = (observed.st_dev, observed.st_ino)
        write_json("validation-compile-asset.json", {"path": "dist/index.html", "sha256": digest(b""),
                   "purpose": "compile-only empty asset; not frontend or GUI validation"})
        desktop = compile_targets("desktop", source / "tauri/Cargo.toml",
            ["--lib", "--test", "sqlite_jsonb", "--test", "storage_reexports"],
            {"desktop_lib_compile_only": "ai_toolbox_lib", "desktop_sqlite": "sqlite_jsonb",
             "desktop_reexports": "storage_reexports"}, source, env, 900)
        for group, selected in DESKTOP_SELECTED.items():
            listed = check_binary_inventory(group, desktop[group], manifest, source, test_env)
            for index, name in enumerate(selected):
                STAGE = "inventory_guard"
                label = group.replace("_", "-") + f"-case-{index+1}"
                exact = inventory(run(label + "-list", TEST_LAUNCH + [str(desktop[group]), "--exact", name, "--list"],
                                      15, source, test_env))
                require(exact == [name], "exact desktop selection absent")
                STAGE = "tests"
                output = run(label + "-test", TEST_LAUNCH + [str(desktop[group]), "--exact", name,
                              "--test-threads=1", "--show-output"], 30, source, test_env)
                check_pass(output, 1, len(listed) - 1)
                require(not list(FIXTURE.iterdir()), "selected desktop case leaked fixture")
                result["desktop_passed"].append(name)
                write_json("validation-progress.json", result)
        require(len(result["desktop_passed"]) == 4, "four desktop controls required")
        result["ok"] = True
        result["outcome"] = "bounded_validation_passed_pending_lock_and_format_review"
    except BaseException as error:
        result["error"] = f"{type(error).__name__}: {error}"
        result["failed_stage"] = STAGE
        result["outcome"] = ("test_failure" if STAGE == "tests" else
                             "blocked_setup_or_compile_not_product_red" if STAGE in {"compile", "lock_resolution", "toolchain_guard"}
                             else "validation_guard_failure_not_product_red")
    finally:
        FINALIZING = True
        STAGE = "finalization"
        errors = []
        all_exited = ACTIVE is None and all(not record["survivors"] for record in RECORDS)
        result["all_owned_processes_exited"] = all_exited
        # Never delete fixture/lease objects or the compile asset while an owned
        # process may remain alive. Unknown outcomes are retained, never green.
        try:
            require(all_exited, "owned process exit not confirmed; preserve fixture and asset")
            if dist_identity is not None:
                observed = dist.lstat()
                require(not dist.is_symlink() and (observed.st_dev, observed.st_ino) == dist_identity,
                        "compile asset directory replaced")
                if asset_identity is not None:
                    asset = dist / "index.html"
                    observed = asset.lstat()
                    require(not asset.is_symlink() and (observed.st_dev, observed.st_ino) == asset_identity
                            and asset.is_file() and asset.read_bytes() == b"", "compile asset replaced")
                    asset.unlink()
                dist.rmdir()
            if FIXTURE is not None:
                observed = FIXTURE.lstat()
                require((observed.st_dev, observed.st_ino) == FIXTURE_ID
                        and observed.st_uid == os.geteuid() and stat.S_IMODE(observed.st_mode) == 0o700,
                        "runtime fixture identity changed")
                require(shutil.rmtree.avoids_symlink_attacks, "safe owned-tree cleanup unavailable")
                shutil.rmtree(FIXTURE)
                require(not FIXTURE.exists(), "owned runtime fixture remains")
                FIXTURE = None
            result["fixture_and_asset_cleanup_confirmed"] = True
        except BaseException as error:
            errors.append("cleanup: " + str(error))
        try:
            if source is not None and tested is not None:
                after = snapshot_source(source, env, "final-all")
                write_json("validation-final-all-filehashes.json", after)
                require(after == tested, "complete source bytes changed after sealing")
                require(not run("final-working-diff", ["git", "diff", "--name-only"], 15, source, env).strip(),
                        "unstaged source changed during compile/test")
                final_tree = run("final-tree", ["git", "write-tree"], 15, source, env).decode().strip()
                final_patch = run("final-patch", ["git", "diff", "--cached", "--binary", "--full-index",
                                  "--no-ext-diff", "--no-renames", BASE], 30, source, env)
                require(final_tree == tested_tree and digest(final_patch) == tested_patch_sha, "tested tree/patch changed")
                require(not run("final-untracked", ["git", "ls-files", "--others", "--exclude-standard"],
                                15, source, env).strip(), "untracked source after compile/test")
                result["final_source_and_lock_hashes_match_tested_bytes"] = True
                result["all_locks_unchanged_after_sealing"] = all(
                    after[path] == tested[path] for path in tested if path.endswith("Cargo.lock"))
            check_disk_budgets(force=True)
        except BaseException as error:
            errors.append("source/disk proof: " + str(error))
        if errors:
            result["ok"] = False
            result["outcome"] = "finalization_inconclusive_not_product_green"
        result.update(finalization_errors=errors, cache=CACHE_OBSERVATION,
                      command_count=len(RECORDS), commands=RECORDS)
        write_json("validation-result.json", result)
    return result


def main():
    global EVIDENCE, PHASE, JOB_DEADLINE
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=["preflight", "validation"], required=True)
    options = parser.parse_args()
    PHASE = options.phase
    require(sys.platform == "linux" and os.geteuid() != 0, "unprivileged Linux required")
    checkout = Path(os.environ["GITHUB_WORKSPACE"]).resolve()
    runner_temp = Path(os.environ["RUNNER_TEMP"]).resolve()
    require(runner_temp != Path("/tmp") and not runner_temp.is_relative_to(Path("/dev/shm")),
            "source/build/evidence must use runner disk, never tmpfs")
    EVIDENCE = runner_temp / "typed-owner-cli-evidence"
    require(not EVIDENCE.is_symlink(), "evidence symlink refused")
    EVIDENCE.mkdir(mode=0o700, exist_ok=True)
    require(not (EVIDENCE / (PHASE + "-summary.json")).exists(), "phase already ran; no blind retry")
    clock = EVIDENCE / "preflight-budget.json"
    if PHASE == "preflight":
        require(not clock.exists(), "budget already initialized")
        write_json(clock.name, {"deadline_monotonic": time.monotonic() + 37 * 60,
                               "deadline_unix_advisory": time.time() + 37 * 60,
                               "shared_setup_validation_seconds": 2220})
    JOB_DEADLINE = json.loads(clock.read_text())["deadline_monotonic"]
    summary = {"phase": PHASE, "ok": False, "error": None, "product_ready": False,
               "shared_setup_validation_seconds": 2220, "finalization_reserve_seconds": FINALIZE_RESERVE}
    for sig in (signal.SIGTERM, signal.SIGINT):
        signal.signal(sig, interrupted)
    try:
        manifest_path = checkout / HARNESS_DIR / "candidate.json"
        require(not manifest_path.is_symlink(), "manifest symlink refused")
        manifest = json.loads(manifest_path.read_text())
        if PHASE == "preflight":
            source_check(checkout, manifest, manifest_path)
            summary["ok"] = True
        else:
            result = validate(checkout, manifest, manifest_path)
            summary.update(ok=result["ok"], outcome=result["outcome"], error=result["error"])
    except BaseException as error:
        summary.update(error=f"{type(error).__name__}: {error}",
                       outcome="guard_or_setup_failure_not_product_red")
    finally:
        summary.update(commands=len(RECORDS), log_bytes_seen=TOTAL_LOG_BYTES)
        write_json(PHASE + "-summary.json", summary)
        print(json.dumps(summary, sort_keys=True), flush=True)
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
