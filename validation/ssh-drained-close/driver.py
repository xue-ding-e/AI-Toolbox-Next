"""One sealed SSH coverage run, not a product fix/full Rust/native/Grok acceptance.

The command/termination kernel is copied unchanged from reviewed 92198. Its
ordinary owned-group cleanup is not a generation-by-generation descendant proof.
Admission is the original 6GiB available/20GiB disk gate, not continuous RSS.
"""
import ast
from collections import Counter
from contextlib import contextmanager
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
REF = "refs/heads/validation/ssh-drained-close-20261026"
BASELINE_SHA256 = "f4668f021f9ea7354d1845c89a0f51b6b43b8b8d208abb68f69e510bf031f017"
GUARD_SHA256 = "9cf38152e91ba9b7b4aec21c87db758c886d3acb877c3017ecf7629115c94993"
MANIFEST_SHA256 = "d4f755accb06a703113462128d9dd7c447d06c6c9232c7d5ffb461747dc9e2c8"
MANIFEST = json.loads((HERE / "source.json").read_text())
spec = importlib.util.spec_from_file_location("reviewed_admission", HERE / "verify-codex-usage-440.py")
guards = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guards)
STAGES = [("rust-lib-compile", 1800), ("new-case-inventory", 120), ("new-case", 180),
          ("probe-inventory", 120), ("probe", 180), ("gate-inventory", 120), ("gate", 180)]


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

    for name in ["short_private_tmpdir", "isolated_env", "fresh_lib_artifact", "verify_focused_inventory", "classify_focused"]:
        found = [[n for n in t.body if isinstance(n, ast.FunctionDef) and n.name == name] for t in trees]
        assert all(len(f) == 1 for f in found)
        assert ast.dump(found[0][0]) == ast.dump(found[1][0]), "Reviewed helper changed: " + name
    # Keep the exact nonzero failure branches. The focused classifier never
    # calls this helper with a zero exit, so exclude unused full-suite success.
    old = next(n for n in trees[0].body if isinstance(n, ast.FunctionDef) and n.name == "classify_rust")
    new = next(n for n in trees[1].body if isinstance(n, ast.FunctionDef) and n.name == "classify_rust")
    assert ast.dump(new.body[0]) == ast.dump(ast.parse("assert code != 0").body[0])
    assert [ast.dump(n) for n in old.body[1:]] == [ast.dump(n) for n in new.body[1:]]


def shared_deadline(epoch, wall, monotonic):
    remaining = min(2100, int(epoch) - wall)
    assert remaining > 5, "Shared workflow budget exhausted"
    return monotonic + remaining


def isolated_env(profile, tool_paths, runtime_tmp):
    # Allowlist rather than deleting a few known credential variable names.
    env = {key: os.environ[key] for key in ["PATH", "LANG", "LC_ALL", "TZ", "LD_LIBRARY_PATH"] if key in os.environ}
    profile.mkdir(parents=True, exist_ok=False)
    for key in ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "CODEX_HOME", "CARGO_HOME"]:
        directory = profile / key.lower()
        directory.mkdir()
        env[key] = str(directory)
    # Cross-platform resolvers may prefer USERPROFILE while dirs::home_dir uses
    # HOME on Linux. Both must identify this same newly-created synthetic home,
    # not two artificial users, and neither may inherit the runner's profile.
    env["USERPROFILE"] = env["HOME"]
    env["TMPDIR"] = str(runtime_tmp)
    env.update(RUSTC=tool_paths["rustc"], RUSTDOC=tool_paths["rustdoc"], CARGO_TARGET_DIR=str(profile / "target"),
               CARGO_BUILD_JOBS="2", CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_TEST_DEBUG="0", CARGO_INCREMENTAL="0",
               RUST_BACKTRACE="0", CI="true", HUSKY="0")
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

    for key, count in [("probe", 13), ("gate", 3)]:
        selection = MANIFEST["focused"][key]
        source = (root / selection["source_path"]).read_text()
        assert source.count(selection["source_section"]) == 1
        section = source.split(selection["source_section"], 1)[1]
        names = re.findall(r"#\[(?:tokio::)?test(?:\([^\n]*\))?\]\s+(?:async\s+)?fn\s+(\w+)", section)
        assert len(names) == count and Counter(selection["prefix"] + n for n in names) == Counter(selection["tests"])
    assert MANIFEST["new_test"] in MANIFEST["focused"]["probe"]["tests"]
    assert len(set(MANIFEST["focused"]["probe"]["tests"] + MANIFEST["focused"]["gate"]["tests"])) == MANIFEST["distinct_test_count"] == 16




