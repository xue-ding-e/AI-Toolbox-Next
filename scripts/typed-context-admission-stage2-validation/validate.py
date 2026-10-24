#!/usr/bin/env python3
"""Prepared short-browser owned-scope diagnostic, not dispatch authorization.

The exact Stage 1 run4 source/locks remain immutable. This sealed CLI mode runs
no long probe, package install, compiler or full suite. Full-suite code remains
for later review, unreachable from this mode. Process/scan policy stays strict.
"""
import argparse
import ctypes
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

BASE = '23b35d815cc5899d0d4f70d872a403ca5fe68f57'
BASE_TREE = 'c74f21f8989d2ba2a093e704be808851a0fa63e7'
INPUT_TREE = 'e32bd235d1d505c9940e16e81fd92f761debde32'
MANIFEST_SHA256 = '19b986c051fc5ed92884d88f34172dc884419bcc3a25a8760531d6b1ab76f92b'
PATCH_SHA256 = 'e58ce582479149f958c2d125ebc158222da49e39a29196428ec6a8e3ef2ff24c'
REPO = "xue-ding-e/AI-Toolbox-Next"
BRANCH = "refs/heads/validation/typed-context-admission-stage2"
HARNESS_DIR = "scripts/typed-context-admission-stage2-validation"
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
FINALIZE_RESERVE = 120
FIXTURE = None
FIXTURE_ID = None
LAST_SCAN = 0
FIXTURE_BUDGET = 2 * 1024**3
FIXTURE_FILE_LIMIT = 512 * 1024**2
CACHE_ROOT = None
TARGET_ROOT = None
RUN_ROOT = None
SOURCE_ROOT = None
NODE_STORE = None
FRONTEND_TMP = None
FRONTEND_FD = FRONTEND_PARENT_FD = None
FRONTEND_ID = FRONTEND_PARENT_ID = None
FRONTEND_PARENT = FRONTEND_NAME = None
NODE_INSTALLING = False
FIXTURE_OBSERVATION = {}
MEMORY_OBSERVATION = {"owned_sum_rss_peak_bytes": 0, "owned_process_count_peak": 0, "sampled_not_a_hard_memory_limit": True, "summed_rss_can_double_count_shared_pages": True}
CACHE_CHECK_AT = 0
CACHE_OBSERVATION = {}
SCAN_REJECTIONS = set()
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


def drain_exception_pipes(record, selector, files):
    """Preserve already-emitted owned pipe bytes after confirmed process cleanup."""
    global TOTAL_LOG_BYTES
    deadline = min(time.monotonic() + 0.25, JOB_DEADLINE - 10) if JOB_DEADLINE is not None else time.monotonic() + 0.25
    while selector.get_map() and time.monotonic() < deadline:
        ready = selector.select(timeout=0)
        if not ready:
            break  # Nonblocking only; never wait on a possibly inherited pipe.
        for key, _mask in ready:
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
            if len(data) > remaining:
                record["exception_pipe_log_limit_hit"] = True
                return
    record["exception_pipe_drain_complete"] = not selector.get_map()


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
            observe_memory(members)
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
            if reason == "exception" and child is not None and not record["survivors"]:
                try:
                    drain_exception_pipes(record, selector, files)
                except (OSError, RuntimeError) as drain_error:
                    record["exception_pipe_drain_error"] = type(drain_error).__name__
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


def inventory(output):
    text = output.decode()
    names = sorted(line[:-6] for line in text.splitlines() if line.endswith(": test"))
    require(len(names) == len(set(names)), "duplicate test inventory")
    require(not any(line.endswith(": benchmark") for line in text.splitlines()), "unexpected benchmark inventory")
    return names


def guard_fixture_deadline():
    reserve = 10 if FINALIZING else FINALIZE_RESERVE
    require(JOB_DEADLINE is None or time.monotonic() < JOB_DEADLINE - reserve,
            "shared deadline reached during private fixture operation")


def filesystem_magic(directory_fd):
    # Linux statfs is at most 120 bytes on the selected x86_64 runner. Read the
    # native unsigned f_type from the held FD, never infer backing from overlay.
    buffer = ctypes.create_string_buffer(256)
    libc = ctypes.CDLL(None, use_errno=True)
    libc.fstatfs.argtypes = [ctypes.c_int, ctypes.c_void_p]
    libc.fstatfs.restype = ctypes.c_int
    require(libc.fstatfs(directory_fd, buffer) == 0, "held directory statfs failed")
    return ctypes.c_ulong.from_buffer(buffer).value


def check_safe_ancestors(path):
    require(path.is_absolute() and path.resolve(strict=True) == path,
            "private scratch parent must be absolute and canonical")
    for ancestor in (path, *path.parents):
        item = ancestor.lstat()
        require(stat.S_ISDIR(item.st_mode) and item.st_uid in {0, os.geteuid()},
                "unsafe scratch ancestor identity")
        require(not item.st_mode & 0o022 or (item.st_uid == 0 and item.st_mode & 0o1000),
                "unsafe writable scratch ancestor")


def observe_memory(members):
    total = count = 0
    for pid, identity in members.items():
        try:
            directory = Path('/proc') / str(pid)
            def start():
                raw = (directory / 'stat').read_text()
                return int(raw[raw.rindex(')') + 2:].split()[19])
            if start() != identity['start']:
                continue
            rss = int((directory / 'statm').read_text().split()[1]) * os.sysconf('SC_PAGE_SIZE')
            if start() == identity['start']:
                total += rss
                count += 1
        except (FileNotFoundError, ProcessLookupError):
            continue
    MEMORY_OBSERVATION['owned_sum_rss_peak_bytes'] = max(total, MEMORY_OBSERVATION['owned_sum_rss_peak_bytes'])
    MEMORY_OBSERVATION['owned_process_count_peak'] = max(count, MEMORY_OBSERVATION['owned_process_count_peak'])


def verify_fixture_identity():
    """Bind all fixture operations to held directory descriptors, never a prefix."""
    guard_fixture_deadline()
    require(FIXTURE_FD is not None and TMPFS_PARENT_FD is not None,
            "private fixture descriptors unavailable")
    parent = os.fstat(TMPFS_PARENT_FD)
    parent_named = TMPFS_PARENT.lstat()
    require(filesystem_magic(TMPFS_PARENT_FD) in {0xef53, 0x58465342, 0x9123683e},
            "private scratch parent is not an approved real disk filesystem")
    check_safe_ancestors(TMPFS_PARENT)
    require(stat.S_ISDIR(parent_named.st_mode)
            and (parent.st_dev, parent.st_ino) == TMPFS_ID
            and (parent_named.st_dev, parent_named.st_ino) == TMPFS_ID,
            "scratch parent identity changed")
    held = os.fstat(FIXTURE_FD)
    named = os.stat(FIXTURE_NAME, dir_fd=TMPFS_PARENT_FD, follow_symlinks=False)
    require(stat.S_ISDIR(held.st_mode) and stat.S_ISDIR(named.st_mode)
            and (held.st_dev, held.st_ino) == FIXTURE_ID
            and (named.st_dev, named.st_ino) == FIXTURE_ID
            and held.st_uid == named.st_uid == os.geteuid()
            and stat.S_IMODE(held.st_mode) == stat.S_IMODE(named.st_mode) == 0o700,
            "private fixture root identity/ownership/mode changed")


def check_fixture_budget(force=False):
    global LAST_SCAN, FIXTURE_OBSERVATION
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
    # of shared /tmp, no path-prefix ownership inference, and no symlink traversal.
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
            require(count <= 16384 and total <= FIXTURE_BUDGET,
                    "synthetic SQLite/file fixture storage budget exceeded")
    verify_fixture_identity()
    space = os.fstatvfs(FIXTURE_FD)
    require(space.f_bavail * space.f_frsize >= 4 * 1024**3, "runtime disk free-space reserve reached")
    FIXTURE_OBSERVATION = {"bytes": total, "entries": count,
        "peak_sampled_bytes": max(total, FIXTURE_OBSERVATION.get("peak_sampled_bytes", 0)),
        "sampled_not_a_filesystem_quota": True}


