"""Manual sealed-product Linux validation, not Windows or live OAuth acceptance."""
from collections import Counter
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time

if not __debug__:
    raise RuntimeError("Optimized Python is unsupported: validation assertions must remain enabled")

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("reviewed_guards", HERE / "verify-codex-usage-440.py")
guards = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guards)
MANIFEST = json.loads((HERE / "codex-usage-440-full-manifest.json").read_text())
REF = "refs/heads/validation/codex-440-recovery-full-20261025"


def verify_source(root):
    for expression, expected in [("HEAD", MANIFEST["product_sha"]), ("HEAD^{tree}", MANIFEST["product_tree"])]:
        assert subprocess.check_output(["git", "rev-parse", expression], cwd=root, text=True).strip() == expected
    assert not subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=root, text=True).strip()
    assert json.loads((root / "package.json").read_text())["scripts"] == MANIFEST["scripts"]
    verify_focused_source((root / MANIFEST["regressions"]["path"]).read_text())
    observed = []
    for path in sorted((root / "tauri").rglob("*.rs")):
        for line in path.read_text().splitlines():
            if "#[ignore" in line:
                observed.append((str(path.relative_to(root)), line.strip()))
    assert observed == [(case["path"], case["attribute"]) for case in MANIFEST["ignored"]], "Ignored source inventory changed"
    for case in MANIFEST["ignored"]:
        assert case["context"] in (root / case["path"]).read_text(), "Ignored name/reason/platform context changed"


def verify_ignored(listing):
    names = sorted(line.removesuffix(": test") for line in listing.splitlines() if line.endswith(": test"))
    assert names == sorted(case["name"] for case in MANIFEST["ignored"]), "Runtime ignored identities differ"


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


def full_inventory(listing):
    names = [line.removesuffix(": test") for line in listing.splitlines() if line.endswith(": test")]
    assert names, "Default suite inventory is empty"
    assert set(case["name"] for case in MANIFEST["ignored"]) < set(names), "No runnable tests in default inventory"
    return names


