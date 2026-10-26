"""Pure contract mocks: no compiler, child process, network or resource reads."""
import ast
from contextlib import contextmanager
import importlib.util
import json
import os
import re
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("ssh_driver", HERE / "driver.py")
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


class DriverTests(unittest.TestCase):
    def test_exact_reviewed_helpers_and_command_kernel(self):
        driver.verify_reused_kernel()

    def test_changed_cleanup_output_limit_cancellation_and_failure_classification_reject(self):
        original = (HERE / "driver.py").read_text()
        for old, new in [("os.WNOHANG | os.WNOWAIT", "os.WNOHANG"),
                         ("os.killpg(child.pid, sig)", "child.kill()"),
                         ("128 * 1024**2", "256 * 1024**2"),
                         ('cancellation["spawning"] = True', 'cancellation["spawning"] = False'),
                         ('"pending": None', '"pending": 0'),
                         ('signal.signal(signal.SIGTERM, terminate)', 'signal.signal(signal.SIGTERM, signal.SIG_IGN)'),
                         ('return "test-failure"', 'return "compile-or-doc-compile-failure"')]:
            with self.subTest(old=old), self.assertRaises(AssertionError):
                driver.verify_reused_kernel(original.replace(old, new, 1))

    def test_shared_clock_accounts_for_setup_and_cannot_restart_per_stage(self):
        self.assertEqual(driver.shared_deadline(3100, 1300, 50), 1850)
        self.assertEqual(driver.shared_deadline(99999, 1300, 50), 2150)
        for epoch in [1290, 1300, 1305]:
            with self.assertRaises(AssertionError):
                driver.shared_deadline(epoch, 1300, 50)
        tree = ast.parse((HERE / "driver.py").read_text())
        run = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "run_validation")
        assignments = [n for n in ast.walk(run) if isinstance(n, ast.Assign)
                       and any(isinstance(t, ast.Name) and t.id == "deadline" for t in n.targets)]
        self.assertEqual(len(assignments), 1)
        self.assertEqual(ast.unparse(assignments[0].value), "shared_deadline(os.environ['SHARED_DEADLINE_EPOCH'], time.time(), time.monotonic())")

    def test_environment_has_one_synthetic_user_and_no_credentials(self):
        inherited = {"HOME": "/forbidden", "USERPROFILE": "/other", "GITHUB_TOKEN": "synthetic", "OPENAI_API_KEY": "synthetic", "KIMI_CODE_HOME": "/forbidden", "RUSTUP_HOME": "/forbidden"}
        tools = {name: "/synthetic/" + name for name in ["cargo", "rustc", "rustdoc"]}
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, inherited):
            root = Path(temporary)
            first = driver.isolated_env(root / "first", tools, root / "tmp")
            second = driver.isolated_env(root / "second", tools, root / "tmp")
            self.assertEqual(first["HOME"], first["USERPROFILE"])
            self.assertNotEqual(first["HOME"], second["HOME"])
            self.assertEqual(list(Path(first["HOME"]).iterdir()), [])
            self.assertEqual(os.environ["HOME"], "/forbidden")
            self.assertEqual(first["TMPDIR"], str(root / "tmp"))
            self.assertEqual(first["CARGO_BUILD_JOBS"], "2")
            self.assertEqual(first["CARGO_INCREMENTAL"], "0")
            self.assertEqual(first["RUSTC"], tools["rustc"])
            for name in ["GITHUB_TOKEN", "OPENAI_API_KEY", "KIMI_CODE_HOME", "RUSTUP_HOME"]:
                self.assertNotIn(name, first)

    def names(self):
        return driver.MANIFEST["focused"]["probe"]["tests"]

    def log(self, names=None):
        names = self.names() if names is None else names
        return "\n".join("test " + n + " ... ok" for n in names) + f"\ntest result: ok. {len(names)} passed; 0 failed; 0 ignored; 0 measured; 3114 filtered out; finished in 0.01s\n"

    def test_manifest_is_one_product_and_sixteen_distinct_tests(self):
        m = driver.MANIFEST
        self.assertEqual(m["product_sha"], "a18b38030c8f811481ed63d050d4c601fc29e044")
        self.assertEqual(m["product_tree"], "7091cf5ac5a54c697c6b9945c6b06f704160223e")
        self.assertEqual(len(m["files"]), 1845)
        self.assertEqual(len(self.names()), 13)
        gate = m["focused"]["gate"]["tests"]
        self.assertEqual(len(gate), 3)
        self.assertEqual(len(set(self.names() + gate)), 16)
        self.assertIn(m["new_test"], self.names())
        self.assertEqual(m["distinct_test_count"], 16)

    def test_inventory_rejects_zero_missing_duplicate_extra(self):
        good = "\n".join(n + ": test" for n in self.names())
        driver.verify_focused_inventory(good, self.names())
        for bad in ["", "\n".join(good.splitlines()[1:]), good + "\n" + good.splitlines()[0], good + "\nunknown: test"]:
            with self.assertRaises(AssertionError):
                driver.verify_focused_inventory(bad, self.names())
        with self.assertRaises(AssertionError):
            driver.verify_focused_inventory("", [])

    def test_focused_execution_requires_each_test_once(self):
        good = self.log()
        self.assertEqual(driver.classify_focused(0, good, self.names()), "passed")
        row = good.splitlines()[0] + "\n"
        for bad in ["", good.replace(row, ""), good + row, good.replace("... ok", "... ignored", 1), good.replace("13 passed", "0 passed"), good.replace("0 failed", "1 failed"), good + "error: synthetic\n"]:
            with self.assertRaises(AssertionError):
                driver.classify_focused(0, bad, self.names())

    def test_new_case_cannot_hide_in_aggregate_count(self):
        row = "test " + driver.MANIFEST["new_test"] + " ... ok\n"
        self.assertIn(row, self.log())
        for bad in [self.log().replace(row, ""), self.log().replace(row, "test wrong ... ok\n")]:
            with self.assertRaises(AssertionError):
                driver.classify_focused(0, bad, self.names())

    def test_failure_categories_do_not_promote_compile_or_env_to_product_pass(self):
        self.assertEqual(driver.classify_focused(101, "test result: FAILED.\nerror: test failed\n", self.names()), "test-failure")
        self.assertEqual(driver.classify_focused(101, "error[E0308]: mismatched types\n", self.names()), "compile-or-doc-compile-failure")
        self.assertEqual(driver.classify_focused(-15, "terminated\n", self.names()), "environment-or-unclassified-failure")
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, "", [])

    def test_binary_must_be_one_fresh_actual_library_test(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "lib-test"
            binary.write_bytes(b"synthetic binary identity")
            item = {"reason": "compiler-artifact", "profile": {"test": True}, "target": {"name": "ai_toolbox_lib"}, "executable": str(binary), "fresh": False}
            text = json.dumps(item)
            self.assertEqual(driver.fresh_lib_artifact(text)["sha256"], driver.digest(binary.read_bytes()))
            for bad in ["", text + "\n" + text, json.dumps({**item, "fresh": True}), json.dumps({**item, "target": {"name": "other"}}), json.dumps({**item, "profile": {"test": False}})]:
                with self.assertRaises(AssertionError):
                    driver.fresh_lib_artifact(bad)

    def test_wrong_product_sha_rejects_before_any_resource_access(self):
        with patch.object(driver.subprocess, "check_output", return_value="wrong\n"):
            with self.assertRaises(AssertionError):
                driver.verify_source(Path("/synthetic/not-read"))

    def test_all_seven_stages_are_required_in_order(self):
        self.assertEqual([name for name, _ in driver.STAGES], ["rust-lib-compile", "new-case-inventory", "new-case", "probe-inventory", "probe", "gate-inventory", "gate"])
        good = [{"stage": n, "passed": True, "exit_code": 0} for n, _ in driver.STAGES]
        driver.verify_stages(good)
        for bad in [[], good[:-1], good + [good[0]], list(reversed(good)), [{**good[0], "passed": False}, *good[1:]], [{**good[0], "exit_code": 1}, *good[1:]]]:
            with self.assertRaises(AssertionError):
                driver.verify_stages(bad)

    def test_context_cleanup_failure_cannot_write_pass(self):
        original = tempfile.TemporaryDirectory
        @contextmanager
        def cleanup_fails(**kwargs):
            with original(**kwargs) as name:
                yield name
            raise RuntimeError("synthetic cleanup failure")
        @contextmanager
        def fake_short():
            yield Path("/synthetic/not-used")
        with original() as temporary:
            root = Path(temporary); output = root / "evidence"
            with patch.dict(os.environ, {"GITHUB_WORKSPACE": str(root)}), patch.object(driver.sys, "argv", ["driver", "product", str(output), "a" * 40]), patch.object(driver, "run_validation"), patch.object(driver, "short_private_tmpdir", fake_short), patch.object(driver.tempfile, "TemporaryDirectory", cleanup_fails):
                self.assertEqual(driver.main(), 1)
            self.assertFalse((output / "PASS.txt").exists())
            self.assertFalse(json.loads((output / "result.json").read_text())["ok"])
            self.assertEqual(list(root.glob("assh-*")), [])

    def test_workflow_is_manual_exact_ref_source_and_readonly(self):
        workflow = (HERE.parents[1] / ".github/workflows/maintenance-validation.yml").read_text()
        self.assertIn("on:\n  workflow_dispatch:\n", workflow)
        self.assertNotRegex(workflow, r"(?m)^  (?:push|pull_request|schedule|release):")
        self.assertIn("permissions:\n  contents: read\n", workflow)
        self.assertIn("github.event_name == 'workflow_dispatch'", workflow)
        self.assertIn("github.ref == '" + driver.REF + "'", workflow)
        self.assertIn("github.sha == inputs.expected_sha", workflow)
        self.assertEqual(workflow.count("persist-credentials: false"), 2)
        self.assertIn("ref: " + driver.MANIFEST["product_sha"], workflow)
        self.assertIn("+ 2100", workflow)
        self.assertIn("toolchain: 1.99.1", workflow)
        self.assertNotIn("pnpm", workflow)
        self.assertNotIn("npm", workflow)
        sealed = re.findall(r"echo '([0-9a-f]{64})  (validation/ssh-drained-close/[^']+)' \| sha256sum -c -", workflow)
        self.assertEqual(len(sealed), 6)
        self.assertEqual(len(set(name for _, name in sealed)), 6)
        for expected, name in sealed:
            self.assertEqual(driver.digest((HERE.parents[1] / name).read_bytes()), expected)

    def test_only_one_actual_lib_compile_and_three_sealed_execution_groups(self):
        source = (HERE / "driver.py").read_text()
        self.assertIn('[tools["cargo"], "test", "--locked", "--manifest-path", "tauri/Cargo.toml", "--lib", "--no-run", "--message-format=json"]', source)
        self.assertEqual(source.count('"--no-run"'), 1)
        self.assertIn('"--test-threads=1"', source)
        self.assertIn('binary.is_relative_to(Path(env["CARGO_TARGET_DIR"]))', source)
        self.assertIn('not binary.is_symlink()', source)
        self.assertEqual(source.count('guards.admit('), 2)
        self.assertEqual(source.count('digest(binary.read_bytes()) == artifact["sha256"]'), 2)

    def test_mock_success_writes_pass_only_after_both_contexts_close(self):
        closed = []
        @contextmanager
        def fake_short():
            yield Path("/synthetic/not-used")
            closed.append("short closed")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); output = root / "evidence"
            def fake_run(runtime, private):
                self.assertFalse((output / "PASS.txt").exists())
                self.assertEqual(closed, [])
            with patch.dict(os.environ, {"GITHUB_WORKSPACE": str(root)}), patch.object(driver.sys, "argv", ["driver", "product", str(output), "a" * 40]), patch.object(driver, "run_validation", fake_run), patch.object(driver, "short_private_tmpdir", fake_short):
                self.assertEqual(driver.main(), 0)
                with self.assertRaises(FileExistsError):
                    driver.main()
            self.assertEqual(closed, ["short closed"])
            self.assertTrue((output / "PASS.txt").exists())
            self.assertEqual(list(root.glob("assh-*")), [])


if __name__ == "__main__":
    unittest.main()
