"""Pure synthetic admission/classifier tests: every proc/cgroup I/O is mocked."""
import contextlib
import importlib.util
import io
from pathlib import Path
import stat
import types
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("codex440driver", Path(__file__).with_name("verify-codex-usage-440.py"))
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)
GIB = 1024**3
ROOT = "/sys/fs/cgroup"


class SyntheticV2:
    def __init__(self):
        self.text = {
            "/proc/meminfo": f"MemTotal: {16 * GIB // 1024} kB\nMemAvailable: {10 * GIB // 1024} kB\n",
            "/proc/self/cgroup": "0::/jobs/test\n",
            "/proc/self/mountinfo": "36 25 0:32 / /sys/fs/cgroup rw - cgroup2 cgroup rw\n",
        }
        self.controls = {
            ROOT + "/cgroup.controllers": b"cpu memory pids\n",
            ROOT + "/memory.stat": b"anon 1\nfile 2\nkernel_stack 3\npagetables 4\n",
            ROOT + "/jobs/memory.max": str(8 * GIB).encode(),
            ROOT + "/jobs/memory.current": str(GIB).encode(),
            ROOT + "/jobs/test/memory.max": b"max",
            ROOT + "/jobs/test/memory.current": str(GIB).encode(),
        }
        self.directories = {ROOT, ROOT + "/jobs", ROOT + "/jobs/test"}
        self.fds, self.counts, self.opened = {}, {}, []
        self.next_fd = 100
        self.denied = set()
        self.nonregular = set()
        self.namespace = "cgroup:[42]"

    def read_text(self, path, *_args, **_kwargs):
        path = str(path)
        if path not in self.text:
            raise AssertionError("Unmocked text read: " + path)
        self.counts[path] = self.counts.get(path, 0) + 1
        value = self.text[path]
        return value[min(self.counts[path] - 1, len(value) - 1)] if isinstance(value, list) else value

    def readlink(self, path):
        assert path == "/proc/self/ns/cgroup", "Another process's namespace must not be read"
        self.counts[path] = self.counts.get(path, 0) + 1
        value = self.namespace
        return value[min(self.counts[path] - 1, len(value) - 1)] if isinstance(value, list) else value

    def open(self, path, flags, *, dir_fd=None):
        path = str(path) if dir_fd is None else self.fds[dir_fd] + "/" + str(path)
        self.opened.append(path)
        assert flags & driver.os.O_NOFOLLOW, "Control traversal must not follow symlinks"
        if path in self.denied:
            raise PermissionError(13, "synthetic denied", path)
        if path not in self.controls and path not in self.directories:
            raise FileNotFoundError(2, "synthetic absent", path)
        self.next_fd += 1
        self.fds[self.next_fd] = path
        return self.next_fd

    def fstat(self, fd):
        path = self.fds[fd]
        kind = stat.S_IFDIR if path in self.directories or path in self.nonregular else stat.S_IFREG
        return types.SimpleNamespace(st_mode=kind | 0o600, st_dev=1, st_ino=hash(path))

    def observe(self):
        with contextlib.ExitStack() as stack:
            # A plain wrapper binds the Path instance; no real /proc or cgroup
            # call is available as a fallback anywhere in this fixture.
            stack.enter_context(mock.patch.object(Path, "read_text", lambda path, *a, **kw: self.read_text(path, *a, **kw)))
            stack.enter_context(mock.patch.object(Path, "resolve", lambda path, **_kw: path))
            stack.enter_context(mock.patch.object(driver.os, "readlink", self.readlink))
            stack.enter_context(mock.patch.object(driver.os, "open", self.open))
            stack.enter_context(mock.patch.object(driver.os, "fstat", self.fstat))
            stack.enter_context(mock.patch.object(driver.os, "read", lambda fd, size: self.controls[self.fds[fd]][:size]))
            stack.enter_context(mock.patch.object(driver.os, "close", lambda fd: self.fds.pop(fd)))
            stack.enter_context(mock.patch.object(driver.os, "environ", {"GITHUB_ACTIONS": "true", "RUNNER_ENVIRONMENT": "github-hosted", "RUNNER_OS": "Linux"}))
            stack.enter_context(mock.patch.object(driver.shutil, "disk_usage", lambda _path: types.SimpleNamespace(free=32 * GIB)))
            stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
            return driver.admit("/synthetic-workspace")