def classify_rust(code, content, inventory):
    if code == 0:
        assert inventory, "No default test inventory"
        assert "Doc-tests" in content, "Missing default suite/doc-test execution"
        assert not re.search(r"test result: FAILED|^error(?:\[|:)", content, re.MULTILINE)
        observed = re.findall(r"^test (.+?) \.{3} (ok|ignored(?:,.*)?|FAILED)$", content, re.MULTILINE)
        assert Counter(name for name, status in observed) == Counter(inventory), "Executed test identities do not match complete default inventory"
        ignored = sorted(name for name, status in observed if status.startswith("ignored"))
        assert ignored == sorted(case["name"] for case in MANIFEST["ignored"]), "Execution ignore inventory changed"
        summaries = re.findall(r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", content, re.MULTILINE)
        assert summaries, "No numeric test summary"
        totals = [sum(int(summary[i]) for summary in summaries) for i in range(5)]
        assert totals == [len(inventory) - len(ignored), 0, len(ignored), 0, 0] and totals[0] > 0, "Incomplete/zero/filtered test execution"
        return "passed"
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


def verify_focused_source(source):
    # This sealed file has exactly these two test modules. Runtime --list still
    # independently verifies every selected compiled identity before execution.
    marker = "mod usage_credential_tests {"
    assert source.count(marker) == 1 and source.count("mod tests {") == 1
    before, credentials = source.split(marker, 1)
    neighboring = before.split("mod tests {", 1)[1]
    for key, section in [("usage-credentials", credentials), ("neighboring-accounts", neighboring)]:
        selection = MANIFEST["focused"][key]
        names = re.findall(r"#\[(?:tokio::)?test\]\s+(?:async\s+)?fn\s+(\w+)", section)
        assert Counter(selection["prefix"] + name for name in names) == Counter(selection["tests"]), "Focused source inventory changed"
    regressions = MANIFEST["regressions"]
    selected = [name for name in MANIFEST["focused"]["usage-credentials"]["tests"] if name.startswith(regressions["prefix"])]
    assert Counter(selected) == Counter(regressions["tests"]), "Permanent regression selection changed"
    assert set(regressions["function_sha256"]) == set(regressions["tests"])
    for name, expected in regressions["function_sha256"].items():
        # Hash the original complete test function, including every assertion.
        # A source-layout change must be reviewed/resealed, never guessed here.
        short = name.rsplit("::", 1)[1]
        start = source.index("    #[tokio::test]\n    async fn " + short + "(")
        end = source.index("\n    }", start) + 6
        assert hashlib.sha256(source[start:end].encode()).hexdigest() == expected, "Permanent regression assertions changed"


def required_stage_names():
    names = ["frontend-install", "frontend-tsc"]
    names += [f"frontend-{index:02d}-{name.replace(':', '-')}" for index, name in enumerate(MANIFEST["test_chain"])]
    names += ["frontend-build", "rust-default-compile"]
    for label in MANIFEST["focused"]:
        names += ["rust-product-" + label + "-inventory", "rust-product-" + label]
    names += ["rust-product-regressions-inventory", "rust-product-regressions",
              "rust-full-inventory", "rust-ignored-inventory", "rust-default-tests-and-docs"]
    assert len(names) == len(set(names))
    return names


def verify_required_stages(results):
    assert Counter(result["stage"] for result in results) == Counter(required_stage_names()), "Required stage missing or duplicated"
    assert all(result["passed"] for result in results), "One or more required stages failed; see stage-results.json"


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


def run_validation(runtime_tmp):
    root, output, expected_sha = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve(), sys.argv[3]
    assert re.fullmatch(r"[0-9a-f]{40}", expected_sha)
    driver_root = HERE.parents[1]
    assert os.environ.get("GITHUB_REPOSITORY") == "xue-ding-e/AI-Toolbox-Next"
    assert os.environ.get("GITHUB_REF") == REF and os.environ.get("GITHUB_SHA") == expected_sha
    assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=driver_root, text=True).strip() == expected_sha
    assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=driver_root, text=True).strip()
    verify_source(root)
    output.mkdir(parents=True, exist_ok=True)
    deadline = time.monotonic() + 100 * 60
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

    results = []
    def checked(args, label, budget, env, category, cwd=None):
        code, content = run(args, root if cwd is None else cwd, label, timeout=budget, env=env)
        result = {"stage": label, "category": category, "exit_code": code, "passed": code == 0,
                  "earlier_failed_stages": [item["stage"] for item in results if not item["passed"]],
                  "failure_attribution": None if code == 0 else "Unattributed stage failure; inspect setup and earlier failures before claiming a product bug"}
        results.append(result)
        (output / "stage-results.json").write_text(json.dumps(results, indent=2) + "\n")
        return code, content

    def focused(binary, selection, label, cwd):
        code, listing = checked([binary, selection["prefix"], "--list"], label + "-inventory", 120, env, "inventory-not-execution", cwd)
        assert code == 0
        verify_focused_inventory(listing, selection["tests"])
        code, content = checked([binary, selection["prefix"], "--test-threads=1"], label, 180, env, "focused-rust-not-full-suite", cwd)
        results[-1]["classification"] = classify_focused(code, content, selection["tests"])
        results[-1]["expected_identities"] = selection["tests"]
        results[-1]["summaries"] = re.findall(r"^test result: .*", content, re.MULTILINE)
        (output / "stage-results.json").write_text(json.dumps(results, indent=2) + "\n")

    guards.admit(root)
    guards.admit(runtime_tmp)
    tools = {name: subprocess.check_output(["rustup", "which", name], text=True).strip() for name in ["cargo", "rustc", "rustdoc"]}
    assert all(Path(path).is_file() for path in tools.values())
    env = isolated_env(output.parent / "private-full-profile", tools, runtime_tmp)
    (output / "source-and-plan.json").write_text(json.dumps({"driver_sha": expected_sha, "manifest": MANIFEST, "tools": tools,
        "runtime_tmpdir": str(runtime_tmp), "runtime_tmpdir_scope": "Owned random 0700 directory, removed after owned child cleanup; not a real user profile",
        "budgets_seconds": {"total": 6000, "install": 600, "tsc": 300, "each_frontend_script": 600, "official_edit_script": 900, "build": 600, "compile": 1800, "focused_inventory": 120, "focused_execution": 180, "rust_default_tests_and_docs": 1200},
        "scope": "Sealed product: Linux default cargo targets/docs and complete pnpm test chain; focused 30+31 and the three permanent baseline-RED regressions are additional evidence, not full-suite substitutes. The same sealed product binary runs all selections; no test-only overlay. No live OAuth or Windows acceptance."}, indent=2) + "\n")
    code, _ = checked(["pnpm", "install", "--frozen-lockfile"], "frontend-install", 600, env, "dependency-environment")
    if code == 0:
        checked(["pnpm", "exec", "tsc", "--noEmit"], "frontend-tsc", 300, env, "typecheck")
        # Expand the exact && chain, retaining conjunction semantics for PASS,
        # but collect every script's outcome even if an earlier script fails.
        for index, name in enumerate(MANIFEST["test_chain"]):
            checked(["pnpm", "run", name], f"frontend-{index:02d}-{name.replace(':', '-')}", 900 if name == "test:claude-desktop-official-edit" else 600, env, "frontend-test")
        checked(["pnpm", "build"], "frontend-build", 600, env, "build")
    guards.admit(root)
    cargo = [tools["cargo"], "test", "--locked", "--manifest-path", "tauri/Cargo.toml"]
    code, content = checked(cargo + ["--no-run", "--message-format=json"], "rust-default-compile", 1800, env, "compile")
    if code == 0:
        artifacts = []
        for line in content.splitlines():
            try:
                item = json.loads(line)
            except ValueError:
                continue
            if item.get("reason") == "compiler-artifact" and item.get("profile", {}).get("test") and item.get("executable"):
                assert not item.get("fresh"), "Own test binary unexpectedly reused"
                path = Path(item["executable"])
                artifacts.append({"target": item["target"], "binary": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
        assert artifacts and any(a["target"]["name"] == "ai_toolbox_lib" for a in artifacts)
        (output / "binary-identities.json").write_text(json.dumps(artifacts, indent=2) + "\n")
        product_lib = fresh_lib_artifact(content)
        for label, selection in MANIFEST["focused"].items():
            focused(product_lib["binary"], selection, "rust-product-" + label, root)
        focused(product_lib["binary"], MANIFEST["regressions"], "rust-product-regressions", root)
        code, content = checked(cargo + ["--", "--list"], "rust-full-inventory", 180, env, "inventory-not-execution")
        assert code == 0
        inventory = full_inventory(content)
        (output / "full-test-inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
        code, content = checked(cargo + ["--", "--ignored", "--list"], "rust-ignored-inventory", 180, env, "inventory-not-execution")
        assert code == 0
        verify_ignored(content)
        code, content = checked(cargo + ["--", "--test-threads=1"], "rust-default-tests-and-docs", 1200, env, "rust-test")
        results[-1]["classification"] = classify_rust(code, content, inventory)
        results[-1]["summaries"] = re.findall(r"^test result: .*", content, re.MULTILINE)
        if code == 0:
            for artifact in artifacts:
                assert Path(artifact["binary"]).name in content, "Compiled default target absent from execution log"
        (output / "stage-results.json").write_text(json.dumps(results, indent=2) + "\n")
        for artifact in artifacts:
            assert hashlib.sha256(Path(artifact["binary"]).read_bytes()).hexdigest() == artifact["sha256"]
    verify_source(root)
    verify_required_stages(results)
    message = "Sealed Linux product/default cargo/docs, complete pnpm chain, focused 30+31 and permanent baseline-RED regressions passed on the same product binary. Not Windows/live OAuth acceptance.\n"
    (output / "PRODUCT-PASS.txt").write_text(message)
    (output / "PASS.txt").write_text(message)


def main():
    # run_validation cleans each owned process group even on exceptions/signals
    # before this scope removes its own temporary files.
    with short_private_tmpdir() as runtime_tmp:
        run_validation(runtime_tmp)


if __name__ == "__main__":
    main()
