"""Static/synthetic tests only: no process launch, proc/cgroup, install or build."""
import ast
from contextlib import contextmanager
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("pricing_driver", HERE / "driver.py")
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


class DriverTests(unittest.TestCase):
    def test_reviewed_kernel_and_admission_are_exact(self):
        driver.verify_reused_kernel()

    def test_changed_wait_cleanup_output_limit_and_cancellation_are_rejected(self):
        original = (HERE / "driver.py").read_text()
        for old, new in [("os.WNOHANG | os.WNOWAIT", "os.WNOHANG"),
                         ("os.killpg(child.pid, sig)", "child.kill()"),
                         ("128 * 1024**2", "256 * 1024**2"),
                         ('cancellation["spawning"] = True', 'cancellation["spawning"] = False'),
                         ('"pending": None', '"pending": 0'),
                         ('signal.signal(signal.SIGTERM, terminate)', 'signal.signal(signal.SIGTERM, signal.SIG_IGN)')]:
            self.assertIn(old, original)
            with self.subTest(old=old), self.assertRaises(AssertionError):
                driver.verify_reused_kernel(original.replace(old, new, 1))

    def test_shared_deadline_accounts_for_setup_time(self):
        self.assertEqual(driver.shared_deadline(2380, 1300, 50), 1130)
        self.assertEqual(driver.shared_deadline(99999, 1300, 50), 1430)
        for epoch in [1290, 1300, 1305]:
            with self.assertRaises(AssertionError):
                driver.shared_deadline(epoch, 1300, 50)

    def test_runner_binds_one_shared_deadline_and_fixed_four_stages(self):
        tree = ast.parse((HERE / "driver.py").read_text())
        parent = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "run_validation")
        assignments = [n for n in parent.body if isinstance(n, ast.Assign)
                       and any(isinstance(t, ast.Name) and t.id == "deadline" for t in n.targets)]
        self.assertEqual(len(assignments), 1)
        self.assertEqual(ast.unparse(assignments[0].value), "shared_deadline(os.environ['SHARED_DEADLINE_EPOCH'], time.time(), time.monotonic())")
        self.assertEqual(driver.STAGES, [("frontend-install", 600), ("frontend-tsc", 300), ("frontend-test-web", 600), ("frontend-build", 600)])

    def test_synthetic_home_alias_and_credentials_boundary(self):
        inherited = {"HOME": "/forbidden", "USERPROFILE": "/other-forbidden", "GITHUB_TOKEN": "synthetic",
                     "OPENAI_API_KEY": "synthetic", "NODE_OPTIONS": "--require=/forbidden",
                     "NODE_PATH": "/forbidden", "KIMI_CODE_HOME": "/forbidden", "NPM_TOKEN": "synthetic"}
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, inherited):
            root = Path(temporary)
            first = driver.isolated_env(root / "first", root / "tmp")
            second = driver.isolated_env(root / "second", root / "tmp")
            self.assertEqual(first["HOME"], first["USERPROFILE"])
            self.assertNotEqual(first["HOME"], second["HOME"])
            self.assertEqual(list(Path(first["HOME"]).iterdir()), [])
            self.assertEqual(os.environ["HOME"], inherited["HOME"])
            self.assertEqual(first["TMPDIR"], str(root / "tmp"))
            self.assertEqual(first["npm_config_userconfig"], "/dev/null")
            for key in ["GITHUB_TOKEN", "OPENAI_API_KEY", "NODE_OPTIONS", "NODE_PATH", "KIMI_CODE_HOME", "NPM_TOKEN", "RUSTC", "RUSTDOC"]:
                self.assertNotIn(key, first)

    def web_log(self):
        rows = ["✔ " + name + " (1.25ms)" for name in driver.MANIFEST["regression_names"]]
        rows += ["ℹ tests 1152", "ℹ pass 1152", "ℹ fail 0", "ℹ cancelled 0", "ℹ skipped 0", "ℹ todo 0"]
        return "\n".join(rows) + "\n"

    def test_exact_web_inventory_accepts(self):
        driver.verify_web_log(self.web_log())
        self.assertEqual(len(driver.MANIFEST["web_test_files"]), 151)
        self.assertEqual(len(set(driver.MANIFEST["web_test_files"])), 151)
        self.assertEqual(len(driver.MANIFEST["files"]), 1845)
        self.assertEqual(driver.MANIFEST["product_sha"], "8269bc61aa3f5932504613e172ed57382e27db02")
        self.assertEqual(driver.MANIFEST["product_tree"], "6b03cf2941066b1a1d1b02ad8327721758038981")

    def test_zero_missing_duplicate_failed_and_skipped_web_summaries_reject(self):
        original = self.web_log()
        for changed in [original.replace("1152", "0"), original.replace("ℹ tests 1152\n", ""),
                        original + "ℹ pass 1152\n", original.replace("ℹ fail 0", "ℹ fail 1"),
                        original.replace("ℹ skipped 0", "ℹ skipped 1"), original.replace("ℹ cancelled 0", "ℹ cancelled 1"),
                        original.replace("ℹ todo 0", "ℹ todo 1"), original + "Web test runner finished without running any tests\n"]:
            with self.subTest(changed=changed[-90:]), self.assertRaises(AssertionError):
                driver.verify_web_log(changed)

    def test_new_regressions_cannot_be_missing_or_duplicated(self):
        original = self.web_log()
        row = original.splitlines()[0] + "\n"
        for changed in [original.replace(row, ""), original + row, original.replace("✔", "✖", 1)]:
            with self.assertRaises(AssertionError):
                driver.verify_web_log(changed)

    def test_exact_four_successful_stages_required(self):
        good = [{"stage": name, "passed": True, "exit_code": 0} for name, _ in driver.STAGES]
        driver.verify_stages(good)
        for changed in [[], good[:-1], good + [good[0]], list(reversed(good)),
                        [{**good[0], "passed": False}, *good[1:]], [{**good[0], "exit_code": 1}, *good[1:]]]:
            with self.assertRaises(AssertionError):
                driver.verify_stages(changed)

    def test_dist_requires_real_index_and_javascript(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(AssertionError):
                driver.dist_inventory(root, float("inf"))
            (root / "dist/assets").mkdir(parents=True)
            (root / "dist/index.html").write_text("synthetic index")
            with self.assertRaises(AssertionError):
                driver.dist_inventory(root, float("inf"))
            (root / "dist/assets/app.js").write_text("synthetic build")
            records = driver.dist_inventory(root, float("inf"))
            self.assertEqual({r["path"] for r in records}, {"index.html", "assets/app.js"})
            self.assertTrue(all(len(r["sha256"]) == 64 for r in records))
            with self.assertRaises(AssertionError):
                driver.dist_inventory(root, 0)

    def test_dist_symlinks_are_not_hashed_as_build_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "dist").mkdir()
            (root / "dist/index.html").symlink_to(root / "other")
            with self.assertRaises(AssertionError):
                driver.dist_inventory(root, float("inf"))

    def test_context_cleanup_failure_cannot_leave_a_pass_marker(self):
        original_temporary_directory = tempfile.TemporaryDirectory

        @contextmanager
        def fails_after_cleanup(**kwargs):
            with original_temporary_directory(**kwargs) as name:
                yield name
            raise RuntimeError("synthetic cleanup failure")

        with original_temporary_directory() as temporary:
            root = Path(temporary)
            output = root / "evidence"
            with patch.dict(os.environ, {"GITHUB_WORKSPACE": str(root)}), \
                 patch.object(driver.sys, "argv", ["driver", str(root / "product"), str(output), "a" * 40]), \
                 patch.object(driver, "run_validation"), \
                 patch.object(driver.tempfile, "TemporaryDirectory", fails_after_cleanup):
                self.assertEqual(driver.main(), 1)
            self.assertFalse((output / "PASS.txt").exists())
            self.assertFalse(json.loads((output / "result.json").read_text())["ok"])
            self.assertEqual(list(root.glob("a441-*")), [])


if __name__ == "__main__":
    unittest.main()