class AdmissionTests(unittest.TestCase):
    def test_real_root_positive_evidence_and_exact_missing_controls(self):
        fake = SyntheticV2()
        fake.text["/proc/self/cgroup"] = "0::/\n"
        result = fake.observe()
        self.assertEqual(result["effective_available_bytes"], 10 * GIB)
        self.assertEqual(result["effective_capacity_bytes"], 16 * GIB)

    def test_ancestor_limit_beats_larger_child_limit(self):
        fake = SyntheticV2()
        fake.controls[ROOT + "/jobs/test/memory.max"] = str(12 * GIB).encode()
        result = fake.observe()
        self.assertEqual(result["effective_capacity_bytes"], 8 * GIB)
        self.assertEqual(result["effective_available_bytes"], 7 * GIB)

    def test_child_max_does_not_ignore_ancestor(self):
        self.assertEqual(SyntheticV2().observe()["effective_available_bytes"], 7 * GIB)

    def test_hidden_mount_root_is_unknown(self):
        fake = SyntheticV2()
        fake.text["/proc/self/mountinfo"] = "36 25 0:32 /hidden /sys/fs/cgroup rw - cgroup2 cgroup rw\n"
        with self.assertRaisesRegex(RuntimeError, "hidden cgroup ancestors"):
            fake.observe()

    def test_visible_root_scalar_control_is_unknown(self):
        for control in ["memory.max", "memory.current"]:
            fake = SyntheticV2()
            fake.controls[ROOT + "/" + control] = b"max" if control == "memory.max" else b"0"
            with self.assertRaisesRegex(RuntimeError, "incomplete ancestry"):
                fake.observe()

    def test_permission_error_is_not_missing_and_has_no_alternate_read(self):
        fake = SyntheticV2()
        fake.denied.add(ROOT + "/memory.max")
        with self.assertRaises(PermissionError):
            fake.observe()
        self.assertEqual(fake.opened[-1], ROOT + "/memory.max")
        self.assertEqual(fake.opened.count(ROOT + "/memory.max"), 1)

    def test_missing_child_control_is_not_unlimited(self):
        fake = SyntheticV2()
        del fake.controls[ROOT + "/jobs/memory.current"]
        with self.assertRaises(FileNotFoundError):
            fake.observe()

    def test_cgroup_migration_is_unknown(self):
        fake = SyntheticV2()
        fake.text["/proc/self/cgroup"] = ["0::/jobs/test\n", "0::/other\n"]
        with self.assertRaisesRegex(RuntimeError, "process migrated"):
            fake.observe()

    def test_namespace_migration_is_unknown(self):
        fake = SyntheticV2()
        fake.namespace = ["cgroup:[42]", "cgroup:[43]"]
        with self.assertRaisesRegex(RuntimeError, "namespace changed"):
            fake.observe()

    def test_capacity_is_not_available_memory(self):
        fake = SyntheticV2()
        fake.controls[ROOT + "/jobs/memory.current"] = str(3 * GIB).encode()
        with self.assertRaisesRegex(RuntimeError, "less than 6 GiB effective available"):
            fake.observe()

    def test_nonregular_or_invalid_root_stat_is_unknown(self):
        fake = SyntheticV2()
        fake.nonregular.add(ROOT + "/memory.stat")
        with self.assertRaisesRegex(RuntimeError, "nonregular"):
            fake.observe()
        fake = SyntheticV2()
        fake.controls[ROOT + "/memory.stat"] += b"invalid record extra\n"
        with self.assertRaisesRegex(RuntimeError, "invalid root memory.stat"):
            fake.observe()

    def test_nested_mount_and_legacy_hierarchy_are_unknown(self):
        fake = SyntheticV2()
        fake.text["/proc/self/mountinfo"] += "37 36 0:33 / /sys/fs/cgroup/jobs rw - tmpfs tmpfs rw\n"
        with self.assertRaisesRegex(RuntimeError, "nested mount"):
            fake.observe()
        fake = SyntheticV2()
        fake.text["/proc/self/cgroup"] = "5:memory:/jobs/test\n"
        with self.assertRaisesRegex(RuntimeError, "v1/mixed"):
            fake.observe()


class RedClassificationTests(unittest.TestCase):
    def test_test_name_is_never_panic_evidence(self):
        name = "issue440_unexpired_401_forces_one_recovery_and_retry"
        header = "thread x panicked at src/coding/codex/official_accounts/usage_refresh_tests.rs:242:70:\n"
        body = 'called `Result::unwrap()` on an `Err` value: "Codex usage request failed: HTTP status client error (401 Mock) for url (http://127.0.0.1:1234/usage)"\n\n'
        driver.verify_business_red(name, header + body)
        for content in [header + "fixture failed " + name, header + 'called `Result::unwrap()` on an `Err` value: "Synthetic Codex HTTP scope"', header.replace(":242:", ":123:") + body, header + body.replace("401 Mock", "401 Unauthorized"), header + body.replace("401 Mock", "403 Mock"), header + body.replace("/usage", "/oauth/token")]:
            with self.assertRaises(AssertionError):
                driver.verify_business_red(name, content)

    def test_rotation_requires_correct_location_and_both_generations(self):
        name = "issue440_rotation_survives_usage_failure"
        log = 'thread x panicked at src/coding/codex/official_accounts/usage_refresh_tests.rs:258:5:\nassertion `left == right` failed\n  left: Object {"refresh_token": String("synthetic-refresh-a-before-500")}\n right: Object {"refresh_token": String("synthetic-refresh-a-after-500")}\n\n'
        driver.verify_business_red(name, log)
        for content in [log.replace(":258:", ":257:"), log.replace("after-500", "before-500")]:
            with self.assertRaises(AssertionError):
                driver.verify_business_red(name, content)


if __name__ == "__main__":
    unittest.main()
