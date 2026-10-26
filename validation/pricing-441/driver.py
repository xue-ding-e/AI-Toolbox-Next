"""One sealed #441 frontend run, not Grok RSS or browser/native acceptance.

The command/termination kernel is copied unchanged from reviewed 92198. Its
ordinary owned-group cleanup is not a generation-by-generation descendant proof.
Admission is the original 6GiB available/20GiB disk gate, not continuous RSS.
"""
import ast
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import tempfile
import time

if not __debug__:
    raise RuntimeError("Optimized Python is unsupported: validation assertions must remain enabled")

HERE = Path(__file__).resolve().parent
REF = "refs/heads/validation/gateway-pricing-441-frontend-20261026"
BASELINE_SHA256 = "f4668f021f9ea7354d1845c89a0f51b6b43b8b8d208abb68f69e510bf031f017"
GUARD_SHA256 = "9cf38152e91ba9b7b4aec21c87db758c886d3acb877c3017ecf7629115c94993"
MANIFEST_SHA256 = "90c7dc82cb8a62fb0f236416d752e749d08c584bb51273fa8756a011f9142d17"
MANIFEST = json.loads((HERE / "source.json").read_text())
spec = importlib.util.spec_from_file_location("reviewed_admission", HERE / "verify-codex-usage-440.py")
guards = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guards)
STAGES = [("frontend-install", 600), ("frontend-tsc", 300), ("frontend-test-web", 600), ("frontend-build", 600)]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def verify_reused_kernel(source=None):
    baseline = (HERE / "reviewed-92198-full.py.txt").read_bytes()
    assert digest(baseline) == BASELINE_SHA256, "Audited baseline changed"
    assert digest((HERE / "verify-codex-usage-440.py").read_bytes()) == GUARD_SHA256, "Admission implementation changed"
    trees = [ast.parse(baseline), ast.parse(source if source is not None else Path(__file__).read_text())]
    parents = []
    for tree in trees:
        found = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "run_validation"]
        assert len(found) == 1
        parents.append(found[0])
    for name in ["run", "terminate"]:
        definitions = [[n for n in parent.body if isinstance(n, ast.FunctionDef) and n.name == name] for parent in parents]
        assert all(len(items) == 1 for items in definitions)
        assert ast.dump(definitions[0][0]) == ast.dump(definitions[1][0]), "Reviewed kernel changed: " + name
    # The unchanged functions must still close over the same cancellation and
    # command-record state, not a shadow object or a per-command reset.
    for name in ["cancellation", "command_records"]:
        assignments = [[n for n in parent.body if isinstance(n, ast.Assign)
                        and any(isinstance(t, ast.Name) and t.id == name for t in n.targets)] for parent in parents]
        assert all(len(items) == 1 for items in assignments)
        assert ast.dump(assignments[0][0]) == ast.dump(assignments[1][0]), "Kernel closure binding changed"
    registrations = [[n for n in parent.body if isinstance(n, ast.Expr) and isinstance(n.value, ast.Call)
                      and ast.unparse(n.value.func) == "signal.signal"] for parent in parents]
    assert all(len(items) == 2 for items in registrations)
    assert [ast.dump(n) for n in registrations[0]] == [ast.dump(n) for n in registrations[1]], "Cancellation registration changed"


def shared_deadline(epoch, wall, monotonic):
    remaining = min(1380, int(epoch) - wall)
    assert remaining > 5, "Shared workflow budget exhausted"
    return monotonic + remaining


def isolated_env(profile, runtime_tmp):
    # Same 92198 allowlist and HOME alias. Omit only unused Rust tool/target
    # overrides rather than inventing tool paths or invoking rustup.
    env = {key: os.environ[key] for key in ["PATH", "LANG", "LC_ALL", "TZ", "LD_LIBRARY_PATH"] if key in os.environ}
    profile.mkdir(parents=True, exist_ok=False)
    for key in ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "CODEX_HOME", "CARGO_HOME"]:
        directory = profile / key.lower()
        directory.mkdir()
        env[key] = str(directory)
    env["USERPROFILE"] = env["HOME"]
    env["TMPDIR"] = str(runtime_tmp)
    env.update(CI="true", HUSKY="0", npm_config_userconfig="/dev/null", npm_config_globalconfig="/dev/null")
    return env


