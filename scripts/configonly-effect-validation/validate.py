#!/usr/bin/env python3
"""Fixed private ConfigOnly SQLite/file fixtures; no desktop, deployment or real profile."""
import argparse,hashlib,json,os,re,selectors,signal,subprocess,sys,time,tempfile,shutil,stat
from pathlib import Path
LOG_LIMIT=16*1024*1024
TOTAL_LOG_LIMIT=64*1024*1024
RECORDS=[]
TOTAL_LOG_BYTES=0
ACTIVE=None
EVIDENCE=None
PHASE='validation'
JOB_DEADLINE=None
FIXTURE=None
FIXTURE_ID=None
LAST_SCAN=0
BASE='044e45d373fc078c3affa1628b5cb4286bb36d77'
MANIFEST_SHA='c40081f5fca6c97e53f9f161700fc57d45c92f99f39c1590e15ec9c24d0ee83d'
FINALIZING=False
FIXTURE_BUDGET=32*1024*1024
FIXTURE_FILE_LIMIT=8*1024*1024
FINALIZE_RESERVE=45
FORMAT_FILES=(
    'crates/ai-toolbox-management/src/codex_master.rs',
    'crates/ai-toolbox-management/src/codex_service.rs',
    'crates/ai-toolbox-management/src/config_file.rs',
)
CANDIDATE_FILES=set(FORMAT_FILES)|{
    'crates/ai-toolbox-management/AGENTS.md',
    'crates/ai-toolbox-management/Cargo.lock',
    'crates/ai-toolbox-management/Cargo.toml',
    'crates/ai-toolbox-management/src/lib.rs',
    'crates/ai-toolbox-management/src/owner.rs',
}
HARNESS_FILES={
    '.github/workflows/maintenance-validation.yml',
    'scripts/configonly-effect-validation/validate.py',
    'scripts/configonly-effect-validation/candidate.json',
    'scripts/configonly-effect-validation/candidate.patch',
}

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
            if FIXTURE is not None:
                check_fixture_budget()
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
                total += item.st_size
            require(count <= 512 and total <= FIXTURE_BUDGET,
                    "synthetic SQLite/file fixture storage budget exceeded")
    space = os.statvfs(FIXTURE)
    require(space.f_bavail * space.f_frsize >= 16 * 1024 * 1024,
            "runtime tmpfs low space")


def safe_env(home):
    env = {
        "PATH": os.environ["PATH"], "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8",
        "RUST_BACKTRACE": "0", "RUSTUP_TOOLCHAIN": "1.99.1",
        "CARGO_BUILD_JOBS": "2", "CARGO_INCREMENTAL": "0",
        "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0",
        "CARGO_TERM_COLOR": "never", "CARGO_HTTP_TIMEOUT": "30",
        "CARGO_NET_RETRY": "1", "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": "/dev/null",
    }
    # Read only the official toolchain installed by this job, not cargo caches,
    # credentials, user HOME contents or an inherited cargo configuration.
    env["RUSTUP_HOME"] = os.environ.get("RUSTUP_HOME", str(Path(os.environ["HOME"]) / ".rustup"))
    for key in ("HOME", "USERPROFILE", "CARGO_HOME", "CARGO_TARGET_DIR",
                "TMPDIR", "TMP", "TEMP", "XDG_CONFIG_HOME", "XDG_DATA_HOME",
                "XDG_CACHE_HOME", "AI_TOOLBOX_DATA_DIR", "CODEX_HOME"):
        path = home / key
        path.mkdir(mode=0o700)
        env[key] = str(path)
    return env


