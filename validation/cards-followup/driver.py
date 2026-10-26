"""Sealed card-fixture follow-up; combines old 43 stages with new browser evidence.

The command/termination kernel is copied unchanged from reviewed 92198. Its
ordinary owned-group cleanup is not a generation-by-generation descendant proof.
Admission is the original 6GiB available/20GiB disk gate, not continuous RSS.
"""
import ast
import zipfile
import shutil
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
REF = "refs/heads/validation/upstream-cards-followup2-20261026"
BASELINE_SHA256 = "f4668f021f9ea7354d1845c89a0f51b6b43b8b8d208abb68f69e510bf031f017"
GUARD_SHA256 = "9cf38152e91ba9b7b4aec21c87db758c886d3acb877c3017ecf7629115c94993"
MANIFEST_SHA256 = "6cc9171f3c76fe79e33390c2595eec4ebcf775071f0e5b3f6d3df4b4666b7118"
MANIFEST = json.loads((HERE / "source.json").read_text())
spec = importlib.util.spec_from_file_location("reviewed_admission", HERE / "verify-codex-usage-440.py")
guards = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guards)
STAGES = [("frontend-install", 600), ("frontend-test-web", 600), ("frontend-migrated-cards", 600)]


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
    assert files == MANIFEST["web_test_files"] and len(files) == 159
    assert json.loads((root / "package.json").read_text())["scripts"] == MANIFEST["scripts"]


def verify_web_log(content):
    for name, expected in [("tests", MANIFEST["web_test_count"]), ("pass", MANIFEST["web_test_count"]),
                           ("fail", 0), ("cancelled", 0), ("skipped", 0), ("todo", 0)]:
        values = re.findall(r"^ℹ " + name + r" (\d+)$", content, re.MULTILINE)
        assert values == [str(expected)], "Missing/duplicate/wrong web summary: " + name
    assert MANIFEST["web_test_count"] == 1199 and len(MANIFEST["regression_names"]) == 9
    passed = re.findall(r"^✔ (.+?) \([\d.]+ms\)$", content, re.MULTILINE)
    assert all(passed.count(name) == 1 for name in MANIFEST["regression_names"]), "New regression not executed exactly once"
    assert "Web test runner finished without running any tests" not in content


def verify_stages(results):
    assert [r["stage"] for r in results] == [name for name, _ in STAGES], "Missing/duplicate/out-of-order stage"
    assert all(r["passed"] is True and r["exit_code"] == 0 for r in results), "One or more stages failed"


def verify_reuse_boundary(root):
    prior = MANIFEST["prior"]
    def git(*args):
        return subprocess.check_output(["git", *args], cwd=root, timeout=5)
    assert git("rev-parse", "HEAD^^").decode().strip() == prior["product_sha"]
    assert git("rev-parse", prior["product_sha"] + "^{tree}").decode().strip() == prior["product_tree"]
    assert git("diff", "--name-only", prior["product_sha"], "HEAD").decode().splitlines() == prior["allowed_changed_paths"]
    assert digest(git("diff", "--binary", "--no-ext-diff", prior["product_sha"], "HEAD")) == prior["exact_diff_sha256"]


def verify_prior_records(plan, stages, commands):
    prior = MANIFEST["prior"]
    assert plan["driver_sha"] == prior["driver_sha"]
    assert plan["manifest"]["product_sha"] == prior["product_sha"]
    assert plan["manifest"]["product_tree"] == prior["product_tree"]
    assert [r["stage"] for r in stages] == prior["stage_names"] and len(stages) == 44
    assert [r["stage"] for r in stages if r["passed"] is not True] == [prior["failed_stage"]]
    assert all(r["exit_code"] == (1 if r["stage"] == prior["failed_stage"] else 0) for r in stages)
    assert [r["label"] for r in commands] == prior["stage_names"]
    assert all(r["timed_out_or_output_limit"] is False and r["cleanup"] == "owned group TERM/KILL sent; leader reaped"
               and r["exit_code"] == (1 if r["label"] == prior["failed_stage"] else 0) for r in commands)
    assert stages[-1]["classification"] == "passed", "Old Rust inventory classification must remain passing"
    return [r["stage"] for r in stages if r["passed"]]


def verify_prior_archive(path):
    prior = MANIFEST["prior"]
    assert path.stat().st_size == prior["bytes"] and digest(path.read_bytes()) == prior["sha256"], "Prior artifact identity changed"
    with zipfile.ZipFile(path) as archive:
        assert archive.testzip() is None
        names = archive.namelist()
        assert len(names) == len(set(names)) and all(not Path(n).is_absolute() and ".." not in Path(n).parts for n in names)
        return verify_prior_records(*(json.loads(archive.read(n)) for n in ["source-and-plan.json", "stage-results.json", "command-records.json"]))