def remove_owned_children(directory_fd):
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
                remove_owned_children(child_fd)
                after = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                require((after.st_dev, after.st_ino) == (held.st_dev, held.st_ino),
                        "owned fixture child replaced during cleanup")
                os.rmdir(name, dir_fd=directory_fd)
            finally:
                os.close(child_fd)
        else:
            # A symlink is unlinked, never followed to its target.
            os.unlink(name, dir_fd=directory_fd)


def remove_owned_fixture():
    """Remove only the verified private scope after owned processes have exited."""
    global FIXTURE, FIXTURE_FD, FIXTURE_NAME, TMPFS_PARENT_FD
    try:
        verify_fixture_identity()
        # Descriptor-relative deletion supports the runner's Python 3.10 too;
        # shutil.rmtree(dir_fd=...) is only available in newer Python versions.
        remove_owned_children(FIXTURE_FD)
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


def allowed_owned_link(path, explicit):
    if path in explicit:
        return True
    # Chrome profile lock/cookie/socket links are metadata only. Never resolve
    # their targets; the profile itself is not uploaded and cleanup is no-follow.
    if FRONTEND_TMP is not None and path.is_relative_to(FRONTEND_TMP):
        return True
    if SOURCE_ROOT is not None and path.is_relative_to(SOURCE_ROOT / "node_modules"):
        resolved = path.resolve(strict=not NODE_INSTALLING)
        return resolved.is_relative_to(SOURCE_ROOT / "node_modules") or resolved.is_relative_to(NODE_STORE)
    return False


def allowed_fontconfig_compat_link(root, relative, info, current_fd, name):
    """Metadata-only exception for observed fontconfig cache 9/10/11 -> 12.

    O_PATH pins the target inode without reading cache contents. Named identities
    are rechecked while that FD is held. Concurrent same-UID mutation during or
    after observation (including directory rename) is outside this non-adversarial
    scanner contract. Sampled budgets are not quotas; later scans recheck.
    """
    if (RUN_ROOT is None or root != RUN_ROOT
            or relative.parent != Path("environment/XDG_CACHE_HOME/fontconfig")
            or not stat.S_ISLNK(info.st_mode) or info.st_uid != os.geteuid()):
        return False
    source = re.fullmatch(r"([0-9a-f]{32})-(le64)\.cache-(9|10|11)", name)
    if not source:
        return False
    target = source[1] + "-" + source[2] + ".cache-12"
    target_fd = None
    try:
        directory = os.fstat(current_fd)
        if (not stat.S_ISDIR(directory.st_mode) or directory.st_uid != os.geteuid()
                or stat.S_IMODE(directory.st_mode) & 0o022
                or os.readlink(name, dir_fd=current_fd) != target):
            return False
        # A single exact basename with no slash, held relative to fwalk's FD.
        # O_NOFOLLOW + O_PATH also lets us reject a target that is itself a link.
        target_fd = os.open(target, os.O_PATH | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=current_fd)
        held_target = os.fstat(target_fd)
        if (not stat.S_ISREG(held_target.st_mode) or held_target.st_uid != os.geteuid()
                or stat.S_IMODE(held_target.st_mode) & 0o022
                or not 0 < held_target.st_size <= 16 * 1024 * 1024):
            return False
        named_target = os.stat(target, dir_fd=current_fd, follow_symlinks=False)
        named_link = os.stat(name, dir_fd=current_fd, follow_symlinks=False)
        def identity(value):
            return (value.st_dev, value.st_ino, value.st_mode, value.st_uid,
                    value.st_size, value.st_mtime_ns, value.st_ctime_ns)
        return (identity(named_target) == identity(held_target)
                and identity(os.fstat(target_fd)) == identity(held_target)
                and identity(named_link) == identity(info)
                and os.readlink(name, dir_fd=current_fd) == target)
    except OSError:
        return False
    finally:
        if target_fd is not None:
            os.close(target_fd)


def bounded_cache_link_metadata(root, relative, target):
    # Diagnostic only, never an allowance. Expose only a fixed cache basename
    # in this run's exact fontconfig cache directory; never resolve its target.
    result = {"target_text_length": len(target), "raw_target_omitted": True}
    cache_name = r"[0-9a-f]{32}-le64\.cache-(?:9|10|11|12)"
    if (RUN_ROOT is not None and root == RUN_ROOT
            and relative.parent == Path("environment/XDG_CACHE_HOME/fontconfig")
            and re.fullmatch(cache_name, relative.name)
            and re.fullmatch(cache_name, target)):
        result.update(cache_basename_text=target, raw_target_omitted=False)
    return result


def record_scan_rejection(root, relative, info, held, current_fd, name):
    """One metadata record per runtime/finalization; never open a link target."""
    phase = "finalization" if FINALIZING else "runtime"
    if phase in SCAN_REJECTIONS:
        return
    SCAN_REJECTIONS.add(phase)
    scopes = [("frontend_tmp", FRONTEND_TMP), ("rust_scratch", FIXTURE),
              ("cargo_cache", CACHE_ROOT), ("rust_target", TARGET_ROOT),
              ("pnpm_store", NODE_STORE), ("npm_cache", None if RUN_ROOT is None else RUN_ROOT / "npm-cache"),
              ("node_modules", None if SOURCE_ROOT is None else SOURCE_ROOT / "node_modules"),
              ("run_root", RUN_ROOT), ("evidence", EVIDENCE)]
    types = {stat.S_IFDIR: "directory", stat.S_IFREG: "regular", stat.S_IFLNK: "symlink",
             stat.S_IFSOCK: "socket", stat.S_IFIFO: "fifo", stat.S_IFCHR: "char_device", stat.S_IFBLK: "block_device"}
    value = {"phase": phase, "stage": STAGE, "active_helper_pid": ACTIVE,
             "observed_monotonic_ns": time.monotonic_ns(),
             "scan_scope": next((label for label, path in scopes if path == root), "unclassified_owned_root"),
             "relative_path": str(relative)[:2048], "path_metadata_truncated": len(str(relative)) > 2048,
             "type": types.get(stat.S_IFMT(info.st_mode), "other"), "mode": oct(stat.S_IMODE(info.st_mode)),
             "uid": info.st_uid, "device": info.st_dev, "inode": info.st_ino,
             "held_scan_root_identity": [held.st_dev, held.st_ino],
             "decision": "rejected_by_unchanged_object_policy", "target_content_read": False}
    if stat.S_ISLNK(info.st_mode):
        try:
            # readlink reads only the link metadata. No resolve/stat/open of its
            # target or shared-directory traversal. Only bounded cache basenames
            # may be recorded; other link text remains omitted.
            target = os.readlink(name, dir_fd=current_fd)
            after = os.stat(name, dir_fd=current_fd, follow_symlinks=False)
            require((after.st_dev, after.st_ino, after.st_mode) == (info.st_dev, info.st_ino, info.st_mode),
                    "link identity changed during metadata observation")
            lexical = Path(os.path.normpath(str((root / relative).parent / target)))
            match = next(((label, path) for label, path in scopes
                          if path is not None and lexical.is_relative_to(path)), None)
            value["link"] = {"absolute_text": os.path.isabs(target),
                "lexical_scope_only": match[0] if match else "outside_known_owned_scopes",
                "resolved_ownership": "not_checked_no_target_follow",
                **bounded_cache_link_metadata(root, relative, target)}
            if name == "SingletonLock":
                hint = re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9.-]{0,252}-([1-9][0-9]{0,9})", target)
                if hint:
                    value["link"]["singleton_pid_hint_unverified"] = int(hint[1])
        except (OSError, RuntimeError):
            value["link"] = {"metadata_unavailable_or_identity_changed": True, "raw_target_omitted": True}
    write_json("validation-scan-rejection-" + phase + ".json", value)


