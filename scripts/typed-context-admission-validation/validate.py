#!/usr/bin/env python3
"""Prepared finite Stage 1 admission validation. Not dispatch authorization.

Reuse the reviewed pidfd supervisor and official APT capture wrapper. Apply only
one frozen 59-file patch above the exact base; no lock resolution, formatting,
dependency changes, full suite, GUI, real profile, credential, or remote service.
All source and lock bytes remain immutable. Cargo --locked rejection stops this
stage. Stage 2 pnpm test, tsc, and full desktop cargo test remain mandatory.
"""
import argparse
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

BASE = '7932250c5d619c7f43938d31b91d1242e9535b07'
BASE_TREE = '882f28625bfc7380863a4d67987d068328832921'
INPUT_TREE = 'e4a72da8e3bc19afa139a65a34ecf92e6d743e34'
MANIFEST_SHA256 = '62a4a118a5cef8c7f6bacc048af05a723623808209ba06ccd6bfdb4d22329502'
PATCH_SHA256 = '23bc2bdca7ad7d6e9c36c51dbd2c3214f208894a6b9eb9acc98658929482b836'
REPO = "xue-ding-e/AI-Toolbox-Next"
BRANCH = "refs/heads/validation/typed-context-admission-newbase"
HARNESS_DIR = "scripts/typed-context-admission-validation"
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
TMPFS_PARENT = None
TMPFS_ID = None
TMPFS_PARENT_FD = None
FIXTURE_FD = None
FIXTURE_NAME = None
# Sealed from candidate.json once its complete bytes and scope have been reviewed.
CANDIDATE_PATHS = set()
EXPECTED_COUNTS = {}
DESKTOP_SELECTED = {}

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


def guard_fixture_deadline():
    reserve = 10 if FINALIZING else FINALIZE_RESERVE
    require(JOB_DEADLINE is None or time.monotonic() < JOB_DEADLINE - reserve,
            "shared deadline reached during private fixture operation")


def verify_fixture_identity():
    """Bind all fixture operations to held directory descriptors, never a prefix."""
    guard_fixture_deadline()
    require(FIXTURE_FD is not None and TMPFS_PARENT_FD is not None,
            "private fixture descriptors unavailable")
    parent = os.fstat(TMPFS_PARENT_FD)
    parent_named = TMPFS_PARENT.lstat()
    require(stat.S_ISDIR(parent_named.st_mode)
            and (parent.st_dev, parent.st_ino) == TMPFS_ID
            and (parent_named.st_dev, parent_named.st_ino) == TMPFS_ID,
            "tmpfs parent identity changed")
    held = os.fstat(FIXTURE_FD)
    named = os.stat(FIXTURE_NAME, dir_fd=TMPFS_PARENT_FD, follow_symlinks=False)
    require(stat.S_ISDIR(held.st_mode) and stat.S_ISDIR(named.st_mode)
            and (held.st_dev, held.st_ino) == FIXTURE_ID
            and (named.st_dev, named.st_ino) == FIXTURE_ID
            and held.st_uid == named.st_uid == os.geteuid()
            and stat.S_IMODE(held.st_mode) == stat.S_IMODE(named.st_mode) == 0o700,
            "private fixture root identity/ownership/mode changed")


def check_fixture_budget(force=False):
    global LAST_SCAN
    if not force and time.monotonic() - LAST_SCAN < .5:
        return
    LAST_SCAN = time.monotonic()
    verify_fixture_identity()
    total = count = 0
    def scan_error(error):
        if isinstance(error, FileNotFoundError):
            return  # A bounded test may finish/remove one of its own children.
        raise error
    # fwalk remains relative to the held private-root descriptor. No enumeration
    # of /dev/shm, no path-prefix ownership inference, and no symlink traversal.
    for _current, dirs, files, current_fd in os.fwalk(
            ".", topdown=True, onerror=scan_error, follow_symlinks=False, dir_fd=FIXTURE_FD):
        for name in dirs + files:
            guard_fixture_deadline()
            try:
                item = os.stat(name, dir_fd=current_fd, follow_symlinks=False)
            except FileNotFoundError:
                continue
            count += 1
            if stat.S_ISREG(item.st_mode):
                require(item.st_size <= FIXTURE_FILE_LIMIT, "fixture file exceeds per-file limit")
                total += item.st_size
            require(count <= 512 and total <= FIXTURE_BUDGET,
                    "synthetic SQLite/file fixture storage budget exceeded")
    verify_fixture_identity()
    space = os.fstatvfs(FIXTURE_FD)
    require(space.f_bavail * space.f_frsize >= 16 * 1024 * 1024,
            "runtime tmpfs low space")