def verify_source_inventory(source, manifest):
    actual = []
    for path, spec in manifest["source_inventory"].items():
        text = (source / path).read_text()
        names = re.findall(r"#\[test\](?:\s*#\[[^\n]+\])*\s*fn\s+(\w+)", text)
        require(len(names) == spec["count"], "source inventory count changed: " + path)
        actual.extend(spec["prefix"] + name for name in names)
    require(sorted(actual) == manifest["inventories"]["lib"],
            "source lib names differ from reviewed inventory")
    owner = (source / "crates/ai-toolbox-management/tests/owner_process.rs").read_text()
    names = re.findall(r"#\[test\](?:\s*#\[[^\n]+\])*\s*fn\s+(\w+)", owner)
    require(sorted(names) == manifest["inventories"]["owner"], "owner source inventory changed")
    ignored = re.findall(r'#\[ignore[^\n]*\]\s*fn\s+(\w+)', owner)
    require(ignored == ["fixture_process"], "unexpected ignored source test")
    require(len(actual) == 48 and len(names) == 12, "wrong ordinary test inventory")


def audit_registry_lock(lock):
    packages = lock.read_text().split("[[package]]")[1:]
    require(packages, "empty independent lock")
    registry = 0
    local = set()
    for package in packages:
        name = re.search(r'^name = "([^"]+)"$', package, re.M)
        source = re.search(r'^source = "([^"]+)"$', package, re.M)
        require(name is not None, "unnamed locked package")
        if source:
            require(source.group(1) == "registry+https://github.com/rust-lang/crates.io-index",
                    "unreviewed dependency source")
            require(re.search(r'^checksum = "[0-9a-f]{64}"$', package, re.M),
                    "registry package missing checksum")
            registry += 1
        else:
            local.add(name.group(1))
    require(local == {"ai-toolbox-management", "ai-toolbox-storage", "ai-toolbox-codex-config"},
            "unreviewed local dependency")
    return {"registry_packages": registry, "local_packages": sorted(local)}