def scan_disk(root, byte_limit, entry_limit, index_only=False, allowed_symlinks=()):
    total = count = 0
    try:
        root_info = root.lstat()
    except FileNotFoundError:
        return {"bytes": 0, "entries": 0}
    require(stat.S_ISDIR(root_info.st_mode) and root.resolve(strict=True) == root,
            "owned scan root or ancestor replaced by symlink")
    def scan_error(error):
        if not isinstance(error, FileNotFoundError):
            raise error
    root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        held = os.fstat(root_fd)
        require((held.st_dev, held.st_ino) == (root_info.st_dev, root_info.st_ino), "owned scan root changed")
        for current, directories, files, current_fd in os.fwalk(".", follow_symlinks=False, onerror=scan_error, dir_fd=root_fd):
            guard_fixture_deadline()
            for name in directories + files:
                path = root / current / name
                try:
                    info = os.stat(name, dir_fd=current_fd, follow_symlinks=False)
                except FileNotFoundError:
                    continue
                guard_fixture_deadline()
                frontend_object = FRONTEND_TMP is not None and path.is_relative_to(FRONTEND_TMP)
                admitted = (stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode)
                        or (stat.S_ISLNK(info.st_mode) and (allowed_owned_link(path, allowed_symlinks)
                            or allowed_fontconfig_compat_link(root, Path(current) / name, info, current_fd, name)))
                        or (frontend_object and (stat.S_ISSOCK(info.st_mode) or stat.S_ISFIFO(info.st_mode))))
                if not admitted:
                    try:
                        record_scan_rejection(root, Path(current) / name, info, held, current_fd, name)
                    except (OSError, RuntimeError):
                        pass  # Diagnostic failure must not weaken or replace the original rejection.
                require(admitted, "unexpected object outside reviewed owned link/temporary scopes")
                if index_only:
                    require(not path.is_relative_to(root / "registry/cache")
                            and not path.is_relative_to(root / "registry/src"),
                            "archive/source download forbidden before immutable source seal")
                count += 1
                total += info.st_size if stat.S_ISREG(info.st_mode) else 0
                require(total <= byte_limit and count <= entry_limit, "disk/cache budget exceeded")
        named = root.lstat()
        require((named.st_dev, named.st_ino) == (held.st_dev, held.st_ino)
                and stat.S_ISDIR(named.st_mode), "owned scan root replaced during scan")
    finally:
        os.close(root_fd)
    return {"bytes": total, "entries": count}


def check_disk_budgets(force=False):
    global CACHE_CHECK_AT, CACHE_OBSERVATION
    if CACHE_ROOT is None or (not force and time.monotonic() < CACHE_CHECK_AT):
        return
    CACHE_CHECK_AT = time.monotonic() + (5 if ARCHIVES_ALLOWED else 0.5)
    cache = scan_disk(CACHE_ROOT, 2 * 1024**3 if ARCHIVES_ALLOWED else 160 * 1024**2,
                      200000 if ARCHIVES_ALLOWED else 30000, not ARCHIVES_ALLOWED)
    target = scan_disk(TARGET_ROOT, 12 * 1024**3, 200000)
    node = {"bytes": 0, "entries": 0}
    for directory in [NODE_STORE, RUN_ROOT / "npm-cache", None if SOURCE_ROOT is None else SOURCE_ROOT / "node_modules"]:
        if directory is not None and directory.exists():
            value = scan_disk(directory, 4 * 1024**3, 250000)
            for key in node: node[key] += value[key]
    require(node["bytes"] <= 4 * 1024**3 and node["entries"] <= 250000, "combined Node/store cap exceeded")
    if FRONTEND_TMP is not None:
        verify_frontend_identity()
    frontend = scan_disk(FRONTEND_TMP, 4 * 1024**3, 100000) if FRONTEND_TMP is not None else {"bytes": 0, "entries": 0}
    combined = scan_disk(RUN_ROOT, 24 * 1024**3, 750000, allowed_symlinks={
        RUN_ROOT / "source/.claude/skills", RUN_ROOT / "source/public/tray-icon.png"})
    evidence = scan_disk(EVIDENCE, 256 * 1024**2, 50000)
    scratch = FIXTURE_OBSERVATION.get("bytes", 0) if FIXTURE is not None else 0
    # The original long diagnostic scope is inside RUN_ROOT; the independent
    # short frontend scope is not. Count exactly once, never omit the latter.
    frontend_outside = frontend["bytes"] if FRONTEND_TMP is not None and not FRONTEND_TMP.is_relative_to(RUN_ROOT) else 0
    require(combined["bytes"] + evidence["bytes"] + scratch + frontend_outside <= 24 * 1024**3,
            "whole owned source/home/cache/target/evidence/scratch disk cap exceeded")
    space = os.statvfs(RUN_ROOT)
    require(space.f_bavail * space.f_frsize >= 4 * 1024**3, "runner disk free-space reserve reached")
    for leaf in (CACHE_ROOT / "registry/index").glob("*"):
        require(leaf.name.startswith("index.crates.io-") and not leaf.is_symlink(), "unreviewed registry cache identity")
        config = leaf / "config.json"
        if config.exists():
            value = json.loads(config.read_text())
            require(value.get("dl") in {"https://static.crates.io/crates",
                                        "https://static.crates.io/crates/{crate}/{crate}-{version}.crate"}
                    and value.get("api", "https://crates.io") == "https://crates.io", "nonofficial registry URL")
    CACHE_OBSERVATION = {"cache": cache, "target": target, "node_store": node, "frontend_temp": frontend,
        "combined_run_root": combined, "evidence": evidence, "rust_scratch": FIXTURE_OBSERVATION,
        "whole_owned_disk_bytes": combined["bytes"] + evidence["bytes"] + scratch + frontend_outside,
        "frontend_outside_run_root_bytes": frontend_outside,
        "sampled_not_a_filesystem_quota": True, "source_archives_allowed": ARCHIVES_ALLOWED}


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
        "compiled": False, "snapshot_phase": "pre_compilation",
        "compilation_evidence": "Final full-cargo raw logs, compiler/actual-artifact records and validation-result, not this immutable pre-compilation snapshot",
        "lock_resolution_attempted": False, "formatting_attempted": False,
        "actual_locked_compile_rejection_requires_stop": True,
        "stage2_required": manifest["stage2_required"]})
    return before, INPUT_TREE, PATCH_SHA256


def create_runtime_fixture(source, env):
    global FIXTURE, FIXTURE_ID, TMPFS_PARENT, TMPFS_ID
    global FIXTURE_FD, FIXTURE_NAME, TMPFS_PARENT_FD, STAGE
    STAGE = "runtime_fixture"
    parent = Path("/tmp")
    require(parent.is_dir() and not parent.is_symlink(), "existing private scratch parent unavailable")
    check_safe_ancestors(parent)
    parent_fd = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        parent_info = os.fstat(parent_fd)
        magic = filesystem_magic(parent_fd)
        require(magic in {0xef53, 0x58465342, 0x9123683e},
                "short scratch requires actual ext-family/XFS/BTRFS; no overlay/tmpfs or fallback")
        named_parent = parent.lstat()
        require((parent_info.st_dev, parent_info.st_ino) == (named_parent.st_dev, named_parent.st_ino),
                "scratch parent changed before private root creation")
        space = os.fstatvfs(parent_fd)
        require(space.f_bavail * space.f_frsize >= FIXTURE_BUDGET + 4 * 1024**3,
                "runtime disk space insufficient")
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
        "root": str(FIXTURE), "magic": hex(magic), "mode": "0700", "identity": list(FIXTURE_ID),
        "sampled_aggregate_bytes": FIXTURE_BUDGET, "sampled_per_file_bytes": FIXTURE_FILE_LIMIT,
        "entry_limit": 16384, "no_mount_or_existing_directory_mode_change": True,
        "fixture_scope": "Exclusive short private root; held parent/root descriptors and inode checks",
        "shared_tmp_objects": "Not enumerated, opened, read, or removed",
        "cleanup_scope": "Only the verified private root, after all owned processes exit",
    })
    test_env = env.copy()
    test_env.update(TMPDIR=str(FIXTURE), TMP=str(FIXTURE), TEMP=str(FIXTURE), SQLITE_TMPDIR=str(FIXTURE))
    return test_env