def preserve_browser_evidence(runtime_tmp, output, deadline):
    roots = list(runtime_tmp.glob("migrated-provider-cards-*"))
    assert len(roots) <= 1, "Duplicate browser fixture attempts"
    if not roots:
        return None  # A setup failure keeps its command log, never becomes PASS.
    root = roots[0]
    assert root.is_dir() and not root.is_symlink()
    destination = output / "browser"
    destination.mkdir(exist_ok=False)
    total = 0
    paths = [p for p in root.iterdir() if p.suffix in [".json", ".png", ".html"]]
    assert len(paths) <= 100
    for source in paths:
        assert time.monotonic() < deadline - 5
        info = source.lstat()
        assert stat.S_ISREG(info.st_mode) and info.st_size <= 32 * 1024**2
        total += info.st_size
        assert total <= 128 * 1024**2
        shutil.copyfile(source, destination / source.name)
    return destination


def verify_browser_reports(root, content):
    assert root is not None, "Missing real browser evidence"
    scenarios = json.loads((root / "scenarios.json").read_text())
    routing = json.loads((root / "desktop-routing-cases.json").read_text())
    results = json.loads((root / "results.json").read_text())
    assert scenarios["completed"] == list(MANIFEST["browser_scenarios"])
    assert scenarios["assertionCounts"] == MANIFEST["browser_scenarios"]
    assert len(scenarios["checks"]) == 132 and len(set(scenarios["checks"])) == 132
    assert routing["assertionCounts"] == MANIFEST["routing_scenarios"]
    assert routing["passed"] == list(MANIFEST["routing_scenarios"]) and routing["failed"] == [] and routing["fixtureErrors"] == []
    assert routing["summary"] == {"attempted": 8, "passed": 8, "failed": 0, "passedControls": 6, "failedControls": 0,
                                   "expectedDefectFailures": 0, "unexpectedFailures": 0}
    assert [case["name"] for case in routing["cases"]] == list(MANIFEST["routing_scenarios"])
    assert all(case["status"] == "passed" and case["phase"] == "complete"
               and case["passedAssertions"] == MANIFEST["routing_scenarios"][case["name"]] for case in routing["cases"])
    assert len(routing["checks"]) == 28 and len(set(routing["checks"])) == 28
    assert results["checks"] == scenarios["checks"] + routing["checks"] and len(results["checks"]) == 160
    assert all(record["browserExceptions"] == [] for record in [scenarios, routing, results])
    for name in ["migrated-provider-cards-light.png", "migrated-provider-cards-dark.png"]:
        assert (root / name).read_bytes().startswith(b"\x89PNG\r\n\x1a\n")
    for line in ["Passed 11 migrated-provider-card scenarios (132 assertions).", "Passed 8 Desktop routing scenarios (28 assertions).", "Passed 160 browser checks."]:
        assert content.splitlines().count(line) == 1, "Missing/duplicated actual browser completion"


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
    verify_reuse_boundary(root)
    reused_stages = verify_prior_archive(output / "prior-artifact.zip")
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

    env = isolated_env(runtime_tmp / "private-profile", runtime_tmp)
    cpu = str(min(os.sched_getaffinity(0)))
    commands = [["pnpm", "install", "--frozen-lockfile"],
                ["taskset", "-c", cpu, "pnpm", "run", "test:web"], ["pnpm", "run", "test:migrated-provider-cards"]]
    results = []
    plan = {"driver_sha": expected_sha, "product_sha": MANIFEST["product_sha"], "product_tree": MANIFEST["product_tree"],
            "stages": STAGES, "commands": commands, "shared_workflow_seconds": 1380,
            "scope": "Fixture-only follow-up: install, all 1199 web tests, real 160-assertion card browser. Prior run remains FAILED; its other 43 stages are reused only across the exact sealed test/docs delta. No new Rust/build execution or Windows/native/Grok RSS acceptance.", "prior": MANIFEST["prior"], "reused_stages": reused_stages,
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
            if name == "frontend-migrated-cards":
                reports = preserve_browser_evidence(runtime_tmp, output, deadline)
                if code == 0:
                    verify_browser_reports(reports, content)
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
    verify_reuse_boundary(root)
    assert len(command_records) == 3 and all(r["exit_code"] == 0 and not r["timed_out_or_output_limit"]
        and r["cleanup"] == "owned group TERM/KILL sent; leader reaped" for r in command_records)
    assert time.monotonic() < deadline - 5


def main():
    workspace = Path(os.environ["GITHUB_WORKSPACE"]).resolve()
    output = Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    with (output / "attempt.started").open("x") as marker:
        marker.write("One sealed card-fixture follow-up attempt only\n")
    result = {"ok": False, "private_tmp_cleanup": "not_started"}
    try:
        # Original 92198 short random private TMPDIR for browser/socket fixtures.
        with tempfile.TemporaryDirectory(prefix="a440-", dir="/tmp") as name:
            private = Path(name)
            info = private.lstat()
            assert stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and stat.S_IMODE(info.st_mode) == 0o700
            assert len(os.fsencode(private)) <= 24
            runtime_tmp = private
            run_validation(runtime_tmp)
        result.update(ok=True, private_tmp_cleanup="context-owned directory removed")
    except BaseException as error:
        result["error"] = type(error).__name__ + ": " + str(error)
        result["private_tmp_cleanup"] = "context cleanup attempted; overall failed"
    (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    if result["ok"]:
        (output / "PASS.txt").write_text("Sealed fixture follow-up install/1199 web tests/160 browser checks passed, combined with exact prior 43 stages. Prior run40299947926 remains FAILED; this is not a single full green run or Windows/native acceptance.\n")
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