def verify_source(root):
    assert digest((HERE / "source.json").read_bytes()) == MANIFEST_SHA256, "Source manifest changed"
    for expression, expected in [("HEAD", MANIFEST["product_sha"]), ("HEAD^{tree}", MANIFEST["product_tree"])]:
        assert subprocess.check_output(["git", "rev-parse", expression], cwd=root, text=True, timeout=5).strip() == expected
    assert not subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=root, text=True, timeout=5).strip()
    tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=root, timeout=5).decode().split("\0")[:-1]
    assert set(tracked) == set(MANIFEST["files"]) and len(tracked) == len(MANIFEST["files"])
    for name, expected in MANIFEST["files"].items():
        relative = Path(name)
        assert not relative.is_absolute() and ".." not in relative.parts
        path = root / relative
        info = path.lstat()
        assert stat.S_IMODE(info.st_mode) == expected["mode"]
        if expected["kind"] == "symlink":
            assert stat.S_ISLNK(info.st_mode)
            data = os.readlink(path).encode()
        else:
            assert expected["kind"] == "file" and stat.S_ISREG(info.st_mode)
            data = path.read_bytes()
        assert digest(data) == expected["sha256"], "Tracked source changed: " + name
    files = sorted(str(p.relative_to(root)) for p in (root / "web/test").rglob("*")
                   if p.is_file() and p.name.endswith((".test.ts", ".spec.ts")))
    assert files == MANIFEST["web_test_files"] and len(files) == 151
    assert json.loads((root / "package.json").read_text())["scripts"] == MANIFEST["scripts"]


def verify_web_log(content):
    for name, expected in [("tests", MANIFEST["web_test_count"]), ("pass", MANIFEST["web_test_count"]),
                           ("fail", 0), ("cancelled", 0), ("skipped", 0), ("todo", 0)]:
        values = re.findall(r"^ℹ " + name + r" (\d+)$", content, re.MULTILINE)
        assert values == [str(expected)], "Missing/duplicate/wrong web summary: " + name
    assert MANIFEST["web_test_count"] == 1152 and len(MANIFEST["regression_names"]) == 9
    passed = re.findall(r"^✔ (.+?) \([\d.]+ms\)$", content, re.MULTILINE)
    assert all(passed.count(name) == 1 for name in MANIFEST["regression_names"]), "New regression not executed exactly once"
    assert "Web test runner finished without running any tests" not in content


def verify_stages(results):
    assert [r["stage"] for r in results] == [name for name, _ in STAGES], "Missing/duplicate/out-of-order stage"
    assert all(r["passed"] is True and r["exit_code"] == 0 for r in results), "One or more stages failed"


def dist_inventory(root, deadline):
    dist = root / "dist"
    assert dist.is_dir() and not dist.is_symlink(), "Missing production dist"
    entries = []
    for parent, directories, files in os.walk(dist, followlinks=False):
        for name in directories:
            assert not (Path(parent) / name).is_symlink(), "Unexpected output directory link"
        for name in sorted(files):
            assert time.monotonic() < deadline - 5, "Evidence budget exhausted"
            path = Path(parent) / name
            info = path.lstat()
            assert stat.S_ISREG(info.st_mode), "Unexpected output type"
            hasher = hashlib.sha256()
            with path.open("rb") as stream:
                for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                    assert time.monotonic() < deadline - 5
                    hasher.update(chunk)
            after = path.lstat()
            assert (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns) == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
            entries.append({"path": str(path.relative_to(dist)), "bytes": info.st_size, "sha256": hasher.hexdigest()})
            assert len(entries) <= 10000, "Unexpected build output inventory"
    assert any(e["path"] == "index.html" and e["bytes"] > 0 for e in entries)
    assert any(e["path"].startswith("assets/") and e["path"].endswith(".js") and e["bytes"] > 0 for e in entries)
    return entries