def verify_stages(results):
    assert [r["stage"] for r in results] == [name for name, _ in STAGES], "Missing/duplicate/out-of-order stage"
    assert all(r["passed"] is True and r["exit_code"] == 0 for r in results), "One or more stages failed"




@contextmanager
def short_private_tmpdir():
    # Linux filesystem sockets have a short sockaddr_un path limit. Keep the
    # scratch root independent of the long checkout/profile path, but never use
    # a shared fixed directory or a user's profile. TemporaryDirectory owns only
    # this randomly-created 0700 directory and removes it after child cleanup.
    with tempfile.TemporaryDirectory(prefix="a440-", dir="/tmp") as name:
        root = Path(name)
        info = root.lstat()
        assert root.is_dir() and not root.is_symlink()
        assert info.st_uid == os.getuid() and info.st_mode & 0o777 == 0o700
        assert len(os.fsencode(root)) <= 24, "Private TMPDIR leaves insufficient Unix socket path budget"
        yield root

def classify_rust(code, content, inventory):
    assert code != 0
    if "test result: FAILED" in content:
        return "test-failure"
    # Cargo appends `error: test failed` after a real failed test binary; it is
    # not a compiler diagnostic. Preserve runtime failure evidence above.
    if re.search(r"^error(?:\[|:)|could not compile", content, re.MULTILINE):
        return "compile-or-doc-compile-failure"
    return "environment-or-unclassified-failure"

def verify_focused_inventory(listing, expected):
    names = [line.removesuffix(": test") for line in listing.splitlines() if line.endswith(": test")]
    assert expected and len(set(expected)) == len(expected), "Empty/duplicate expected focused inventory"
    assert Counter(names) == Counter(expected), "Focused runtime identities differ from the sealed inventory"