def create_frontend_temp(short=False):
    global FRONTEND_TMP, FRONTEND_FD, FRONTEND_PARENT_FD, FRONTEND_ID, FRONTEND_PARENT_ID
    global FRONTEND_PARENT, FRONTEND_NAME
    parent_path = Path("/tmp") if short else RUN_ROOT
    check_safe_ancestors(parent_path)
    FRONTEND_PARENT_FD = os.open(parent_path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        parent = os.fstat(FRONTEND_PARENT_FD)
        named = parent_path.lstat()
        require((parent.st_dev, parent.st_ino) == (named.st_dev, named.st_ino), "frontend parent changed")
        require(filesystem_magic(FRONTEND_PARENT_FD) in {0xef53, 0x58465342, 0x9123683e},
                "frontend requires actual supported disk filesystem; no fallback")
        space = os.fstatvfs(FRONTEND_PARENT_FD)
        require(space.f_bavail * space.f_frsize >= 8 * 1024**3, "frontend budget plus free reserve unavailable")
        FRONTEND_PARENT, FRONTEND_PARENT_ID = parent_path, (parent.st_dev, parent.st_ino)
        name = None
        for _attempt in range(8 if short else 1):
            guard_fixture_deadline()
            candidate = "f" + os.urandom(4).hex()[:7] if short else "frontend-tmp"
            try:
                os.mkdir(candidate, mode=0o700, dir_fd=FRONTEND_PARENT_FD)
                name = candidate
                break
            except FileExistsError:
                continue  # Never inspect or delete a colliding shared /tmp name.
        require(name is not None, "cannot exclusively create frontend scope")
        FRONTEND_NAME, FRONTEND_TMP = name, parent_path / name
        FRONTEND_FD = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=FRONTEND_PARENT_FD)
        root = os.fstat(FRONTEND_FD)
        FRONTEND_ID = (root.st_dev, root.st_ino)
        verify_frontend_identity()
    except BaseException:
        close_frontend_fds()
        raise  # Preserve any unconfirmed newly created path, never guess ownership.


def close_frontend_fds():
    global FRONTEND_FD, FRONTEND_PARENT_FD
    root_fd, parent_fd = FRONTEND_FD, FRONTEND_PARENT_FD
    FRONTEND_FD = FRONTEND_PARENT_FD = None
    try:
        if root_fd is not None:
            os.close(root_fd)
    finally:
        if parent_fd is not None:
            os.close(parent_fd)


def verify_frontend_identity():
    guard_fixture_deadline()
    require(FRONTEND_FD is not None and FRONTEND_PARENT_FD is not None, "frontend descriptors unavailable")
    held_parent, named_parent = os.fstat(FRONTEND_PARENT_FD), FRONTEND_PARENT.lstat()
    check_safe_ancestors(FRONTEND_PARENT)
    require(filesystem_magic(FRONTEND_PARENT_FD) in {0xef53, 0x58465342, 0x9123683e}, "frontend filesystem changed")
    require((held_parent.st_dev, held_parent.st_ino) == FRONTEND_PARENT_ID
            and (named_parent.st_dev, named_parent.st_ino) == FRONTEND_PARENT_ID
            and stat.S_ISDIR(named_parent.st_mode), "frontend parent replaced")
    held = os.fstat(FRONTEND_FD)
    named = os.stat(FRONTEND_NAME, dir_fd=FRONTEND_PARENT_FD, follow_symlinks=False)
    require(stat.S_ISDIR(held.st_mode) and stat.S_ISDIR(named.st_mode)
            and (held.st_dev, held.st_ino) == FRONTEND_ID == (named.st_dev, named.st_ino)
            and held.st_uid == named.st_uid == os.geteuid()
            and stat.S_IMODE(held.st_mode) == stat.S_IMODE(named.st_mode) == 0o700,
            "frontend root identity/ownership/mode changed")


def preserve_frontend_failures():
    """Only direct fixture-root failure HTML/PNG, never a browser-profile subtree."""
    verify_frontend_identity()
    report = {"collected": [], "not_collected": [], "bytes": 0,
              "per_file_bytes": 4 * 1024**2, "aggregate_bytes": 16 * 1024**2,
              "scope": "Only direct files in direct fixture roots; no browser-profile traversal or symlink following"}
    seen = 0
    with os.scandir(FRONTEND_FD) as fixtures:
        for fixture in fixtures:
            guard_fixture_deadline()
            seen += 1
            require(seen <= 100000, "frontend failure artifact entry cap exceeded")
            metadata = os.stat(fixture.name, dir_fd=FRONTEND_FD, follow_symlinks=False)
            if not stat.S_ISDIR(metadata.st_mode) or fixture.name.startswith("browser-profile"):
                continue
            fixture_fd = os.open(fixture.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=FRONTEND_FD)
            try:
                held = os.fstat(fixture_fd)
                require((held.st_dev, held.st_ino) == (metadata.st_dev, metadata.st_ino), "failure fixture replaced")
                with os.scandir(fixture_fd) as entries:
                    for entry in entries:
                        guard_fixture_deadline()
                        seen += 1
                        require(seen <= 100000, "frontend failure artifact entry cap exceeded")
                        if not re.fullmatch(r"(?:failure|case-[A-Za-z0-9_.-]+-failure)\.(html|png)", entry.name):
                            continue
                        relative = fixture.name + "/" + entry.name
                        file_fd = None
                        try:
                            before = os.stat(entry.name, dir_fd=fixture_fd, follow_symlinks=False)
                            require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
                                    and before.st_nlink == 1, "not an owned single-link regular file")
                            require(before.st_size <= 4 * 1024**2 and report["bytes"] + before.st_size <= 16 * 1024**2,
                                    "failure artifact byte budget exceeded; not copied")
                            file_fd = os.open(entry.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fixture_fd)
                            actual = os.fstat(file_fd)
                            require((actual.st_dev, actual.st_ino, actual.st_size) == (before.st_dev, before.st_ino, before.st_size),
                                    "failure artifact replaced before read")
                            with os.fdopen(file_fd, "rb", closefd=False) as stream:
                                data = stream.read(4 * 1024**2 + 1)
                            after = os.fstat(file_fd)
                            require(len(data) == before.st_size and after.st_size == before.st_size,
                                    "failure artifact changed during bounded read")
                            name = "validation-frontend-failure-" + str(len(report["collected"]) + 1).zfill(4) + Path(entry.name).suffix
                            with (EVIDENCE / name).open("xb") as output:
                                output.write(data)
                            report["collected"].append({"source_relative": relative, "evidence": name,
                                                        "bytes": len(data), "sha256": digest(data)})
                            report["bytes"] += len(data)
                        except (OSError, RuntimeError) as error:
                            report["not_collected"].append({"source_relative": relative, "reason": str(error)})
                        finally:
                            if file_fd is not None:
                                os.close(file_fd)
            finally:
                os.close(fixture_fd)
    verify_frontend_identity()
    write_json("validation-frontend-failures.json", report)
    return report