def remove_owned_fixture():
    """Remove only the verified private scope after owned processes have exited."""
    global FIXTURE, FIXTURE_FD, FIXTURE_NAME, TMPFS_PARENT_FD
    try:
        verify_fixture_identity()
        # Descriptor-relative deletion supports the runner's Python 3.10 too;
        # shutil.rmtree(dir_fd=...) is only available in newer Python versions.
        def remove_children(directory_fd):
            with os.scandir(directory_fd) as entries:
                names = []
                for entry in entries:
                    guard_fixture_deadline()
                    names.append(entry.name)
            for name in names:
                guard_fixture_deadline()
                before = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                if stat.S_ISDIR(before.st_mode):
                    child_fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                       dir_fd=directory_fd)
                    try:
                        held = os.fstat(child_fd)
                        require((held.st_dev, held.st_ino) == (before.st_dev, before.st_ino),
                                "owned fixture child changed before cleanup")
                        remove_children(child_fd)
                        after = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                        require((after.st_dev, after.st_ino) == (held.st_dev, held.st_ino),
                                "owned fixture child replaced during cleanup")
                        os.rmdir(name, dir_fd=directory_fd)
                    finally:
                        os.close(child_fd)
                else:
                    # A symlink is unlinked, never followed to its target.
                    os.unlink(name, dir_fd=directory_fd)
        remove_children(FIXTURE_FD)
        verify_fixture_identity()
        require(not os.listdir(FIXTURE_FD), "private fixture root not empty after cleanup")
        os.rmdir(FIXTURE_NAME, dir_fd=TMPFS_PARENT_FD)
        FIXTURE = FIXTURE_NAME = None
    finally:
        root_fd, parent_fd = FIXTURE_FD, TMPFS_PARENT_FD
        FIXTURE_FD = TMPFS_PARENT_FD = None
        try:
            if root_fd is not None:
                os.close(root_fd)
        finally:
            if parent_fd is not None:
                os.close(parent_fd)


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
    """Hash every tracked source/build input, including every Cargo.lock and the package-manager lock."""
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
                        "archive/source download forbidden before immutable source seal")
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
    require(manifest["schema_version"] == 2
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
            and len(CANDIDATE_PATHS) == 59 and manifest["mutable_source_paths"] == []
            and manifest["counts"] == EXPECTED_COUNTS
            and manifest["desktop_selected"] == DESKTOP_SELECTED,
            "reviewed immutable scope differs")
    require(manifest["stage2_required"] == ["pnpm test", "pnpm exec tsc --noEmit", "desktop cargo test"],
            "mandatory complete Stage 2 gates differ")
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


def source_test_declarations(text):
    # This is a fixed-source declaration guard, not a general Rust cfg parser.
    # Prefix/cfg selection is reviewed explicitly in the sealed manifest; the
    # compiled binary's --list and per-case --exact --list are authoritative.
    pattern = r"(?P<attrs>(?:^[ \t]*#\[[^\n]+\][ \t]*\n)+)[ \t]*(?:async\s+)?fn\s+(?P<name>\w+)\s*\("
    result = []
    for match in re.finditer(pattern, text, re.M):
        attrs = match.group("attrs")
        if re.search(r"#\[(?:test|tokio::test(?:\([^\n]*\))?)\]", attrs):
            result.append({"name": match.group("name"),
                           "attributes": [line.strip() for line in attrs.splitlines()]})
    return result