def classify_focused(code, content, expected):
    if code:
        # Keep the reviewed distinction: compiler/setup failure is not a failed assertion.
        return classify_rust(code, content, [])
    assert expected and len(set(expected)) == len(expected), "Empty/duplicate expected focused inventory"
    assert not re.search(r"test result: FAILED|^error(?:\[|:)", content, re.MULTILINE)
    observed = re.findall(r"^test (.+?) \.{3} (ok|ignored(?:,.*)?|FAILED)$", content, re.MULTILINE)
    assert Counter(name for name, _ in observed) == Counter(expected), "Focused execution omitted or added identities"
    assert all(status == "ok" for _, status in observed), "Focused tests may not be ignored"
    summaries = re.findall(r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", content, re.MULTILINE)
    assert len(summaries) == 1, "Expected exactly one focused binary summary"
    assert [int(value) for value in summaries[0][:4]] == [len(expected), 0, 0, 0], "Incomplete/zero focused execution"
    return "passed"

def fresh_lib_artifact(content):
    found = []
    for line in content.splitlines():
        try:
            item = json.loads(line)
        except ValueError:
            continue
        if (item.get("reason") == "compiler-artifact" and item.get("profile", {}).get("test")
                and item.get("target", {}).get("name") == "ai_toolbox_lib" and item.get("executable")):
            assert not item.get("fresh"), "Own test library unexpectedly reused"
            path = Path(item["executable"])
            found.append({"binary": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
    assert len(found) == 1, "Expected exactly one freshly built production lib-test binary"
    return found[0]


def run_validation(runtime_tmp, private):
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

    tools = {name: subprocess.check_output(["rustup", "which", name], text=True, timeout=10).strip() for name in ["cargo", "rustc", "rustdoc"]}
    assert all(Path(path).is_file() for path in tools.values())
    env = isolated_env(private / "profile", tools, runtime_tmp)
    results = []
    plan = {"driver_sha": expected_sha, "product_sha": MANIFEST["product_sha"], "product_tree": MANIFEST["product_tree"],
            "stages": STAGES, "shared_workflow_seconds": 2100, "tools": tools, "distinct_tests": 16,
            "scope": "One fresh actual Tauri lib-test binary: probe 13 + gate 3. The new case also runs separately, not a seventeenth distinct test. No frontend/Grok/full Rust/docs/real SSH/native acceptance.",
            "cleanup_assurance": "Reviewed ordinary owned group TERM/KILL and leader reap, then owned profile/TMP deletion; not hostile escaped-descendant containment."}
    (output / "source-and-plan.json").write_text(json.dumps(plan, indent=2) + "\n")

    def checked(command, label, expected=None, listing=False):
        assert time.monotonic() < deadline - 5
        guards.admit(root)
        guards.admit(runtime_tmp)
        entry = {"stage": label, "passed": False, "exit_code": None}
        results.append(entry)
        try:
            code, content = run(command, root, label, timeout=dict(STAGES)[label], env=env)
            entry["exit_code"] = code
            if expected is None:
                entry["classification"] = "compiled" if code == 0 else classify_rust(code, content, [])
                assert code == 0, "Compilation/setup failed, not a test assertion"
            elif listing:
                assert code == 0, "Inventory command failed"
                verify_focused_inventory(content, expected)
                entry["classification"] = "inventory-not-execution"
            else:
                entry["classification"] = classify_focused(code, content, expected)
                assert entry["classification"] == "passed", "Focused test execution failed"
                entry["expected_identities"] = expected
                entry["summaries"] = re.findall(r"^test result: .*", content, re.MULTILINE)
            entry["passed"] = True
            return content
        except BaseException as error:
            entry["error"] = type(error).__name__ + ": " + str(error)
            raise
        finally:
            (output / "stage-results.json").write_text(json.dumps(results, indent=2) + "\n")

    compile_log = checked([tools["cargo"], "test", "--locked", "--manifest-path", "tauri/Cargo.toml", "--lib", "--no-run", "--message-format=json"], "rust-lib-compile")
    artifact = fresh_lib_artifact(compile_log)
    binary = Path(artifact["binary"])
    assert binary.is_relative_to(Path(env["CARGO_TARGET_DIR"])) and binary.is_file() and not binary.is_symlink()
    (output / "binary-identity.json").write_text(json.dumps(artifact, indent=2) + "\n")
    groups = [("new-case", MANIFEST["new_test"], [MANIFEST["new_test"]], ["--exact"])]
    groups += [(key, s["prefix"], s["tests"], []) for key, s in MANIFEST["focused"].items()]
    for label, selector, expected, options in groups:
        assert digest(binary.read_bytes()) == artifact["sha256"]
        checked([str(binary), selector, *options, "--list"], label + "-inventory", expected, True)
        checked([str(binary), selector, *options, "--test-threads=1"], label, expected)
    assert digest(binary.read_bytes()) == artifact["sha256"]
    verify_source(root)
    (output / "source-final.json").write_text(json.dumps({"product_sha": MANIFEST["product_sha"], "product_tree": MANIFEST["product_tree"], "tracked_files": len(MANIFEST["files"]), "status": "unchanged"}, indent=2) + "\n")
    verify_stages(results)
    assert len(command_records) == len(STAGES) and all(r["exit_code"] == 0 and not r["timed_out_or_output_limit"] and r["cleanup"] == "owned group TERM/KILL sent; leader reaped" for r in command_records)
    assert time.monotonic() < deadline - 5


def main():
    workspace = Path(os.environ["GITHUB_WORKSPACE"]).resolve()
    output = Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    with (output / "attempt.started").open("x") as marker:
        marker.write("One sealed SSH coverage attempt only\n")
    result = {"ok": False, "private_tmp_cleanup": "not_started"}
    try:
        with tempfile.TemporaryDirectory(prefix="assh-", dir=workspace) as name:
            private = Path(name)
            info = private.lstat()
            assert stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and stat.S_IMODE(info.st_mode) == 0o700
            with short_private_tmpdir() as runtime_tmp:
                run_validation(runtime_tmp, private)
        result.update(ok=True, private_tmp_cleanup="both context-owned profile and short TMP directories removed")
    except BaseException as error:
        result["error"] = type(error).__name__ + ": " + str(error)
        result["private_tmp_cleanup"] = "context cleanup attempted; overall failed"
    (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    if result["ok"]:
        (output / "PASS.txt").write_text("Sealed actual Linux Tauri lib: 13 probe + 3 gate tests passed, including the new case (also checked separately). Not a product bug fix/full Rust/docs/frontend/native/real SSH acceptance.\n")
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