def remove_frontend_temp():
    global FRONTEND_TMP, FRONTEND_NAME
    try:
        verify_frontend_identity()
        remove_owned_children(FRONTEND_FD)
        verify_frontend_identity()
        require(not os.listdir(FRONTEND_FD), "frontend root not empty after no-follow cleanup")
        os.rmdir(FRONTEND_NAME, dir_fd=FRONTEND_PARENT_FD)
        FRONTEND_TMP = FRONTEND_NAME = None
    finally:
        close_frontend_fds()


# Diagnostic only. Same original flags/CDP path, no dump-dom, sandbox change,
# product logger edit, alternate browser or browser-launch retry. The existing
# Python supervisor remains authoritative for all descendants and hard limits.
BROWSER_PROBE = r"""
const {spawn} = require('node:child_process');
const net = require('node:net');
const fs = require('node:fs/promises');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
(async () => {
  const [browser, profile, mode, startEvidence] = process.argv.slice(1);
  const reservation = net.createServer();
  await new Promise((resolve, reject) => { reservation.once('error', reject); reservation.listen(0, '127.0.0.1', resolve); });
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  const child = spawn(browser, [
    '--headless=new', '--remote-debugging-port=' + port, '--user-data-dir=' + profile,
    '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--disable-background-networking',
    '--window-size=1280,900', '--force-device-scale-factor=1', 'about:blank',
  ], {stdio: ['ignore', 'ignore', 'inherit']});
  let ended = null, socket = null, verified = null, error = null, forcedShutdown = false, browserStart = null, listenerChecks = 0;
  child.once('error', value => { ended = {error: String(value)}; });
  child.once('close', (code, signal) => { ended = {code, signal}; });
  try {
    const startTicks = async () => {
      const raw = await fs.readFile('/proc/' + child.pid + '/stat', 'utf8');
      return raw.slice(raw.lastIndexOf(')') + 2).trim().split(/\s+/)[19];
    };
    browserStart = await startTicks();
    const started = {mode,port,browserIdentity:{pid:child.pid,start:browserStart},helperPid:process.pid,
      observedMonotonicNs:process.hrtime.bigint().toString()};
    const serializedStart = JSON.stringify(started) + '\n';
    await new Promise(resolve => process.stdout.write('BROWSER_PROBE_STARTED ' + serializedStart, resolve));
    // Publish complete metadata exclusively; an interrupted .tmp is not proof.
    await fs.writeFile(startEvidence + '.tmp', serializedStart, {flag:'wx',mode:0o600});
    await fs.link(startEvidence + '.tmp', startEvidence);
    await fs.unlink(startEvidence + '.tmp');
    const verifyListener = async () => {
      if (ended || await startTicks() !== browserStart) throw new Error('browser identity unavailable');
      const tcp = await fs.readFile('/proc/' + child.pid + '/net/tcp', 'utf8');
      const address = '0100007F:' + port.toString(16).toUpperCase().padStart(4, '0');
      const inodes = new Set(tcp.split('\n').slice(1).map(line => line.trim().split(/\s+/))
        .filter(fields => fields[1] === address && fields[3] === '0A').map(fields => fields[9]));
      let owned = false;
      for (const name of await fs.readdir('/proc/' + child.pid + '/fd')) {
        try {
          const target = await fs.readlink('/proc/' + child.pid + '/fd/' + name);
          const match = /^socket:\[(\d+)\]$/.exec(target);
          if (match && inodes.has(match[1])) owned = true;
        } catch (error) { if (error.code !== 'ENOENT') throw error; }
      }
      if (!owned || ended || await startTicks() !== browserStart) throw new Error('CDP listener not bound to this browser identity');
      listenerChecks++;
    };
    let target = null;
    const deadline = Date.now() + 25000;
    while (!ended && !target && Date.now() < deadline) {
      try {
        const response = await fetch('http://127.0.0.1:' + port + '/json/list', {signal: AbortSignal.timeout(1000), redirect:'error'});
        if (!response.ok) throw new Error('CDP target list HTTP failure');
        let bytes = 0, chunks = [];
        for await (const chunk of response.body) {
          bytes += chunk.length;
          if (bytes > 65536) throw new Error('CDP target list exceeds 64 KiB');
          chunks.push(chunk);
        }
        const targets = JSON.parse(Buffer.concat(chunks).toString('utf8'));
        target = targets.find(item => item.type === 'page' && item.url === 'about:blank' && item.webSocketDebuggerUrl);
      } catch { /* Bounded polling of the same browser, never a relaunch. */ }
      if (!target) await delay(100);
    }
    if (ended || !target) throw new Error('browser exited or blank page target unavailable: ' + JSON.stringify(ended));
    const address = new URL(target.webSocketDebuggerUrl);
    if (address.protocol !== 'ws:' || address.hostname !== '127.0.0.1' || address.port !== String(port) || address.username || address.password
        || address.search || address.hash || !/^[A-Za-z0-9-]+$/.test(target.id) || address.pathname !== '/devtools/page/' + target.id)
      throw new Error('unexpected CDP target');
    await verifyListener();
    socket = new WebSocket(address);
    await new Promise((resolve, reject) => { socket.addEventListener('open', resolve, {once:true}); socket.addEventListener('error', reject, {once:true}); });
    const response = new Promise((resolve, reject) => {
      socket.addEventListener('message', event => {
        try {
          if (typeof event.data !== 'string' || Buffer.byteLength(event.data) > 65536) throw new Error('CDP response exceeds bound');
          const value = JSON.parse(event.data);
          if (value.id === 1) resolve(value);
        } catch (error) { reject(error); }
      });
      socket.addEventListener('error', reject, {once:true});
    });
    socket.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{
      expression:"new Promise(resolve => { const done = () => resolve({href:location.href,state:document.readyState}); if (document.readyState === 'complete') done(); else addEventListener('load', done, {once:true}); })",
      awaitPromise:true,returnByValue:true}}));
    const evaluated = await response;
    verified = evaluated.result?.result?.value;
    if (evaluated.error || evaluated.result?.exceptionDetails || verified?.href !== 'about:blank' || verified?.state !== 'complete')
      throw new Error('blank page evaluation did not succeed');
    await verifyListener();
    socket.send(JSON.stringify({id:2,method:'Browser.close'}));
    const closeDeadline = Date.now() + 3000;
    while (!ended && Date.now() < closeDeadline) await delay(50);
    if (!ended || ended.code !== 0) throw new Error('browser did not exit normally after Browser.close');
  } catch (value) {
    error = String(value);
    console.error('browser diagnostic: ' + error);
  } finally {
    if (socket) socket.close();
    if (!ended) { forcedShutdown = true; child.kill('SIGTERM'); }
    const stopDeadline = Date.now() + 3000;
    while (!ended && Date.now() < stopDeadline) await delay(50);
    if (!ended) { forcedShutdown = true; child.kill('SIGKILL'); }
    // Give crashpad normal shutdown a short grace before this helper exits.
    // Any descendant still alive remains a hard outer-supervisor rejection.
    await delay(250);
  }
  console.log('BROWSER_PROBE_RESULT ' + JSON.stringify({mode,port,verified,ended,error,forcedShutdown,browserIdentity:{pid:child.pid,start:browserStart},listenerChecks}));
  process.exitCode = !error && verified && ended?.code === 0 ? 0 : 1;
})().catch(error => { console.error(String(error)); process.exitCode = 1; });
"""


