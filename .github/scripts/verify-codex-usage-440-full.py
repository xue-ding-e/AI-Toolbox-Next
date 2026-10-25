"""Manual sealed-product Linux validation, not Windows or live OAuth acceptance."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("reviewed_guards", HERE / "verify-codex-usage-440.py")
guards = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guards)
MANIFEST = json.loads((HERE / "codex-usage-440-full-manifest.json").read_text())
REF = "refs/heads/validation/codex-usage-440-full-20261025"


def verify_source(root):
    for expression, expected in [("HEAD", MANIFEST["product_sha"]), ("HEAD^{tree}", MANIFEST["product_tree"])]:
        assert subprocess.check_output(["git", "rev-parse", expression], cwd=root, text=True).strip() == expected
    assert not subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=root, text=True).strip()
    assert json.loads((root / "package.json").read_text())["scripts"] == MANIFEST["scripts"]
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


def isolated_env(profile, tool_paths):
    # Allowlist rather than deleting a few known credential variable names.
    env = {key: os.environ[key] for key in ["PATH", "LANG", "LC_ALL", "TZ", "LD_LIBRARY_PATH"] if key in os.environ}
    profile.mkdir(parents=True, exist_ok=False)
    for key in ["HOME", "USERPROFILE", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "CODEX_HOME", "CARGO_HOME", "TMPDIR"]:
        directory = profile / key.lower()
        directory.mkdir()
        env[key] = str(directory)
    env.update(RUSTC=tool_paths["rustc"], RUSTDOC=tool_paths["rustdoc"], CARGO_TARGET_DIR=str(profile / "target"),
               CARGO_BUILD_JOBS="2", CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_TEST_DEBUG="0", CARGO_INCREMENTAL="0",
               RUST_BACKTRACE="0", CI="true", HUSKY="0")
    return env


def classify_rust(code, content):
    if code == 0:
        assert "test result: ok." in content and "Doc-tests" in content, "Missing default suite/doc-test execution"
        assert not re.search(r"test result: FAILED|^error(?:\[|:)", content, re.MULTILINE)
        observed = sorted(re.findall(r"^test (\S+) \.{3} ignored(?:,.*)?$", content, re.MULTILINE))
        assert observed == sorted(case["name"] for case in MANIFEST["ignored"]), "Execution ignore inventory changed"
        return "passed"
    if re.search(r"^error(?:\[|:)|could not compile", content, re.MULTILINE):
        return "compile-or-doc-compile-failure"
    if "test result: FAILED" in content:
        return "test-failure"
    return "environment-or-unclassified-failure"


def main():
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
    def checked(args, label, budget, env, category):
        code, content = run(args, root, label, timeout=budget, env=env)
        result = {"stage": label, "category": category, "exit_code": code, "passed": code == 0,
                  "earlier_failed_stages": [item["stage"] for item in results if not item["passed"]],
                  "failure_attribution": None if code == 0 else "Unattributed stage failure; inspect setup and earlier failures before claiming a product bug"}
        results.append(result)
        (output / "stage-results.json").write_text(json.dumps(results, indent=2) + "\n")
        return code, content

    guards.admit(root)
    tools = {name: subprocess.check_output(["rustup", "which", name], text=True).strip() for name in ["cargo", "rustc", "rustdoc"]}
    assert all(Path(path).is_file() for path in tools.values())
    env = isolated_env(output.parent / "private-full-profile", tools)
    (output / "source-and-plan.json").write_text(json.dumps({"driver_sha": expected_sha, "manifest": MANIFEST, "tools": tools,
        "budgets_seconds": {"total": 6000, "install": 600, "tsc": 300, "each_frontend_script": 600, "official_edit_script": 900, "build": 600, "compile": 1800, "rust_default_tests_and_docs": 1200},
        "scope": "Linux default cargo targets and docs; exact complete pnpm test chain, no added skip; not Windows/live OAuth"}, indent=2) + "\n")
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
        code, content = checked(cargo + ["--", "--ignored", "--list"], "rust-ignored-inventory", 180, env, "inventory-not-execution")
        assert code == 0
        verify_ignored(content)
        code, content = checked(cargo + ["--", "--test-threads=1"], "rust-default-tests-and-docs", 1200, env, "rust-test")
        results[-1]["classification"] = classify_rust(code, content)
        results[-1]["summaries"] = re.findall(r"^test result: .*", content, re.MULTILINE)
        if code == 0:
            for artifact in artifacts:
                assert Path(artifact["binary"]).name in content, "Compiled default target absent from execution log"
        (output / "stage-results.json").write_text(json.dumps(results, indent=2) + "\n")
        for artifact in artifacts:
            assert hashlib.sha256(Path(artifact["binary"]).read_bytes()).hexdigest() == artifact["sha256"]
    verify_source(root)
    assert all(result["passed"] for result in results), "One or more required stages failed; see stage-results.json"
    assert len(results) == len(MANIFEST["test_chain"]) + 6, "Required stage missing"
    (output / "PASS.txt").write_text("Sealed Linux default cargo suite/docs and complete pnpm test chain passed. Not Windows/live OAuth acceptance.\n")


if __name__ == "__main__":
    main()