def verify_source_inventory(source, manifest):
    for path, expected in manifest["source_inventory"].items():
        require(source_test_declarations((source / path).read_text()) == expected["declarations"],
                "source test attributes/names differ: " + path)
    console = (source / "crates/ai-toolbox-automation/tests/owner_console.rs").read_text()
    require('const CHILD_LIMIT: Duration = Duration::from_secs(13);' in console
            and 'const CLEANUP_LIMIT: Duration = Duration::from_secs(2);' in console,
            "owner console child/cleanup limits differ")
    write_json("validation-source-inventories.json", {
        "inventories": manifest["inventories"], "ignored": manifest["ignored"],
        "source_declarations": manifest["source_inventory"],
        "desktop_selected": DESKTOP_SELECTED,
        "method": "Reviewed module prefixes and Linux cfg; compiled list is authoritative; tokio tests included"})


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


def seal_immutable_source(source, manifest, before, env):
    global STAGE
    STAGE = "seal_immutable_source"
    require(not run("sealed-working-diff", ["git", "diff", "--name-only"], 15, source, env).strip(),
            "source changed before compilation")
    for path, checksum in manifest["immutable_locks"].items():
        require(file_digest(source / path) == checksum, "immutable lock differs: " + path)
    write_json("validation-tested-all-filehashes.json", before)
    write_json("validation-tested-source.json", {
        "base": BASE, "input_tree": INPUT_TREE, "tested_tree": INPUT_TREE,
        "input_patch_sha256": PATCH_SHA256, "tested_patch_sha256": PATCH_SHA256,
        "mutable_output_paths": [], "actual_changed_input_bytes": [],
        "immutable_locks": manifest["immutable_locks"], "tested_file_count": len(before),
        "compiled": False, "lock_resolution_attempted": False, "formatting_attempted": False,
        "actual_locked_compile_rejection_requires_stop": True,
        "stage2_required": manifest["stage2_required"]})
    return before, INPUT_TREE, PATCH_SHA256