def browser_probe(mode, browser, source, env, pair_deadline):
    verify_frontend_identity()
    if mode == "long":
        require(len(os.fsencode(FRONTEND_TMP)) == 81, "original long frontend TMPDIR is not exactly 81 bytes")
    else:
        require(mode == "short" and re.fullmatch(r"/tmp/f[0-9a-f]{7}", str(FRONTEND_TMP)), "short frontend root differs")
    probe_root = FRONTEND_TMP / "probe-empty"
    probe_root.mkdir(mode=0o700)
    profile = probe_root / "browser-profile"
    profile.mkdir(mode=0o700)
    probe_env = env.copy()
    probe_env.update(TMPDIR=str(FRONTEND_TMP), TMP=str(FRONTEND_TMP), TEMP=str(FRONTEND_TMP))
    label = "browser-probe-" + mode
    before_records = len(RECORDS)
    # Original cleanup allows TERM2 + KILL3 + reap1 seconds. Keep additional
    # margin for scans/evidence; never grant these diagnostics a fresh clock.
    active_seconds = min(35, pair_deadline - time.monotonic() - 8)
    require(active_seconds > 0, "combined browser diagnostic deadline reached before launch")
    try:
        output = run(label, ["node", "-e", BROWSER_PROBE, str(browser), str(profile), mode,
                              str(EVIDENCE / ("validation-browser-probe-started-" + mode + ".json"))],
                     active_seconds, source, probe_env)
    except RuntimeError:
        if mode != "long":
            raise
        # Expected long failure is narrowly recognized only after the unchanged
        # supervisor confirms its real nonzero exit, complete logs and cleanup.
        require(mode == "long" and len(RECORDS) == before_records + 1, "browser diagnostic did not complete")
        record = RECORDS[-1]
        require(record["reason"] == "nonzero_exit" and record["exit_code"] == 1 and not record["survivors"],
                "long probe was incomplete, timed out or retained descendants")
        output = (EVIDENCE / record["logs"]["stdout"]).read_bytes()
    record = RECORDS[-1]
    stderr = (EVIDENCE / record["logs"]["stderr"]).read_bytes()
    matches = re.findall(rb"^BROWSER_PROBE_RESULT (.+)$", output, re.M)
    require(len(matches) == 1, "unique browser diagnostic result missing")
    value = json.loads(matches[0])
    require(value["mode"] == mode and record["duration_seconds"] <= 45, "browser diagnostic identity or 45s total bound differs")
    evidence = {"mode": mode, "tmpdir": str(FRONTEND_TMP), "tmpdir_bytes": len(os.fsencode(FRONTEND_TMP)),
                "profile": str(profile), "result": value, "command": record,
                "stdout_sha256": digest(output), "stderr_sha256": digest(stderr),
                "browser_path": str(browser), "browser_entrypoint_sha256": file_digest(browser),
                "same_original_browser_flags": True, "no_sandbox_bypass": True,
                "entrypoint_stat": list((browser.stat().st_dev, browser.stat().st_ino, browser.stat().st_size)),
                "version_binding": "Chromium 155.0.8059.0 matches run1; run1 did not record a binary hash, so byte identity to its runner is not asserted"}
    write_json("validation-" + label + ".json", evidence)
    classify_browser_probe(evidence, stderr)
    write_json("validation-" + label + ".json", evidence)
    return evidence


def classify_browser_probe(evidence, stderr):
    """Pure evidence gate; a synthetic parser test is not a browser experiment."""
    mode, record, value = evidence["mode"], evidence["command"], evidence["result"]
    require(mode in {"long", "short"} and not record["survivors"]
            and record["exit_code"] == (1 if mode == "long" else 0)
            and record["duration_seconds"] <= 45, "browser diagnostic command or cleanup incomplete")
    if mode == "long":
        paths = re.findall(rb"Socket path too long: ([^\r\n]+)", stderr)
        owned = [os.fsdecode(path) for path in paths if path.startswith(os.fsencode(evidence["tmpdir"]) + b"/") and len(path) >= 108]
        ended = value.get("ended") or {}
        spontaneous_failure = (ended.get("signal") is not None
                               or isinstance(ended.get("code"), int) and ended["code"] != 0)
        require(record["reason"] == "nonzero_exit" and value["error"] is not None and owned
                and spontaneous_failure and value.get("forcedShutdown") is False,
                "long probe lacks the exact owned overlength socket diagnostic; cause not confirmed, full suite blocked")
        evidence["explicit_overlength_socket_paths"] = owned
    else:
        require(record["reason"] == "completed" and value["error"] is None
                and value["verified"] == {"href": "about:blank", "state": "complete"}
                and value["ended"] == {"code": 0, "signal": None} and value.get("forcedShutdown") is False
                and value.get("listenerChecks") == 2,
                "short original-mode browser did not verify blank page and exit cleanly")


def libtest_blocks(output):
    """Read actual result lines, never turn a static inventory into passes."""
    text = output.decode()
    pattern = re.compile(r"^running (\d+) tests?\n(.*?)^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;[^\n]*", re.M | re.S)
    blocks = []
    for match in pattern.finditer(text):
        outcome_section = match[2].split("\nsuccesses:\n", 1)[0].split("\nfailures:\n", 1)[0]
        entries = re.findall(r"^test (.*?) \.\.\. (ok|FAILED|ignored)(?:, [^\n]*)?$", outcome_section, re.M)
        blocks.append({"listed_run_count": int(match[1]), "entries": entries, "status": match[3],
                       **dict(zip(["passed", "failed", "ignored", "measured", "filtered"], map(int, match.group(4,5,6,7,8))))})
    require(len(blocks) == len(re.findall(r"^test result:", text, re.M)), "unparsed libtest summary")
    return blocks