def main():
    global EVIDENCE, JOB_DEADLINE, FIXTURE, FIXTURE_ID, FINALIZING
    started = time.monotonic()
    require(sys.platform == "linux" and os.geteuid() != 0, "unprivileged Linux required")
    checkout = Path(os.environ["GITHUB_WORKSPACE"]).resolve()
    runner_temp = Path(os.environ["RUNNER_TEMP"]).resolve()
    EVIDENCE = runner_temp / "configonly-effect-evidence"
    EVIDENCE.mkdir(mode=0o700)
    JOB_DEADLINE = started + 600
    result = {
        "ok": False, "error": None, "format_review_required": True,
        "scope": "48 management lib cases and 11 owner parents; 13 actors separately; no desktop/Windows/admission closure",
        "deadline_seconds": 600, "finalization_reserve_seconds": FINALIZE_RESERVE,
    }
    source = None
    env = None
    manifest = None
    tested_files = None
    tested_tree = None
    tested_patch_sha = None
    try:
        require(os.environ.get("GITHUB_REPOSITORY") == "xue-ding-e/AI-Toolbox-Next", "wrong repository")
        require(os.environ.get("GITHUB_REF") == "refs/heads/validation/codex-configonly-effect", "wrong branch")
        require(os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch", "manual only")
        head = os.environ["EXPECTED_SHA"]
        require(re.fullmatch("[0-9a-f]{40}", head) and head == os.environ["GITHUB_SHA"],
                "exact reviewed validation SHA required")
        envroot = runner_temp / "configonly-effect-isolation"
        envroot.mkdir(mode=0o700)
        env = safe_env(envroot)
        def git(label, arguments, cwd=checkout):
            return run(label, ["git", *arguments], 15, cwd, env)
        parents = git("parents", ["rev-list", "--parents", "-n", "1", "HEAD"]).decode().split()
        require(parents == [head, BASE], "one harness commit directly above fixed base required")
        require(git("base-tree", ["rev-parse", BASE + "^{tree}"]).decode().strip()
                == "f4d64278fbbef295971acf6fea69f0da8c2beecd", "fixed base tree mismatch")
        require(not git("initial-status", ["status", "--porcelain"]).strip(), "dirty harness checkout")
        scope = git("harness-scope", ["diff", "--name-only", BASE, "HEAD"]).decode().splitlines()
        require(set(scope) == HARNESS_FILES, "unexpected harness scope")
        manifest_path = checkout / "scripts/configonly-effect-validation/candidate.json"
        require(file_digest(manifest_path) == MANIFEST_SHA, "sealed input manifest hash mismatch")
        manifest = json.loads(manifest_path.read_text())
        require(manifest["schema_version"] == 1 and manifest["base"] == BASE
                and manifest["state"] == "sealed-input-pending-harness-review",
                "candidate not sealed for this reviewed driver")
        require(set(manifest["files"]) == CANDIDATE_FILES
                and manifest["format_files"] == list(FORMAT_FILES), "candidate/format whitelist mismatch")
        require(manifest["lib_passed"] == 48 and manifest["ordinary_passed"] == 59
                and manifest["owner_parents"] == 11 and manifest["actor_invocations"] == 13,
                "reviewed count mismatch")
        require(not (set(manifest["files"]) & set(manifest["preserved"])), "overlapping source maps")
        expected_files = {**manifest["files"], **manifest["preserved"]}
        for path, checksum in expected_files.items():
            require(not Path(path).is_absolute() and ".." not in Path(path).parts
                    and re.fullmatch("[0-9a-f]{64}", checksum), "unsafe source map entry")
        patch = manifest_path.with_name("candidate.patch")
        require(file_digest(patch) == manifest["patch_sha256"], "input patch hash mismatch")
        space = os.statvfs(runner_temp)
        require(space.f_bavail * space.f_frsize >= 2 * 1024**3, "build disk space insufficient")
        # Source, registry download, compiler intermediates and linker targets
        # stay on runner disk. Only runtime fixtures use existing tmpfs.
        source = runner_temp / "configonly-effect-source"
        git("worktree", ["worktree", "add", "--detach", str(source), BASE])
        git("apply", ["apply", "--index", "--whitespace=error", str(patch)], source)
        input_paths = git("candidate-scope", ["diff", "--cached", "--name-only", BASE], source).decode().splitlines()
        require(set(input_paths) == CANDIDATE_FILES, "applied patch changed unexpected product files")
        input_files = {path: file_digest(source / path) for path in expected_files}
        require(input_files == expected_files, "input candidate/preserved source hash mismatch")
        result["input_files"] = input_files
        # This complete git tree covers all tracked build inputs, not only the
        # eight changed files, including both path crates and embedded resources.
        input_tree = git("input-tree", ["write-tree"], source).decode().strip()
        result["input_tree"] = input_tree
        git("input-working-diff", ["diff", "--exit-code"], source)
        require(not git("input-untracked", ["ls-files", "--others", "--exclude-standard"], source).strip(),
                "untracked input source")
        verify_source_inventory(source, manifest)
        lock = source / "crates/ai-toolbox-management/Cargo.lock"
        result["lock_audit"] = audit_registry_lock(lock)
        result["input_lock_sha256"] = file_digest(lock)
        require(run("rust-version", ["rustc", "--version"], 10, source, env).decode().startswith("rustc 1.99.1 "),
                "unexpected rustc version")
        require(run("cargo-version", ["cargo", "--version"], 10, source, env).decode().startswith("cargo 1.99.1 "),
                "unexpected cargo version")
        run("rustfmt-version", ["rustfmt", "--version"], 10, source, env)
        # These three files contain inline modules only. Refuse out-of-line
        # children instead of letting rustfmt traverse other product modules.
        for path in FORMAT_FILES:
            text = (source / path).read_text()
            require(not re.search(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*;", text)
                    and not re.search(r"\binclude!\s*\(", text), "formatter would traverse an external child")
        fmt_config = envroot / "rustfmt.toml"
        fmt_config.write_text('edition = "2021"\nreorder_imports = false\nreorder_modules = false\n')
        run("format-new-modules", ["rustfmt", "--edition", "2021", "--config-path", str(fmt_config),
                                  *[str(source / path) for path in FORMAT_FILES]], 30, source, env)
        formatted_paths = git("format-scope", ["diff", "--name-only"], source).decode().splitlines()
        require(set(formatted_paths) <= set(FORMAT_FILES), "formatter changed a non-whitelisted path")
        tested_files = {path: file_digest(source / path) for path in expected_files}
        require(all(tested_files[path] == checksum for path, checksum in input_files.items()
                    if path not in FORMAT_FILES), "format modified preserved or non-format candidate bytes")
        git("stage-formatted-only", ["add", "--", *FORMAT_FILES], source)
        tested_tree = git("tested-tree", ["write-tree"], source).decode().strip()
        tested_patch = git("tested-patch", ["diff", "--cached", "--binary", "--full-index", BASE], source)
        tested_patch_sha = digest(tested_patch)
        (EVIDENCE / "validation-tested.patch").write_bytes(tested_patch)
        result.update(after_format_files=tested_files, tested_tree=tested_tree,
                      tested_patch_sha256=tested_patch_sha, formatted_paths=formatted_paths)
        result["format_artifacts"] = {}
        for path in FORMAT_FILES:
            artifact_name = "validation-formatted-" + Path(path).name
            shutil.copyfile(source / path, EVIDENCE / artifact_name)
            result["format_artifacts"][path] = {"name": artifact_name, "sha256": tested_files[path]}
        write_json("validation-source-map.json", {
            "base": BASE, "input_tree": input_tree, "tested_tree": tested_tree,
            "input_patch_sha256": manifest["patch_sha256"], "tested_patch_sha256": tested_patch_sha,
            "input_files": input_files, "after_format_files": tested_files,
            "format_artifacts": result["format_artifacts"],
            "parent_review_required_before_product_commit": True,
        })
        verify_source_inventory(source, manifest)
        build_manifest = source / "crates/ai-toolbox-management/Cargo.toml"
        output = run("compile", ["cargo", "test", "--manifest-path", str(build_manifest),
                     "--locked", "--jobs", "2", "--lib", "--test", "owner_process",
                     "--no-run", "--message-format=json"], 300, source, env)
        require(any(json.loads(line).get("reason") == "build-finished"
                    and json.loads(line).get("success") is True for line in output.decode().splitlines()),
                "compiler completion missing")
        target = Path(env["CARGO_TARGET_DIR"]).resolve()
        bins = {key: artifact(output, build_manifest, name, target)
                for key, name in [("lib", "ai_toolbox_management"), ("owner", "owner_process")]}
        for key, executable in bins.items():
            names = inventory(run(key + "-list", [str(executable), "--list", "--test-threads=1"],
                                  15, source, env))
            require(names == manifest["inventories"][key], "exact compiled inventory mismatch: " + key)
        parent = Path("/dev/shm")
        require(parent.is_dir() and not parent.is_symlink(), "runtime tmpfs unavailable")
        magic = run("runtime-filesystem", ["stat", "-f", "-c", "%t", str(parent)], 5, source, env).decode().strip()
        require(magic == "1021994", "runtime root is not tmpfs")
        space = os.statvfs(parent)
        require(space.f_bavail * space.f_frsize >= FIXTURE_BUDGET + 16 * 1024 * 1024,
                "runtime tmpfs too full")
        FIXTURE = Path(tempfile.mkdtemp(prefix="ac-", dir=parent))
        identity = FIXTURE.lstat()
        FIXTURE_ID = (identity.st_dev, identity.st_ino)
        require(identity.st_uid == os.geteuid() and stat.S_IMODE(identity.st_mode) == 0o700,
                "fixture identity/mode invalid")
        result["runtime"] = {
            "type": magic, "mode": oct(stat.S_IMODE(identity.st_mode)), "root": str(FIXTURE),
            "identity": list(FIXTURE_ID), "sampled_budget_bytes": FIXTURE_BUDGET,
            "hard_per_file_limit": FIXTURE_FILE_LIMIT, "entry_limit": 512,
        }
        test_env = env.copy()
        test_env.update(TMPDIR=str(FIXTURE), TMP=str(FIXTURE), TEMP=str(FIXTURE),
                        SQLITE_TMPDIR=str(FIXTURE), CARGO_NET_OFFLINE="true")
        # Applies only to actual tests/actors, never compiler/linker outputs.
        launch = [sys.executable, "-c",
                  "import os,resource,sys; resource.setrlimit(resource.RLIMIT_FSIZE,(8388608,8388608)); os.execv(sys.argv[1],sys.argv[1:])"]
        output = run("lib-tests", launch + [str(bins["lib"]), "--test-threads=1", "--show-output"],
                     120, source, test_env)
        check_pass(output, 48, 0)
        output = run("owner-tests", launch + [str(bins["owner"]), "--test-threads=1", "--show-output"],
                     180, source, test_env)
        text = output.decode()
        require(len(re.findall(r"^test result: ok\. 11 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;",
                               text, re.M)) == 1, "owner pass count mismatch")
        require(len(re.findall(r"^verified actor execution: ", text, re.M)) == 12,
                "expected twelve normal actor libtest completions")
        require(re.findall(r"^verified abrupt actor execution: (.+)$", text, re.M) == ["stale"],
                "expected one deliberate abrupt actor exit")
        check_fixture_budget()
        require(not list(FIXTURE.iterdir()), "fixture leaked artifacts despite test success")
        result.update(ok=True, ordinary_passed=59, lib_passed=48, owner_parent_passed=11,
                      ignored_actor_entry=1, actual_actor_invocations=13,
                      normal_actor_test_passes=12, abrupt_actor_exits=1,
                      binaries={key: file_digest(path) for key, path in bins.items()})
    except BaseException as error:
        result["error"] = f"{type(error).__name__}: {error}"
    finally:
        FINALIZING = True
        # Source proof errors must not skip cleanup of a separately verified,
        # owned runtime root after the supervisor has reaped its descendants.
        try:
            if source is not None and source.exists() and "input_files" in result:
                final_files = {path: file_digest(source / path) for path in result["input_files"]}
                result["after_test_files"] = final_files
                require(final_files == (tested_files or result["input_files"]), "source bytes changed during validation")
                run("final-working-diff", ["git", "diff", "--exit-code"], 10, source, env)
                final_tree = run("final-tree", ["git", "write-tree"], 10, source, env).decode().strip()
                require(final_tree == (tested_tree or result["input_tree"]), "full tracked source tree changed")
                if tested_patch_sha is not None:
                    final_patch = run("final-staged-diff", ["git", "diff", "--cached", "--binary", "--full-index", BASE],
                                      10, source, env)
                    require(digest(final_patch) == tested_patch_sha, "tested staged patch changed")
                untracked = run("final-untracked", ["git", "ls-files", "--others", "--exclude-standard"],
                                10, source, env)
                require(not untracked.strip(), "untracked source files after validation")
                result["after_lock_sha256"] = file_digest(source / "crates/ai-toolbox-management/Cargo.lock")
                require(result["after_lock_sha256"] == result["input_lock_sha256"], "independent lock changed")
        except BaseException as error:
            result["ok"] = False
            result["source_proof_error"] = f"{type(error).__name__}: {error}"
        try:
            if FIXTURE is not None:
                identity = FIXTURE.lstat()
                require((identity.st_dev, identity.st_ino) == FIXTURE_ID,
                        "fixture root identity changed at cleanup")
                require(ACTIVE is None and all(not record["survivors"] for record in RECORDS),
                        "cannot remove runtime root with unknown live descendants")
                require(shutil.rmtree.avoids_symlink_attacks, "FD-safe owned-tree cleanup unavailable")
                shutil.rmtree(FIXTURE)
                result["runtime_removed"] = not FIXTURE.exists()
                require(result["runtime_removed"], "owned runtime root remains")
                FIXTURE = None
            result["cleanup_confirmed"] = ACTIVE is None and all(not record["survivors"] for record in RECORDS)
        except BaseException as error:
            result["ok"] = False
            result["cleanup_error"] = f"{type(error).__name__}: {error}"
        result["commands"] = RECORDS
        result["duration_seconds"] = round(time.monotonic() - started, 3)
        write_json("validation-result.json", result)
        print(json.dumps(result), flush=True)
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    for signum in (signal.SIGTERM, signal.SIGINT):
        signal.signal(signum, interrupted)
    sys.exit(main())
