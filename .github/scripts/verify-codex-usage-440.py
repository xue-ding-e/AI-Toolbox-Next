"""Manual-only synthetic RED/GREEN validation; never accepts build errors as RED."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import sys
import time

RED = "bae5a78387be2a9365c0b2bafa41fc4bfc14c65b"
GREEN = "ed843373628fed574c20d61ffa259a01bd0c60ee"
TREES = {"red": "4dfe692ec9bb03cfc57891b8571ad55740da6415", "green": "9185300968d02f9e767ee05021993b9dd97e7e85"}
PREFIX = "coding::codex::official_accounts::usage_refresh_tests::"
TEST_FILE = "tauri/src/coding/codex/official_accounts/usage_refresh_tests.rs"
FRONTEND = "web/test/features/coding/codex/utils/codexAccountRefresh.test.ts"
NEIGHBOR_CASES = [
    "coding::codex::official_accounts::tests::codex_credits_parse_strings_numbers_and_preserve_subscription_windows",
    "coding::codex::official_accounts::tests::codex_credits_preserve_zero_negative_and_conflicting_availability",
    "coding::codex::official_accounts::tests::codex_credits_malformed_fields_do_not_fail_or_invent_a_balance",
    "coding::codex::official_accounts::tests::codex_credits_round_trip_refresh_clear_and_failure_are_account_scoped",
    "coding::codex::official_accounts::tests::codex_credits_old_records_and_virtual_logins_have_no_invented_balance",
    "coding::codex::official_accounts::tests::codex_credits_preparation_failure_clears_persisted_balance",
    "coding::codex::official_accounts::tests::codex_credits_oauth_refresh_does_not_restore_an_older_usage_snapshot",
    "coding::codex::official_accounts::tests::refreshed_auth_preserves_rotated_fields_omitted_by_server",
    "coding::codex::official_accounts::tests::official_runtime_merge_preserves_unknown_runtime_fields",
    "coding::codex::official_accounts::tests::atomic_auth_writer_replaces_content_and_sets_owner_only_permissions",
    "coding::codex::official_accounts::tests::parse_usage_snapshot_non_free_uses_window_duration_before_order",
    "coding::codex::official_accounts::tests::parse_usage_snapshot_free_treats_primary_week_window_as_weekly_limit",
]
FAILURES = {
    "issue440_expired_token_rotates_and_persists_before_usage": {"line": 207, "kind": "usage_401"},
    "issue440_applied_account_adopts_newer_same_identity_runtime": {"line": 226, "kind": "usage_401"},
    "issue440_unexpired_401_forces_one_recovery_and_retry": {"line": 242, "kind": "usage_401"},
    "issue440_rotation_survives_usage_failure": {"line": 258, "kind": "persisted_tokens"},
}


def verify_business_red(name, content):
    expected = FAILURES[name]
    headers = list(re.finditer(r"panicked at ([^\n]+):(\d+):(\d+):\n", content))
    assert len(headers) == 1, "RED must have exactly one identified business panic"
    header = headers[0]
    assert header[1].endswith("src/coding/codex/official_accounts/usage_refresh_tests.rs")
    assert int(header[2]) == expected["line"], "Unexpected fixture/mock/assertion panic location"
    body = content[header.end():].split("\n\n", 1)[0]
    lines = body.splitlines()
    if expected["kind"] == "usage_401":
        assert re.fullmatch(
            r'called `Result::unwrap\(\)` on an `Err` value: "Codex usage request failed: HTTP status client error \(401 Unauthorized\) for url \(http://127\.0\.0\.1:\d+/usage\)"',
            lines[0],
        ), "Wrong RED error body; the test name is not business evidence"
    else:
        assert lines[0] == "assertion `left == right` failed"
        assert re.search(r'^  left: Object.*String\("synthetic-refresh-a-before-500"\)', body, re.MULTILINE)
        assert re.search(r'^ right: Object.*String\("synthetic-refresh-a-after-500"\)', body, re.MULTILINE)



def admit(path):
    """Owned-process/visible-v2 observation, only on the configured hosted VM.

    Never reads PID 1 or another process's namespace. This is not a generic
    hostile-container capacity proof. Unsupported layouts and denied reads stop.
    """
    def require(condition, message):
        if not condition:
            raise RuntimeError("ADMISSION_UNKNOWN: " + message)

    def record_read(label, operation):
        print("MEMORY_READ " + label, flush=True)
        try:
            return operation()
        except OSError as error:
            print(json.dumps({"observation": "read_failed", "read_class": label,
                              "error_type": type(error).__name__, "errno": error.errno}), flush=True)
            raise

    require(os.environ.get("GITHUB_ACTIONS") == "true"
            and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted"
            and os.environ.get("RUNNER_OS") == "Linux", "requires the configured standard GitHub-hosted Linux VM")
    info = record_read("proc-meminfo", lambda: Path("/proc/meminfo").read_text())
    fields = {}
    for line in info.splitlines():
        key, _, value = line.partition(":")
        if key in ("MemTotal", "MemAvailable"):
            parts = value.split()
            require(len(parts) == 2 and parts[1] == "kB" and parts[0].isdigit(), "invalid memory units")
            fields[key] = int(parts[0]) * 1024
    require(set(fields) == {"MemTotal", "MemAvailable"}, "missing host memory metrics")
    require(0 <= fields["MemAvailable"] <= fields["MemTotal"], "invalid available memory")
    cgroup = record_read("proc-self-cgroup", lambda: Path("/proc/self/cgroup").read_text())
    mounts = record_read("proc-self-mountinfo", lambda: Path("/proc/self/mountinfo").read_text())
    namespace = record_read("proc-self-cgroup-namespace", lambda: os.readlink("/proc/self/ns/cgroup"))
    require(bool(re.fullmatch(r"cgroup:\[[0-9]+\]", namespace)), "unknown own namespace")
    entries = cgroup.splitlines()
    require(len(entries) == 1 and entries[0].startswith("0::"), "v1/mixed cgroups unsupported")
    relative = entries[0][3:]

    def safe_absolute(value):
        require(value.startswith("/") and "\\" not in value and "\0" not in value,
                "unsupported path encoding")
        require(value == "/" or all(part not in ("", ".", "..") for part in value[1:].split("/")), "unsafe cgroup path")
        return value

    safe_absolute(relative)
    found, points = [], []
    for line in mounts.splitlines():
        left, separator, right = line.partition(" - ")
        require(bool(separator), "invalid mountinfo")
        columns = left.split()
        require(len(columns) >= 6, "invalid mount columns")
        kind = right.split()[0]
        require(kind != "cgroup", "mixed/v1 hierarchy unsupported")
        points.append(safe_absolute(columns[4]))
        if kind == "cgroup2":
            require(columns[3] == "/", "hidden cgroup ancestors")
            found.append(safe_absolute(columns[4]))
    require(len(found) == 1, "ambiguous or missing v2 mount")
    mount = found[0]
    require(not any(point.startswith(mount + "/") for point in points), "nested mount hides ancestry")
    require(str(Path(mount).resolve(strict=True)) == mount, "cgroup mount alias")
    components = relative.strip("/").split("/") if relative != "/" else []
    require(len(components) <= 128, "cgroup hierarchy too deep")
    descriptor = record_read("cgroup-v2-root-directory", lambda: os.open(mount, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW))
    records = []

    def control(fd, name, depth, maximum=128):
        label = f"cgroup-v2-depth-{depth}-{name}"
        def read():
            child = os.open(name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
            try:
                require(stat.S_ISREG(os.fstat(child).st_mode), "nonregular control")
                data = os.read(child, maximum + 1)
                require(len(data) <= maximum, "oversized control")
                return data.decode("ascii").strip()
            finally:
                os.close(child)
        return record_read(label, read)

    try:
        for depth in range(len(components) + 1):
            before = os.fstat(descriptor)
            if depth == 0:
                require("memory" in control(descriptor, "cgroup.controllers", depth).split(), "memory controller unavailable")
                root_stat = control(descriptor, "memory.stat", depth, 65536)
                names = set()
                for line in root_stat.splitlines():
                    parts = line.split()
                    require(len(parts) == 2 and bool(re.fullmatch(r"[a-z][a-z0-9_]*", parts[0])) and parts[1].isdigit(), "invalid root memory.stat")
                    require(parts[0] not in names, "duplicate root memory.stat field")
                    names.add(parts[0])
                require({"anon", "file", "kernel_stack", "pagetables"} <= names, "incomplete root memory.stat")
                # On this ordinary VM, positive memory.stat plus BOTH scalar
                # controls absent identifies the real kernel root, not a hidden
                # namespaced child. An EACCES is never treated as absence.
                for name in ("memory.max", "memory.current"):
                    try:
                        control(descriptor, name, depth)
                    except FileNotFoundError:
                        pass
                    else:
                        raise RuntimeError("ADMISSION_UNKNOWN: root scalar controls imply incomplete ancestry")
                records.append({"depth": depth, "limit": None, "current": None,
                                "root": "positive memory.stat and exact ENOENT scalar controls"})
            else:
                maximum = control(descriptor, "memory.max", depth)
                current = control(descriptor, "memory.current", depth)
                require(maximum == "max" or maximum.isdigit(), "invalid memory.max")
                require(current.isdigit(), "invalid memory.current")
                records.append({"depth": depth, "limit": None if maximum == "max" else int(maximum), "current": int(current)})
            after = os.fstat(descriptor)
            require((before.st_dev, before.st_ino) == (after.st_dev, after.st_ino), "cgroup changed during observation")
            if depth < len(components):
                child = record_read(f"cgroup-v2-descend-{depth + 1}", lambda: os.open(components[depth], os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor))
                os.close(descriptor)
                descriptor = child
    finally:
        os.close(descriptor)
    require(record_read("proc-self-cgroup-recheck", lambda: Path("/proc/self/cgroup").read_text()) == cgroup, "process migrated")
    require(record_read("proc-self-mountinfo-recheck", lambda: Path("/proc/self/mountinfo").read_text()) == mounts, "mounts changed")
    require(record_read("proc-self-namespace-recheck", lambda: os.readlink("/proc/self/ns/cgroup")) == namespace, "namespace changed")
    finite = [item for item in records if item["limit"] is not None]
    available = min([fields["MemAvailable"]] + [max(0, item["limit"] - item["current"]) for item in finite])
    capacity = min([fields["MemTotal"]] + [item["limit"] for item in finite])
    disk = shutil.disk_usage(path).free
    observation = {"host_available_bytes": fields["MemAvailable"], "effective_capacity_bytes": capacity,
                   "effective_available_bytes": available, "disk_free_bytes": disk,
                   "visible_ancestor_limits": records}
    print(json.dumps(observation), flush=True)
    require(available >= 6 * 1024**3, "less than 6 GiB effective available memory")
    require(disk >= 20 * 1024**3, "less than 20 GiB available disk")
    return observation


def bodies(source):
    found = {}
    for match in re.finditer(r"async fn (issue440_\w+)\(\) \{", source):
        index, depth = match.end(), 1
        while depth:
            depth += (source[index] == "{") - (source[index] == "}")
            index += 1
        found[match[1]] = source[match.start():index]
    return found


def main():
    expected_sha = sys.argv[3]
    assert re.fullmatch(r"[0-9a-f]{40}", expected_sha), "An exact approved validation SHA is required"
    driver_root = Path(__file__).resolve().parents[2]
    assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=driver_root, text=True).strip() == expected_sha, "Driver checkout differs from approved validation SHA"
    assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=driver_root, text=True).strip(), "Driver source is dirty"
    base = Path(sys.argv[1]).resolve()
    output = Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    deadline = time.monotonic() + 40 * 60
    identities = {}
    command_records = []
    cancellation = {"spawning": False, "pending": None}

    def terminate(signum, _frame):
        if cancellation["spawning"]:
            cancellation["pending"] = signum
            return
        raise InterruptedError(f"Validation interrupted by signal {signum}")

    signal.signal(signal.SIGTERM, terminate)
    signal.signal(signal.SIGINT, terminate)

    def run(args, cwd, name, timeout=120, env=None):
        remaining = int(deadline - time.monotonic())
        assert remaining > 5, "Validation's process budget reached the cleanup reserve"
        print(f"RUN {name}", flush=True)
        command_deadline = time.monotonic() + min(timeout, remaining) - 5
        path = output / f"{name}.log"
        child = None
        timed_out = False
        with path.open("w") as stream:
            try:
                # Defer the parent's cancellation exception until the child
                # object is owned; do not give children a blocked signal mask.
                cancellation["spawning"] = True
                try:
                    child = subprocess.Popen(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                             stdout=stream, stderr=subprocess.STDOUT,
                                             start_new_session=True)
                finally:
                    cancellation["spawning"] = False
                if cancellation["pending"] is not None:
                    raise InterruptedError("Validation interrupted during child creation")
                # WNOWAIT preserves the leader PID until group cleanup, avoiding
                # signaling a recycled numeric process group after a reap.
                while os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None:
                    if time.monotonic() >= command_deadline or path.stat().st_size > 128 * 1024**2:
                        timed_out = True
                        break
                    time.sleep(0.1)
                timed_out |= path.stat().st_size > 128 * 1024**2
            finally:
                if child is not None:
                    # Standard runner teardown is the outer boundary. The owned
                    # session additionally contains ordinary compiler/test
                    # descendants, not deliberately escaping hostile daemons.
                    previous_handlers = {sig: signal.signal(sig, signal.SIG_IGN)
                                         for sig in (signal.SIGINT, signal.SIGTERM)}
                    record = {"label": name, "pid": child.pid, "timed_out_or_output_limit": timed_out,
                              "cleanup": "pending", "exit_code": None}
                    try:
                        for sig, seconds in [(signal.SIGTERM, 1), (signal.SIGKILL, 1)]:
                            try:
                                os.killpg(child.pid, sig)
                            except ProcessLookupError:
                                pass
                            time.sleep(seconds)
                        code = child.wait(timeout=3)
                        record.update(cleanup="owned group TERM/KILL sent; leader reaped", exit_code=code)
                    finally:
                        command_records.append(record)
                        (output / "command-records.json").write_text(json.dumps(command_records, indent=2) + "\n")
                        for sig, handler in previous_handlers.items():
                            signal.signal(sig, handler)
        if timed_out:
            raise RuntimeError(f"Process time/output budget exceeded: {name}; owned group terminated and leader reaped")
        content = path.read_text()
        print(f"RESULT {name}: {code}", flush=True)
        if code:
            print(content[-10000:], flush=True)
        return code, content

    sources = {}
    for variant, sha in [("red", RED), ("green", GREEN)]:
        root = base / variant
        assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip() == sha
        assert subprocess.check_output(["git", "rev-parse", "HEAD^{tree}"], cwd=root, text=True).strip() == TREES[variant]
        assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True).strip()
        sources[variant] = bodies((root / TEST_FILE).read_text())
    assert len(sources["red"]) == 6 and len(sources["green"]) == 27
    for name, body in sources["red"].items():
        assert sources["green"][name] == body, f"Shared regression body differs: {name}"
    assert (base / "red" / FRONTEND).read_bytes() == (base / "green" / FRONTEND).read_bytes()
    (output / "case-manifest.json").write_text(json.dumps({
        "source_commits": {"red": RED, "green": GREEN}, "source_trees": TREES,
        "cases": {variant: sorted(cases) for variant, cases in sources.items()},
        "shared_body_sha256": {name: hashlib.sha256(body.encode()).hexdigest()
                               for name, body in sources["red"].items()},
        "red_expected_business_failures": FAILURES,
        "green_existing_regressions": NEIGHBOR_CASES,
    }, indent=2) + "\n")

    for variant in ["red", "green"]:
        root = base / variant
        # Keep Cargo's real toolchain environment only for compilation. Test
        # processes receive synthetic home/config roots, never runner profiles.
        isolated = output / f"{variant}-synthetic-profile"
        isolated.mkdir()
        test_env = {name: os.environ[name] for name in ["PATH", "LANG", "LC_ALL", "TZ", "LD_LIBRARY_PATH"] if name in os.environ}
        test_env["RUST_BACKTRACE"] = "0"
        for name in ["HOME", "USERPROFILE", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "CODEX_HOME"]:
            directory = isolated / name.lower()
            directory.mkdir()
            test_env[name] = str(directory)
        test_env["TMPDIR"] = str(isolated)
        code, content = run(["node", "--test", "--test-reporter=tap", FRONTEND], root, f"{variant}-frontend", env=test_env)
        assert "# tests 6" in content
        if variant == "red":
            assert code == 1 and "# pass 5" in content and "# fail 1" in content
            assert "false !== true" in content, "RED must be the metadata attribution assertion"
            assert "not ok 5 - saved-account refresh accepts corrected credential metadata without changing record identity" in content.splitlines()
            assert "codexAccountRefresh.test.ts:76:10)" in content
            assert "code: 'ERR_ASSERTION'" in content
        else:
            assert code == 0 and "# pass 6" in content and "# fail 0" in content
        # Shared target caches dependencies, never a previous tree's own crate.
        code, _ = run(["cargo", "clean", "--manifest-path", "tauri/Cargo.toml", "-p", "ai-toolbox"], root, f"{variant}-clean-package")
        assert code == 0
        code, cargo_json = run(["cargo", "test", "--locked", "--manifest-path", "tauri/Cargo.toml", "--lib", "--no-run", "--message-format=json"], root, f"{variant}-compile", timeout=1800)
        assert code == 0, f"{variant} compilation failed; not a business RED"
        artifacts = []
        for line in cargo_json.splitlines():
            try:
                item = json.loads(line)
            except ValueError:
                continue
            if item.get("reason") == "compiler-artifact" and item.get("profile", {}).get("test") and item.get("target", {}).get("name") == "ai_toolbox_lib" and item.get("executable"):
                assert not item.get("fresh"), "Own crate was reused instead of compiled from this tree"
                artifacts.append(item["executable"])
        assert len(set(artifacts)) == 1, "Expected exactly one freshly built library test executable"
        executable = output / f"{variant}-tests"
        shutil.copy2(artifacts[0], executable)
        digest = hashlib.sha256(executable.read_bytes()).hexdigest()
        identities[variant] = {"source_sha": RED if variant == "red" else GREEN,
                               "source_tree": TREES[variant], "binary_sha256": digest}
        (output / "binary-identities.json").write_text(json.dumps(identities, indent=2) + "\n")
        code, listing = run([str(executable), "issue440_", "--list"], root, f"{variant}-list", env=test_env)
        assert code == 0
        names = sorted(line.removesuffix(": test") for line in listing.splitlines() if line.startswith(PREFIX) and line.endswith(": test"))
        assert names == sorted(PREFIX + name for name in sources[variant]), "Unexpected or missing regression selection"
        for name in sorted(sources[variant]):
            code, content = run([str(executable), "--exact", PREFIX + name, "--nocapture", "--test-threads=1"], root, f"{variant}-{name}", timeout=30, env=test_env)
            if variant == "red" and name in FAILURES:
                assert code == 101 and "test result: FAILED. 0 passed; 1 failed;" in content
                verify_business_red(name, content)
            else:
                assert code == 0 and "test result: ok. 1 passed; 0 failed;" in content, f"Unexpected result: {variant} {name}"
        if variant == "green":
            code, listing = run([str(executable), "coding::codex::official_accounts::tests::", "--list"], root, "green-existing-list", env=test_env)
            assert code == 0
            for name in NEIGHBOR_CASES:
                assert name + ": test" in listing.splitlines(), f"Missing existing regression: {name}"
                code, content = run([str(executable), "--exact", name, "--nocapture", "--test-threads=1"], root, "green-existing-" + name.rsplit("::", 1)[1], timeout=30, env=test_env)
                assert code == 0 and "test result: ok. 1 passed; 0 failed;" in content, f"Existing regression failed: {name}"
        # Only logs/identities are published, never executables or profiles.
        executable.unlink()
    assert identities["red"]["binary_sha256"] != identities["green"]["binary_sha256"]
    (output / "PASS.txt").write_text("RED: 2 controls pass, 4 business assertions fail. GREEN: 27 new and 12 existing Rust tests pass. Frontend: 5/1 RED, 6/0 GREEN. Native UI and Windows/UNC are not validated.\n")
    print("SEALED SYNTHETIC RED/GREEN VALIDATION PASSED", flush=True)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--admission":
        admit(sys.argv[2])
    else:
        main()