def full_cargo_evidence(output, stderr, manifest, source, env):
    global STAGE
    STAGE = "full_cargo_evidence"
    require(len(re.findall(r"^\s*Finished `test` profile \[[^\n]+\] target\(s\) in [^\n]+$", stderr, re.M)) == 1,
            "unique actual full Cargo compilation completion missing")
    write_json("validation-full-cargo-compiler.json", {"exit_code": 0, "locked": True,
        "stdout_sha256": digest(output), "stderr_sha256": digest(stderr.encode()),
        "artifact_provenance": "Original Cargo Finished and Running records, followed by identity/hash and actual --list",
        "input_tree": INPUT_TREE, "input_patch_sha256": PATCH_SHA256})
    targets = manifest["stage2"]["targets"]
    by_source = {item["src"]: group for group, item in targets.items()}
    executions = re.findall(r"^\s*Running (?:unittests )?(\S+) \(([^\n]+)\)$", stderr, re.M)
    require(len(executions) == len(targets), "full Cargo actual binary count differs")
    require(sorted(path for path, _ in executions) == sorted(by_source), "full Cargo executed source targets differ")
    blocks = libtest_blocks(output)
    require(len(blocks) == len(targets) + 1, "all binary and normal doctest summaries required")
    results = {}
    for (src, raw_path), block in zip(executions, blocks):
        group = by_source[src]
        item = targets[group]
        path = Path(raw_path)
        if not path.is_absolute():
            path = source / "tauri" / path
        path = path.resolve(strict=True)
        require(path.is_relative_to(TARGET_ROOT) and path.is_file() and os.access(path, os.X_OK),
                "actual Cargo test artifact escaped owned target")
        identity = path.stat()
        artifact_record = {"path": str(path), "sha256": file_digest(path),
                           "identity": [identity.st_dev, identity.st_ino], "source": src,
                           "provenance": "Actual full cargo stderr Running path, not a guessed filename"}
        write_json("validation-" + group.replace("_", "-") + "-artifact.json", artifact_record)
        listed = inventory(run(group.replace("_", "-") + "-full-list",
                               [str(path), "--list", "--test-threads=1"], 15, source, env))
        ignored = inventory(run(group.replace("_", "-") + "-ignored-list",
                                [str(path), "--ignored", "--list", "--test-threads=1"], 15, source, env))
        after = path.stat()
        require((after.st_dev, after.st_ino) == (identity.st_dev, identity.st_ino)
                and file_digest(path) == artifact_record["sha256"], "artifact changed during inventory")
        require(listed == item["names"] and ignored == item["ignored"], "actual full/ignored inventory differs: " + group)
        expected = sorted((name, "ignored" if name in ignored else "ok") for name in listed)
        require(sorted(block["entries"]) == expected and block["listed_run_count"] == len(listed),
                "actual full Cargo case outcomes differ: " + group)
        require(block["status"] == "ok" and block["passed"] == len(listed) - len(ignored)
                and block["ignored"] == len(ignored) and block["failed"] == block["measured"] == block["filtered"] == 0,
                "full Cargo summary differs: " + group)
        results[group] = {**block, "artifact": artifact_record, "names": listed, "ignored_names": ignored}
    docs = blocks[-1]
    actual_docs = []
    for name, status in docs["entries"]:
        parsed = re.fullmatch(r"(.+?) - (.*?) \(line (\d+)\)( - compile)?", name)
        require(parsed is not None and status == "ok", "doctest did not pass or name unparsed")
        actual_docs.append((parsed[1], int(parsed[3]), "no_run" if parsed[4] else "run"))
    expected_docs = sorted((item["path"], item["line"], item["mode"]) for item in manifest["stage2"]["doctest_source_examples"])
    require(sorted(actual_docs) == expected_docs, "actual normal doctest inventory differs")
    require(docs["status"] == "ok" and docs["passed"] == docs["listed_run_count"] == len(expected_docs)
            and docs["failed"] == docs["ignored"] == docs["measured"] == docs["filtered"] == 0,
            "normal doctest summary differs")
    require(re.findall(r"^\s*Doc-tests (\S+)\s*$", stderr, re.M) == ["ai_toolbox_lib"], "normal rustdoc target missing")
    markers = re.findall(r"^ordinary integration child: (.*); 1 passed; 0 failed; 0 ignored; reaped$", output.decode(), re.M)
    expected_children = sorted(name.split(":", 1)[1] for name in manifest["integration_child_parents"])
    require(sorted(markers) == expected_children and len(markers) == len(set(markers)) == 20,
            "ordinary integration success markers absent, duplicated or unexpected")
    parent_actors = {key: value for key, value in manifest["actor_parents"].items() if key.startswith("desktop_")}
    require(sum(item["normal"] for item in parent_actors.values()) == 3
            and all(item["abrupt"] == 0 for item in parent_actors.values()), "desktop parent actor contract changed")
    return {"binaries": results, "doctests": docs,
            "ordinary_passed": sum(item["passed"] for item in results.values()),
            "ignored_not_counted_as_pass": sum(item["ignored"] for item in results.values()),
            "nested_execution_accounting": {
                "admission_normal_actor_completions_proved_by_parent_assertions": 3,
                "other_source_bound_nested_parent": {
                    "parent": "desktop_lib:settings::backup::utils::tests::restore_migration_failure_rolls_back_synthetic_database",
                    "successful_child_asserted_by_passing_parent": 1,
                    "externally_observed_child_success_marker": False,
                    "not_an_additional_ordinary_pass": True},
                "abrupt_actor_completions": 0,
                "external_actor_success_marker_count": 0,
                "ordinary_integration_child_markers": markers,
                "source_bound_actor_parents": parent_actors,
                "note": "All three actor parents passed; fixed source asserts successful exact child summary, completion-file marker and reaping, but emits no actor success marker. Twenty integration markers are externally observed via show-output. The 20/3 counts are admission-specific, not all suite subprocesses. The preserved rollback parent asserts one additional exact child. None is added to ordinary passes; Stage1 core actors are not rerun."}}



