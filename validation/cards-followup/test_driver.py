"""Static/synthetic tests only: no process launch, proc/cgroup, install or build."""
import ast
import copy
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
                         ("path.stat().st_size > 128 * 1024**2", "path.stat().st_size > 256 * 1024**2"),
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

    def test_runner_binds_one_shared_deadline_and_fixed_three_stages(self):
        tree = ast.parse((HERE / "driver.py").read_text())
        parent = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "run_validation")
        assignments = [n for n in parent.body if isinstance(n, ast.Assign)
                       and any(isinstance(t, ast.Name) and t.id == "deadline" for t in n.targets)]
        self.assertEqual(len(assignments), 1)
        self.assertEqual(ast.unparse(assignments[0].value), "shared_deadline(os.environ['SHARED_DEADLINE_EPOCH'], time.time(), time.monotonic())")
        self.assertEqual(driver.STAGES, [("frontend-install", 600), ("frontend-test-web", 600), ("frontend-migrated-cards", 600)])

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
        rows += ["ℹ tests 1202", "ℹ pass 1202", "ℹ fail 0", "ℹ cancelled 0", "ℹ skipped 0", "ℹ todo 0"]
        return "\n".join(rows) + "\n"

    def test_exact_web_inventory_accepts(self):
        driver.verify_web_log(self.web_log())
        self.assertEqual(len(driver.MANIFEST["web_test_files"]), 159)
        self.assertEqual(len(set(driver.MANIFEST["web_test_files"])), 159)
        self.assertEqual(len(driver.MANIFEST["files"]), 1879)
        self.assertEqual(driver.MANIFEST["product_sha"], "7acb2ac95a765fd9966213c82daa2c12048a0a48")
        self.assertEqual(driver.MANIFEST["product_tree"], "f2d53ab84ee7c20f4b516cd6a918290409928bec")

    def test_zero_missing_duplicate_failed_and_skipped_web_summaries_reject(self):
        original = self.web_log()
        for changed in [original.replace("1202", "0"), original.replace("ℹ tests 1202\n", ""),
                        original + "ℹ pass 1202\n", original.replace("ℹ fail 0", "ℹ fail 1"),
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

    def test_exact_three_successful_stages_required(self):
        good = [{"stage": name, "passed": True, "exit_code": 0} for name, _ in driver.STAGES]
        driver.verify_stages(good)
        for changed in [[], good[:-1], good + [good[0]], list(reversed(good)),
                        [{**good[0], "passed": False}, *good[1:]], [{**good[0], "exit_code": 1}, *good[1:]]]:
            with self.assertRaises(AssertionError):
                driver.verify_stages(changed)

    def prior_records(self):
        prior = driver.MANIFEST["prior"]
        plan = {"driver_sha": prior["driver_sha"], "manifest": {k: prior[k] for k in ["product_sha", "product_tree"]}}
        stages = [{"stage": name, "passed": name != prior["failed_stage"], "exit_code": int(name == prior["failed_stage"])} for name in prior["stage_names"]]
        stages[-1]["classification"] = "passed"
        commands = [{"label": item["stage"], "timed_out_or_output_limit": False, "cleanup": "owned group TERM/KILL sent; leader reaped", "exit_code": item["exit_code"]} for item in stages]
        return plan, stages, commands

    def test_prior_requires_exact_43_passing_and_one_original_failure(self):
        self.assertEqual(len(driver.verify_prior_records(*self.prior_records())), 43)
        for mutation in [lambda p, s, c: s.pop(), lambda p, s, c: s.append(s[0]),
                         lambda p, s, c: s[0].update(passed=False), lambda p, s, c: s[17].update(passed=True),
                         lambda p, s, c: p.update(driver_sha="x"), lambda p, s, c: p["manifest"].update(product_tree="x"),
                         lambda p, s, c: c[0].update(timed_out_or_output_limit=True), lambda p, s, c: c[0].update(cleanup="pending"),
                         lambda p, s, c: s[-1].update(classification="unclassified")]:
            records = self.prior_records()
            mutation(*records)
            with self.assertRaises(AssertionError): driver.verify_prior_records(*records)

    def write_reports(self, root):
        migration = {"completed": list(driver.MANIFEST["browser_scenarios"]), "assertionCounts": driver.MANIFEST["browser_scenarios"], "checks": [f"migration-{i}" for i in range(134)], "browserExceptions": []}
        routing = {"assertionCounts": driver.MANIFEST["routing_scenarios"], "passed": list(driver.MANIFEST["routing_scenarios"]), "failed": [], "fixtureErrors": [],
                   "summary": {"attempted": 8, "passed": 8, "failed": 0, "passedControls": 6, "failedControls": 0, "expectedDefectFailures": 0, "unexpectedFailures": 0},
                   "cases": [{"name": name, "status": "passed", "phase": "complete", "passedAssertions": count} for name, count in driver.MANIFEST["routing_scenarios"].items()],
                   "checks": [f"routing-{i}" for i in range(28)], "browserExceptions": []}
        results = {"checks": migration["checks"] + routing["checks"], "browserExceptions": []}
        for name, value in [("scenarios.json", migration), ("desktop-routing-cases.json", routing), ("results.json", results)]:
            (root / name).write_text(json.dumps(value))
        for name in ["migrated-provider-cards-light.png", "migrated-provider-cards-dark.png"]:
            (root / name).write_bytes(b"\x89PNG\r\n\x1a\nsynthetic")
        return "Passed 11 migrated-provider-card scenarios (134 assertions).\nPassed 8 Desktop routing scenarios (28 assertions).\nPassed 162 browser checks.\n"

    def test_browser_reports_require_162_exact_checks_and_all_19_scenarios(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = self.write_reports(root)
            driver.verify_browser_reports(root, log)
            for filename, mutate in [
                ("scenarios.json", lambda d: d["completed"].pop()),
                ("scenarios.json", lambda d: d["checks"].pop()),
                ("scenarios.json", lambda d: d["checks"].append(d["checks"][0])),
                ("desktop-routing-cases.json", lambda d: d["cases"][0].update(status="failed")),
                ("desktop-routing-cases.json", lambda d: d["summary"].update(unexpectedFailures=1)),
                ("desktop-routing-cases.json", lambda d: d["checks"].pop()),
                ("results.json", lambda d: d["checks"].reverse()),
                ("results.json", lambda d: d["browserExceptions"].append("error")),
            ]:
                self.write_reports(root)
                record = json.loads((root / filename).read_text()); mutate(record); (root / filename).write_text(json.dumps(record))
                with self.assertRaises(AssertionError): driver.verify_browser_reports(root, log)
            self.write_reports(root)
            for changed in ["", log + log, log.replace("162", "114")]:
                with self.assertRaises(AssertionError): driver.verify_browser_reports(root, changed)
            (root / "migrated-provider-cards-dark.png").write_bytes(b"not a screenshot")
            with self.assertRaises(AssertionError): driver.verify_browser_reports(root, log)

    def test_browser_copy_is_bounded_owned_and_excludes_build_and_profile_data(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); output = root / "output"; output.mkdir()
            self.assertIsNone(driver.preserve_browser_evidence(root, output, float("inf")))
            fixture = root / "migrated-provider-cards-synthetic"; fixture.mkdir()
            (fixture / "scenarios.json").write_text("{}"); (fixture / "profile").mkdir(); (fixture / "dist").mkdir()
            copied = driver.preserve_browser_evidence(root, output, float("inf"))
            self.assertEqual([p.name for p in copied.iterdir()], ["scenarios.json"])
            (root / "migrated-provider-cards-second").mkdir()
            with self.assertRaises(AssertionError): driver.preserve_browser_evidence(root, output, float("inf"))

    def test_browser_copy_rejects_links_and_exhausted_deadline(self):
        for expired in [False, True]:
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary); output = root / "output"; output.mkdir()
                fixture = root / "migrated-provider-cards-synthetic"; fixture.mkdir()
                if expired: (fixture / "failure.json").write_text("{}")
                else: (fixture / "failure.json").symlink_to(root / "other")
                with self.assertRaises(AssertionError): driver.preserve_browser_evidence(root, output, 0 if expired else float("inf"))

    def test_context_cleanup_failure_cannot_leave_a_pass_marker(self):
        original_temporary_directory = tempfile.TemporaryDirectory

        @contextmanager
        def fails_after_cleanup(**kwargs):
            kwargs["dir"] = os.environ.get("TMPDIR")
            with original_temporary_directory(**kwargs) as name:
                yield name
            raise RuntimeError("synthetic cleanup failure")

        with original_temporary_directory() as temporary:
            root = Path(temporary)
            output = root / "evidence"
            with patch.dict(os.environ, {"GITHUB_WORKSPACE": str(root)}), \
                 patch.object(driver.sys, "argv", ["driver", str(root / "product"), str(output), "a" * 40]), \
                 patch.object(driver, "run_validation") as run_mock, \
                 patch.object(driver.os, "fsencode", return_value=b"/tmp/a440-test"), \
                 patch.object(driver.tempfile, "TemporaryDirectory", fails_after_cleanup):
                self.assertEqual(driver.main(), 1)
                run_mock.assert_called_once()
            self.assertFalse((output / "PASS.txt").exists())
            self.assertFalse(json.loads((output / "result.json").read_text())["ok"])
            self.assertEqual(list(root.glob("a440-*")), [])


if __name__ == "__main__":
    unittest.main()