def run_validation(runtime_tmp):
    root, output, expected_sha = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve(), sys.argv[3]
    assert re.fullmatch(r"[0-9a-f]{40}", expected_sha)
    driver_root = HERE.parents[1]
    assert os.environ.get("GITHUB_REPOSITORY") == "xue-ding-e/AI-Toolbox-Next"
    assert os.environ.get("GITHUB_REF") == REF and os.environ.get("GITHUB_SHA") == expected_sha
    assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=driver_root, text=True, timeout=5).strip() == expected_sha
    assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=driver_root, text=True, timeout=5).strip()
    verify_reused_kernel()
    verify_source(root)
    output.mkdir(parents=True, exist_ok=True)
    deadline = shared_deadline(os.environ["SHARED_DEADLINE_EPOCH"], time.time(), time.monotonic())
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

    env = isolated_env(runtime_tmp.parent / "profile", runtime_tmp)
    cpu = str(min(os.sched_getaffinity(0)))
    commands = [["pnpm", "install", "--frozen-lockfile"], ["pnpm", "exec", "tsc", "--noEmit"],
                ["taskset", "-c", cpu, "pnpm", "run", "test:web"], ["pnpm", "build"]]
    results = []
    plan = {"driver_sha": expected_sha, "product_sha": MANIFEST["product_sha"], "product_tree": MANIFEST["product_tree"],
            "stages": STAGES, "commands": commands, "shared_workflow_seconds": 1380,
            "scope": "Only install, tsc, complete test:web and production Vite build. No browser/Rust/native acceptance or Grok RSS claim.",
            "cleanup_assurance": "Original ordinary owned group TERM/KILL and leader reap; not per-generation all-descendant absence.",
            "dependency_seal": "Fresh frozen install; no promise that normal Vite .vite-temp metadata remains unchanged."}
    (output / "source-and-plan.json").write_text(json.dumps(plan, indent=2) + "\n")
    for (name, budget), command in zip(STAGES, commands):
        assert time.monotonic() < deadline - 5, "Shared budget exhausted before admission"
        guards.admit(root)
        guards.admit(runtime_tmp)
        entry = {"stage": name, "passed": False, "exit_code": None}
        results.append(entry)
        try:
            code, content = run(command, root, name, timeout=budget, env=env)
            entry["exit_code"] = code
            entry["passed"] = code == 0
            if code == 0 and name == "frontend-test-web":
                verify_web_log(content)
            if code == 0 and name == "frontend-build":
                (output / "dist-inventory.json").write_text(json.dumps(dist_inventory(root, deadline), indent=2) + "\n")
            if code != 0 and name == "frontend-install":
                raise RuntimeError("Dependency installation failed; downstream stages not executed")
        except BaseException as error:
            entry["passed"] = False
            entry["error"] = type(error).__name__ + ": " + str(error)
            raise
        finally:
            (output / "stage-results.json").write_text(json.dumps(results, indent=2) + "\n")
    verify_source(root)
    (output / "source-final.json").write_text(json.dumps({"product_sha": MANIFEST["product_sha"],
        "product_tree": MANIFEST["product_tree"], "tracked_files": len(MANIFEST["files"]), "status": "unchanged"}, indent=2) + "\n")
    verify_stages(results)
    assert len(command_records) == 4 and all(r["exit_code"] == 0 and not r["timed_out_or_output_limit"]
        and r["cleanup"] == "owned group TERM/KILL sent; leader reaped" for r in command_records)
    assert time.monotonic() < deadline - 5


def main():
    workspace = Path(os.environ["GITHUB_WORKSPACE"]).resolve()
    output = Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    with (output / "attempt.started").open("x") as marker:
        marker.write("One sealed frontend attempt only\n")
    result = {"ok": False, "private_tmp_cleanup": "not_started"}
    try:
        # Same context-owned cleanup model as 92198, under this job's workspace;
        # no browser/Unix socket path constraint or user's profile is involved.
        with tempfile.TemporaryDirectory(prefix="a441-", dir=workspace) as name:
            private = Path(name)
            info = private.lstat()
            assert stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and stat.S_IMODE(info.st_mode) == 0o700
            runtime_tmp = private / "tmp"
            runtime_tmp.mkdir(mode=0o700)
            run_validation(runtime_tmp)
        result.update(ok=True, private_tmp_cleanup="context-owned directory removed")
    except BaseException as error:
        result["error"] = type(error).__name__ + ": " + str(error)
        result["private_tmp_cleanup"] = "context cleanup attempted; overall failed"
    (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    if result["ok"]:
        (output / "PASS.txt").write_text("Sealed #441 frontend install/tsc/1152 web tests/Vite build passed. Not browser, Rust, native or Grok RSS acceptance.\n")
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