def validate(checkout, manifest, manifest_path, diagnostic_only=False):
    global RUN_ROOT, CACHE_ROOT, TARGET_ROOT, ARCHIVES_ALLOWED, STAGE, FINALIZING, FIXTURE
    global SOURCE_ROOT, NODE_STORE, NODE_INSTALLING
    patch = source_check(checkout, manifest, manifest_path)
    RUN_ROOT = Path(os.environ["RUNNER_TEMP"]).resolve() / (
        "typed-context-admission-stage2-" + os.environ["GITHUB_RUN_ID"] + "-" + os.environ["GITHUB_RUN_ATTEMPT"])
    require(not RUN_ROOT.exists() and not RUN_ROOT.is_symlink(), "run root already exists")
    RUN_ROOT.mkdir(mode=0o700)
    env = clean_env(RUN_ROOT / "environment")
    env.update(CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse",
               CARGO_REGISTRIES_CRATES_IO_INDEX="sparse+https://index.crates.io/",
               HUSKY="0", CI="true", NODE_OPTIONS="--max-old-space-size=4096",
               npm_config_registry="https://registry.npmjs.org/", npm_config_fetch_retries="0",
               npm_config_fetch_timeout="20000", npm_config_cache=str(RUN_ROOT / "npm-cache"),
               npm_config_store_dir=str(RUN_ROOT / "pnpm-store"),
               npm_config_userconfig=str(RUN_ROOT / "npmrc"), npm_config_globalconfig=str(RUN_ROOT / "global-npmrc"))
    for name in ["npmrc", "global-npmrc"]:
        (RUN_ROOT / name).write_text("")
    NODE_STORE = RUN_ROOT / "pnpm-store"
    CACHE_ROOT = Path(env["CARGO_HOME"])
    TARGET_ROOT = Path(env["CARGO_TARGET_DIR"]).resolve()
    (CACHE_ROOT / "config.toml").write_text('[source.crates-io]\nregistry = "sparse+https://index.crates.io/"\n')
    source = before = tested = tested_tree = tested_patch_sha = None
    dist = dist_identity = asset_identity = None
    result = {"ok": False, "product_ready": False, "error": None,
              "source_or_lock_mutation_allowed": False,
              "scope": "Short-browser owned-scope diagnostic only" if diagnostic_only else "Complete original pnpm test, tsc and unfiltered desktop Cargo tests including normal doctests",
              "diagnostic_only": diagnostic_only, "complete_suite_executed": False,
              "gate_results": {}, "source_base": BASE}
    try:
        STAGE = "toolchain_guard"
        if not diagnostic_only:
            require(run("rust-version", ["rustc", "--version"], 10, checkout, env).decode().startswith("rustc 1.99.1 "), "unexpected rustc version")
            require(run("cargo-version", ["cargo", "--version"], 10, checkout, env).decode().startswith("cargo 1.99.1 "), "unexpected cargo version")
            require(run("pnpm-version", ["pnpm", "--version"], 10, checkout, env).decode().strip() == "9.15.0", "pnpm 9.15.0 required")
        require(run("node-version", ["node", "--version"], 10, checkout, env).decode().startswith("v22."), "Node 22 required")
        candidates = [Path(path) for path in ["/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome"]]
        browser = next((path for path in candidates if path.exists()), None)
        require(browser is not None and os.access(browser, os.X_OK), "reviewed existing browser unavailable; no install or sandbox bypass")
        version = run("browser-version", [str(browser), "--version"], 15, checkout, env).decode().strip()
        write_json("validation-browser.json", {"path": str(browser), "version": version,
                   "selection": "Existing shared runner order; original scripts choose the same candidates", "sandbox_disabled": False})
        source, before = prepare_source(checkout, manifest, patch, env)
        SOURCE_ROOT = source
        tested, tested_tree, tested_patch_sha = seal_immutable_source(source, manifest, before, env)
        shutil.copyfile(patch, EVIDENCE / "validation-tested.patch")
        result.update(tested_tree=tested_tree, tested_patch_sha256=tested_patch_sha)
        create_frontend_temp(short=True)
        STAGE = "browser_scope_diagnostic"
        require(str(browser) == "/usr/bin/chromium" and version == "Chromium 155.0.8059.0",
                "diagnostic requires the same recorded browser version; no alternate/version fallback")
        # Run2 already retained the explicit long-path fatal evidence. Never
        # relaunch that known-failing long probe in this diagnostic or later gate.
        probe_started = time.monotonic()
        short_probe = browser_probe("short", browser, source, env, probe_started + 45)
        result["browser_scope_diagnostic"] = {"short": short_probe,
            "duration_seconds": round(time.monotonic() - probe_started, 3),
            "historical_long_run": "40052460948", "long_probe_rerun": False,
            "not_a_frontend_test_pass": True,
            "historical_rejection_attribution": "A fresh short-only success does not identify run2's unrecorded object; that run shared HOME/XDG with its preceding long probe"}
        if diagnostic_only:
            result["ok"] = True
            result["outcome"] = "short_browser_diagnostic_completed_full_suite_not_run"
            return result  # The unchanged finally still proves cleanup/source/locks.
        # Preserved future full-suite code is unreachable in this sealed mode.
        rust_env = create_runtime_fixture(source, env)
        check_disk_budgets(force=True)
        frontend_env = env.copy()
        frontend_env.update(TMPDIR=str(FRONTEND_TMP), TMP=str(FRONTEND_TMP), TEMP=str(FRONTEND_TMP))
        ARCHIVES_ALLOWED = True
        STAGE = "frozen_install"
        NODE_INSTALLING = True
        try:
            run("pnpm-install", ["pnpm", "install", "--frozen-lockfile"], 300, source, frontend_env)
        finally:
            NODE_INSTALLING = False
        check_disk_budgets(force=True)  # Completed install requires strict existing link targets.
        verify_hashes(source, manifest)
        result["gate_results"]["frozen_install"] = "passed"
        STAGE = "frontend_tests"
        output = run("pnpm-test", ["pnpm", "test"], 1200, source, frontend_env)
        headers = re.findall(r"^> ai-toolbox@1\.2\.0 (test:[^ ]+) ", output.decode(), re.M)
        require(headers == manifest["stage2"]["pnpm_chain"], "complete original 27-entry test chain not observed")
        result["gate_results"]["pnpm_test"] = {"exit": 0, "observed_chain": headers,
            "actual_summary_lines": [line for line in output.decode().splitlines() if re.match(r"^(# |ℹ )(tests|suites|pass|fail|cancelled|skipped|todo|duration_ms)\b", line)],
            "raw_stdout": RECORDS[-1]["logs"]["stdout"], "static_counts_not_used_as_passes": True}
        verify_hashes(source, manifest)
        STAGE = "typescript"
        run("typescript", ["pnpm", "exec", "tsc", "--noEmit"], 180, source, frontend_env)
        result["gate_results"]["typescript"] = "passed"
        verify_hashes(source, manifest)
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
                   "purpose": "compile-only empty asset; not frontend packaging or GUI validation"})
        STAGE = "full_cargo"
        output = run("full-cargo", ["cargo", "test", "--locked", "--manifest-path", "tauri/Cargo.toml",
                     "--jobs", "2", "--", "--test-threads=1", "--show-output"], 1680, source, rust_env)
        stderr = (EVIDENCE / RECORDS[-1]["logs"]["stderr"]).read_bytes().decode()
        result["gate_results"]["full_cargo"] = full_cargo_evidence(output, stderr, manifest, source, rust_env)
        check_fixture_budget(force=True)
        result["runtime_residual_before_cleanup"] = dict(FIXTURE_OBSERVATION)
        # Cargo/rustdoc own temporary remnants are not evidence of a leaked test;
        # only the held private scope is removed after every owned process exits.
        result["complete_suite_executed"] = True
        result["ok"] = True
        result["outcome"] = "stage2_complete_gates_passed_final_review_required_not_product_ready"
    except BaseException as error:
        result["error"] = f"{type(error).__name__}: {error}"
        result["failed_stage"] = STAGE
        confirmed_test_failure = False
        if RECORDS and STAGE == "full_cargo" and RECORDS[-1]["reason"] == "nonzero_exit":
            logged = (EVIDENCE / RECORDS[-1]["logs"]["stdout"]).read_text()
            diagnostic = logged + (EVIDENCE / RECORDS[-1]["logs"]["stderr"]).read_text()
            fixture_incomplete = any(marker in diagnostic for marker in (
                "ordinary fixture timed out:", "ordinary fixture output exceeded bound:",
                "ordinary fixture child exit unconfirmed", "fixture log limit",
                "admission actor deadline exceeded; retain fixture",
                "isolated GUI admission actor was not reaped; fixture retained",
                "isolated rollback test log exceeded 16 KiB",
                "isolated rollback test exceeded 60s or wait failed:", "test log exceeds 16 KiB"))
            confirmed_test_failure = not fixture_incomplete and bool(re.search(r"^test result: FAILED\.", logged, re.M))
        result["outcome"] = ("rust_test_failure_requires_diagnosis_no_retry" if confirmed_test_failure else
                             "frontend_or_typecheck_failed_requires_diagnosis_no_retry" if STAGE in {"frontend_tests", "typescript"}
                             else "setup_compile_guard_or_incomplete_requires_diagnosis_no_retry")
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
            if not result["ok"] and FRONTEND_TMP is not None:
                try:
                    result["frontend_failure_artifacts"] = preserve_frontend_failures()
                except BaseException as artifact_error:
                    # Diagnostic capture never changes the failed gate's exit or
                    # expands scope/limits; cleanup still runs on the same root.
                    result["frontend_failure_artifacts_error"] = str(artifact_error)
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
            if FRONTEND_TMP is not None:
                remove_frontend_temp()
            result["fixture_cleanup_scope"] = "Verified held-FD Rust and frontend roots only; shared /tmp parent never enumerated; links unlinked without following"
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
        result.update(finalization_errors=errors, cache=CACHE_OBSERVATION, memory=MEMORY_OBSERVATION,
                      command_count=len(RECORDS), commands=RECORDS)
        write_json("validation-result.json", result)
    return result


def main():
    global EVIDENCE, PHASE, JOB_DEADLINE, CANDIDATE_PATHS, EXPECTED_COUNTS, DESKTOP_SELECTED
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=["preflight", "validation"], required=True)
    parser.add_argument("--diagnostic-only", action="store_true")
    options = parser.parse_args()
    PHASE = options.phase
    require(sys.platform == "linux" and os.geteuid() != 0, "unprivileged Linux required")
    checkout = Path(os.environ["GITHUB_WORKSPACE"]).resolve()
    runner_temp = Path(os.environ["RUNNER_TEMP"]).resolve()
    require(runner_temp != Path("/tmp") and not runner_temp.is_relative_to(Path("/dev/shm")),
            "source/build/evidence must use runner disk, never tmpfs")
    EVIDENCE = runner_temp / "typed-context-admission-stage2-evidence"
    require(not EVIDENCE.is_symlink(), "evidence symlink refused")
    EVIDENCE.mkdir(mode=0o700, exist_ok=True)
    require(not (EVIDENCE / (PHASE + "-summary.json")).exists(), "phase already ran; no blind retry")
    clock = EVIDENCE / "preflight-budget.json"
    if PHASE == "preflight":
        require(not clock.exists(), "budget already initialized")
        write_json(clock.name, {"deadline_monotonic": time.monotonic() + 8 * 60,
                               "deadline_unix_advisory": time.time() + 8 * 60,
                               "shared_setup_validation_seconds": 480})
    JOB_DEADLINE = json.loads(clock.read_text())["deadline_monotonic"]
    summary = {"phase": PHASE, "ok": False, "error": None, "product_ready": False,
               "execution_mode": "short-browser-scope-diagnostic-only", "complete_suite_executed": False,
               "shared_setup_validation_seconds": 480, "finalization_reserve_seconds": FINALIZE_RESERVE}
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
        require(manifest["stage2"]["execution_mode"] == "short-browser-scope-diagnostic-only",
                "this reviewed harness permits only the bounded browser diagnostic")
        if PHASE == "preflight":
            source_check(checkout, manifest, manifest_path)
            summary["ok"] = True
        else:
            require(options.diagnostic_only, "full-suite dispatch is disabled in this diagnostic-only harness")
            result = validate(checkout, manifest, manifest_path, diagnostic_only=True)
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