def create_runtime_fixture(source, env):
    global FIXTURE, FIXTURE_ID, TMPFS_PARENT, TMPFS_ID
    global FIXTURE_FD, FIXTURE_NAME, TMPFS_PARENT_FD, STAGE
    STAGE = "runtime_fixture"
    parent = Path("/dev/shm")
    require(parent.is_dir() and not parent.is_symlink(), "existing tmpfs unavailable")
    magic = run("runtime-filesystem", ["stat", "-f", "-c", "%t", str(parent)], 5, source, env).decode().strip()
    require(magic == "1021994", "runtime root must be TMPFS_MAGIC, never inferred from overlay")
    parent_fd = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        parent_info = os.fstat(parent_fd)
        named_parent = parent.lstat()
        require((parent_info.st_dev, parent_info.st_ino) == (named_parent.st_dev, named_parent.st_ino),
                "tmpfs parent changed before private root creation")
        space = os.fstatvfs(parent_fd)
        require(space.f_bavail * space.f_frsize >= FIXTURE_BUDGET + 16 * 1024**2,
                "runtime tmpfs space insufficient")
        # mkdirat is exclusive creation: collisions are retried without reading
        # or deleting the existing object. Never enumerate this shared parent.
        name = None
        for _attempt in range(8):
            guard_fixture_deadline()
            candidate = "a" + os.urandom(4).hex()[:7]
            try:
                os.mkdir(candidate, mode=0o700, dir_fd=parent_fd)
                name = candidate
                break
            except FileExistsError:
                continue
        require(name is not None, "cannot exclusively create a private short fixture root")
        # Keep the newly created path as unconfirmed evidence even if opening
        # or validating it fails. Finalization must not report cleanup success.
        TMPFS_PARENT, TMPFS_PARENT_FD = parent, parent_fd
        TMPFS_ID = (parent_info.st_dev, parent_info.st_ino)
        FIXTURE, FIXTURE_NAME = parent / name, name
        root_fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        try:
            identity = os.fstat(root_fd)
            named = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
            require((identity.st_dev, identity.st_ino) == (named.st_dev, named.st_ino)
                    and identity.st_uid == os.geteuid() and stat.S_IMODE(identity.st_mode) == 0o700,
                    "private fixture ownership/mode differs")
            FIXTURE_FD = root_fd
            FIXTURE_ID = (identity.st_dev, identity.st_ino)
            verify_fixture_identity()
        except BaseException:
            os.close(root_fd)
            if FIXTURE_FD == root_fd:
                FIXTURE_FD = None
            raise  # Retain any unconfirmed new directory; never guess ownership.
    except BaseException:
        os.close(parent_fd)
        if TMPFS_PARENT_FD == parent_fd:
            TMPFS_PARENT_FD = None
        raise
    write_json("validation-runtime.json", {
        "root": str(FIXTURE), "magic": magic, "mode": "0700", "identity": list(FIXTURE_ID),
        "sampled_aggregate_bytes": FIXTURE_BUDGET, "hard_per_file_bytes": FIXTURE_FILE_LIMIT,
        "entry_limit": 512, "no_mount_or_existing_directory_mode_change": True,
        "fixture_scope": "Exclusive short private root; held parent/root descriptors and inode checks",
        "shared_tmpfs_objects": "Not enumerated, opened, read, or removed",
        "cleanup_scope": "Only the verified private root, after all owned processes exit",
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
    listed = inventory(run(group.replace("_", "-") + "-list",
        TEST_LAUNCH + [str(binary), "--list", "--test-threads=1"], 15, source, test_env))
    expected = manifest["inventories"][group]
    if group in manifest["partial_inventory_groups"]:
        require(set(expected) <= set(listed), "selected desktop inventory absent: " + group)
    else:
        require(listed == expected, "exact compiled inventory differs: " + group)
    ignored = inventory(run(group.replace("_", "-") + "-ignored-list",
        TEST_LAUNCH + [str(binary), "--ignored", "--list", "--test-threads=1"], 15, source, test_env))
    require(sorted(set(ignored) & set(expected)) == manifest["ignored"].get(group, []),
            "unexpected selected ignored compiled test")
    if group not in manifest["partial_inventory_groups"]:
        require(ignored == manifest["ignored"].get(group, []), "unexpected ignored compiled inventory")
    write_json("validation-" + group.replace("_", "-") + "-inventory.json", {
        "names": listed, "ignored": ignored, "expected": expected,
        "complete_inventory_match": group not in manifest["partial_inventory_groups"],
        "selected_subset_match": group in manifest["partial_inventory_groups"]})
    return listed


def require_fixture_empty(label):
    check_fixture_budget(force=True)
    require(not os.listdir(FIXTURE_FD), "test case leaked owned fixture objects: " + label)


def execute_exact(group, binary, names, listed, manifest, source, test_env, result):
    global STAGE
    ignored = set(manifest["ignored"].get(group, []))
    for index, name in enumerate(names):
        require(name not in ignored, "actor entry must never count as an ordinary test")
        label = group.replace("_", "-") + f"-case-{index + 1}"
        STAGE = "inventory_guard"
        exact = inventory(run(label + "-list", TEST_LAUNCH + [str(binary), "--exact", name, "--list"],
                              10, source, test_env))
        require(exact == [name], "exact selected case absent: " + name)
        STAGE = "tests"
        slow = (group in {"owner", "owner_console", "desktop_coding", "desktop_deeplink"}
                or name.startswith(("owner::profile_lease_tests::", "context_session::tests::",
                                    "transport::linux::context_tests::"))
                or "blocking" in name or "worker" in name)
        integration_parent = group + ":" + name in manifest["integration_child_parents"]
        output = run(label + "-test", TEST_LAUNCH + [str(binary), "--exact", name,
                     "--test-threads=1", "--nocapture" if integration_parent else "--show-output"],
                     45 if slow else 30, source, test_env)
        STAGE = "test_result_guard"
        check_pass(output, 1, len(listed) - 1)
        if integration_parent:
            stderr = (EVIDENCE / RECORDS[-1]["logs"]["stderr"]).read_text()
            markers = [line for line in stderr.splitlines() if line.startswith("ordinary integration child: ")]
            require(markers == [f"ordinary integration child: {name}; 1 passed; 0 failed; 0 ignored; reaped"],
                    "bounded ordinary integration child proof absent or duplicated")
            result["ordinary_integration_children"].append({"parent": group + ":" + name,
                "normal_child_executions": 1,
                "evidence": "Observed exact stderr marker; fixed parent asserts successful child summary, marker, and reaping"})
        actor = manifest["actor_parents"].get(group + ":" + name)
        if actor:
            if group == "owner":
                text = output.decode()
                require(len(re.findall(r"^verified actor execution: ", text, re.M)) == actor["normal"],
                        "normal owner actor completions differ")
                require(len(re.findall(r"^verified abrupt actor execution: ", text, re.M)) == actor["abrupt"],
                        "abrupt owner actor completions differ")
            result["actor_parent_results"].append({"parent": group + ":" + name, **actor,
                "evidence": "Fixed parent asserts bounded wait/reap, successful child libtest summary and marker; owner also emits verified markers"})
        STAGE = "fixture_guard"
        require_fixture_empty(label)
        result["passed_cases"].setdefault(group, []).append(name)
        write_json("validation-progress.json", result)


def validate(checkout, manifest, manifest_path):
    global RUN_ROOT, CACHE_ROOT, TARGET_ROOT, ARCHIVES_ALLOWED, STAGE, FINALIZING, FIXTURE
    patch = source_check(checkout, manifest, manifest_path)
    RUN_ROOT = Path(os.environ["RUNNER_TEMP"]).resolve() / (
        "typed-context-admission-" + os.environ["GITHUB_RUN_ID"] + "-" + os.environ["GITHUB_RUN_ATTEMPT"])
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
    result = {"ok": False, "product_ready": False, "error": None, "passed_cases": {},
              "actor_parent_results": [], "ordinary_integration_children": [], "source_or_lock_mutation_allowed": False,
              "scope": "Finite Stage 1 exact core and selected desktop admission regressions only",
              "expected_counts": EXPECTED_COUNTS, "stage2_required": manifest["stage2_required"]}
    try:
        STAGE = "toolchain_guard"
        require(run("rust-version", ["rustc", "--version"], 10, checkout, env).decode().startswith("rustc 1.99.1 "),
                "unexpected rustc version")
        require(run("cargo-version", ["cargo", "--version"], 10, checkout, env).decode().startswith("cargo 1.99.1 "),
                "unexpected cargo version")
        source, before = prepare_source(checkout, manifest, patch, env)
        tested, tested_tree, tested_patch_sha = seal_immutable_source(source, manifest, before, env)
        shutil.copyfile(patch, EVIDENCE / "validation-tested.patch")
        result.update(tested_tree=tested_tree, tested_patch_sha256=tested_patch_sha)
        check_disk_budgets(force=True)
        ARCHIVES_ALLOWED = True
        # No update, resolve, metadata rewrite, or format command exists here.
        # Source/lock bytes are sealed; actual --locked compilation must accept.
        core_specs = [
            ("codex-config", "crates/ai-toolbox-codex-config/Cargo.toml", ["--lib"],
             {"codex_config_lib": "ai_toolbox_codex_config"}),
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
            verify_hashes(source, manifest)
        test_env = create_runtime_fixture(source, env)
        for group, binary in bins.items():
            listed = check_binary_inventory(group, binary, manifest, source, test_env)
            names = [name for name in listed if name not in manifest["ignored"].get(group, [])]
            execute_exact(group, binary, names, listed, manifest, source, test_env, result)
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
            ["--lib", "--test", "sqlite_jsonb", "--test", "storage_reexports", "--test", "management_admission",
             "--test", "coding", "--test", "deeplink"],
            {"desktop_lib": "ai_toolbox_lib", "desktop_sqlite": "sqlite_jsonb",
             "desktop_reexports": "storage_reexports", "desktop_admission": "management_admission",
             "desktop_coding": "coding", "desktop_deeplink": "deeplink"},
            source, env, 900)
        verify_hashes(source, manifest)
        for group, selected in DESKTOP_SELECTED.items():
            listed = check_binary_inventory(group, desktop[group], manifest, source, test_env)
            execute_exact(group, desktop[group], selected, listed, manifest, source, test_env, result)
        ordinary = sum(len(names) for names in result["passed_cases"].values())
        require(ordinary == EXPECTED_COUNTS["ordinary_total"], "ordinary selected total differs")
        actors = {kind: sum(item[kind] for item in result["actor_parent_results"]) for kind in ["normal", "abrupt"]}
        require(actors == {"normal": EXPECTED_COUNTS["normal_actor_passes"],
                           "abrupt": EXPECTED_COUNTS["abrupt_actor_exits"]}, "actor totals differ")
        result["actors"] = {**actors, "ignored_entry_not_counted_as_pass": EXPECTED_COUNTS["ignored_actor_entries"]}
        require(len(result["ordinary_integration_children"]) == EXPECTED_COUNTS["ordinary_positive_integration_child_executions"],
                "ordinary positive integration child count differs")
        result["ok"] = True
        result["outcome"] = "stage1_passed_stage2_required_not_product_ready"
    except BaseException as error:
        result["error"] = f"{type(error).__name__}: {error}"
        result["failed_stage"] = STAGE
        # Only a concrete failed libtest summary is a product test failure.
        # Deadline-before-launch, signals, metadata guards and uncertain cleanup
        # are incomplete/guard outcomes, not evidence of failed assertions.
        confirmed_test_failure = False
        if STAGE == "tests" and RECORDS:
            last = RECORDS[-1]
            if last["label"].endswith("-test") and last["reason"] == "nonzero_exit":
                logged = (EVIDENCE / last["logs"]["stdout"]).read_text()
                diagnostic = logged + (EVIDENCE / last["logs"]["stderr"]).read_text()
                # A bounded fixture's own timeout/output guard panics inside
                # libtest too. Its FAILED summary is not a completed business
                # assertion; preserve this distinction for the frozen helper.
                fixture_incomplete = any(marker in diagnostic for marker in (
                    "ordinary fixture timed out:", "ordinary fixture output exceeded bound:",
                    "ordinary fixture child exit unconfirmed", "fixture log limit",
                    "admission actor deadline exceeded; retain fixture",
                    "isolated GUI admission actor was not reaped; fixture retained"))
                confirmed_test_failure = (not fixture_incomplete
                    and bool(re.search(r"^test result: FAILED\.", logged, re.M)))
        result["outcome"] = ("test_failure" if confirmed_test_failure else
                             "blocked_setup_or_compile_not_product_red" if STAGE in {"compile", "toolchain_guard"}
                             else "validation_inconclusive_or_guard_failure_not_product_red")
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
                remove_owned_fixture()
            result["fixture_cleanup_scope"] = "Verified private root only; shared tmpfs parent never enumerated"
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
                    after[path] == tested[path] for path in manifest["immutable_locks"])
            check_disk_budgets(force=True)
        except BaseException as error:
            errors.append("source/disk proof: " + str(error))
        # Finalization runs more supervised commands; include them in the final
        # ownership proof instead of retaining the earlier pre-cleanup value.
        all_exited_final = ACTIVE is None and all(not record["survivors"] for record in RECORDS)
        result["all_owned_processes_exited"] = all_exited_final
        if not all_exited_final:
            errors.append("final ownership scan: unconfirmed subprocess cleanup")
        if errors:
            result["ok"] = False
            result["outcome"] = "finalization_inconclusive_not_product_green"
        result.update(finalization_errors=errors, cache=CACHE_OBSERVATION,
                      command_count=len(RECORDS), commands=RECORDS)
        write_json("validation-result.json", result)
    return result


def main():
    global EVIDENCE, PHASE, JOB_DEADLINE, CANDIDATE_PATHS, EXPECTED_COUNTS, DESKTOP_SELECTED
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=["preflight", "validation"], required=True)
    options = parser.parse_args()
    PHASE = options.phase
    require(sys.platform == "linux" and os.geteuid() != 0, "unprivileged Linux required")
    checkout = Path(os.environ["GITHUB_WORKSPACE"]).resolve()
    runner_temp = Path(os.environ["RUNNER_TEMP"]).resolve()
    require(runner_temp != Path("/tmp") and not runner_temp.is_relative_to(Path("/dev/shm")),
            "source/build/evidence must use runner disk, never tmpfs")
    EVIDENCE = runner_temp / "typed-context-admission-evidence"
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
        require(file_digest(manifest_path) == MANIFEST_SHA256, "sealed manifest bytes differ")
        manifest = json.loads(manifest_path.read_text())
        CANDIDATE_PATHS = set(manifest["candidate_files"])
        EXPECTED_COUNTS = manifest["counts"]
        DESKTOP_SELECTED = manifest["desktop_selected"]
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
