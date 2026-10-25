"""Injected metadata only: never read real proc/cgroup or run a browser."""
from contextlib import ExitStack
import errno
import io
import os
from pathlib import Path
import stat
import traceback
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import memory_observation as m

GIB = 1024**3
HOSTED = {'GITHUB_ACTIONS': 'true', 'RUNNER_ENVIRONMENT': 'github-hosted', 'RUNNER_OS': 'Linux'}


class FakeMetadata:
    def __init__(self):
        self.text = {
            '/proc/meminfo': 'MemTotal: 8388608 kB\nMemAvailable: 7340032 kB\n',
            '/proc/self/cgroup': '0::/job/child\n',
            '/proc/self/mountinfo': '1 0 0:1 / /cg rw - cgroup2 cgroup rw\n',
        }
        self.controls = {
            '/cg/cgroup.controllers': b'memory cpu',
            '/cg/memory.stat': b'anon 1\nfile 2\nkernel_stack 3\npagetables 4\n',
            '/cg/job/memory.max': str(6*GIB).encode(),
            '/cg/job/memory.current': str(GIB).encode(),
            '/cg/job/child/memory.max': b'max',
            '/cg/job/child/memory.current': b'10',
        }
        self.calls = []
        self.descriptors = {}
        self.next_fd = 10
        self.fail = {}
        self.nonregular = set()
        self.resolve_as = '/cg'
        self.counts = {}
        self.migrate = None
        self.closed = []
        self.namespace = 'cgroup:[123]'
        self.change_inode = False

    def touch(self, operation, path):
        self.calls.append((operation, path))
        key = (operation, path)
        self.counts[key] = self.counts.get(key, 0)+1
        if key in self.fail:
            raise OSError(self.fail[key], 'sensitive host path must not leak: '+path)
        return self.counts[key]

    def read_text(self, path, *args, **kwargs):
        name = str(path)
        count = self.touch('read_text', name)
        assert name in self.text, 'unexpected path read'
        if self.migrate == name and count > 1:
            return self.text[name] + 'changed'
        return self.text[name]

    def readlink(self, path):
        assert path == '/proc/self/ns/cgroup', 'another process namespace is forbidden'
        count = self.touch('readlink', path)
        return 'cgroup:[456]' if self.migrate == path and count > 1 else self.namespace

    def resolve(self, path, strict=False):
        assert strict and str(path) == '/cg'
        self.touch('resolve', str(path))
        return Path(self.resolve_as)

    def open(self, path, flags, *, dir_fd=None):
        name = str(path) if dir_fd is None else self.descriptors[dir_fd]+'/'+str(path)
        self.touch('open', name)
        assert flags & os.O_NOFOLLOW, 'every directory/control must reject symlinks'
        if flags & os.O_DIRECTORY:
            assert name in ['/cg', '/cg/job', '/cg/job/child']
        elif name not in self.controls:
            raise FileNotFoundError(errno.ENOENT, 'missing synthetic metadata')
        fd = self.next_fd
        self.next_fd += 1
        self.descriptors[fd] = name
        return fd

    def read(self, fd, count):
        name = self.descriptors[fd]
        self.touch('read', name)
        return self.controls[name][:count]

    def fstat(self, fd):
        name = self.descriptors[fd]
        count = self.touch('fstat', name)
        mode = stat.S_IFDIR if name in ['/cg', '/cg/job', '/cg/job/child'] else stat.S_IFREG
        if name in self.nonregular:
            mode = stat.S_IFIFO
        return SimpleNamespace(st_mode=mode, st_dev=1, st_ino=fd+(1 if self.change_inode and count > 1 else 0))

    def close(self, fd):
        assert fd in self.descriptors
        self.closed.append(fd)

    def observe(self, env=HOSTED):
        with ExitStack() as stack:
            stack.enter_context(patch.dict(os.environ, env, clear=True))
            stack.enter_context(patch.object(Path, 'read_text', lambda path, *a, **kw: self.read_text(path, *a, **kw)))
            stack.enter_context(patch.object(Path, 'resolve', lambda path, *a, **kw: self.resolve(path, *a, **kw)))
            for name in ['readlink', 'open', 'read', 'fstat', 'close']:
                stack.enter_context(patch.object(m.os, name, getattr(self, name)))
            return m.observe_memory()


class ObservationTests(unittest.TestCase):
    def test_success_limits_include_every_visible_ancestor_and_no_logging(self):
        f = FakeMetadata()
        with patch('sys.stdout', new_callable=io.StringIO) as output:
            value = f.observe()
        self.assertEqual(output.getvalue(), '')
        self.assertEqual(value['MemTotal'], 8*GIB)
        self.assertEqual(value['MemAvailable'], 7*GIB)
        self.assertEqual(value['effective_capacity_bytes'], 6*GIB)
        self.assertEqual(value['effective_headroom_bytes'], 5*GIB)
        self.assertEqual([r['depth'] for r in value['visible_cgroups']], [0, 1, 2])
        self.assertIsNone(value['visible_cgroups'][2]['cap_bytes'])
        self.assertEqual(set(f.closed), set(f.descriptors))
        self.assertFalse(any('/proc/1/' in path for _, path in f.calls))

    def test_low_capacity_and_low_headroom_are_returned_not_upgraded(self):
        f = FakeMetadata()
        f.controls['/cg/job/child/memory.max'] = b'100'
        f.controls['/cg/job/child/memory.current'] = b'120'
        value = f.observe()
        self.assertEqual(value['effective_capacity_bytes'], 100)
        self.assertEqual(value['effective_headroom_bytes'], 0)

    def test_host_low_water_remains_in_effective_minimum(self):
        f = FakeMetadata()
        f.text['/proc/meminfo'] = 'MemTotal: 8388608 kB\nMemAvailable: 1 kB\n'
        self.assertEqual(f.observe()['effective_headroom_bytes'], 1024)

    def test_denied_read_is_once_no_fallback_and_diagnostic_has_no_path(self):
        for operation, path in [('read_text', '/proc/meminfo'), ('readlink', '/proc/self/ns/cgroup'),
                                ('open', '/cg/memory.max'), ('read', '/cg/job/memory.current'),
                                ('resolve', '/cg')]:
            with self.subTest(operation=operation, path=path):
                f = FakeMetadata(); f.fail[(operation, path)] = errno.EACCES
                with self.assertRaises(m.ObservationError) as result:
                    f.observe()
                self.assertEqual(f.counts[(operation, path)], 1)
                self.assertEqual(result.exception.errno, errno.EACCES)
                self.assertNotIn(path, str(result.exception))
                self.assertNotIn('sensitive', str(result.exception.diagnostic()))
                self.assertNotIn('sensitive', ''.join(traceback.format_exception(result.exception)))
                self.assertEqual(set(f.closed), set(f.descriptors))

    def test_missing_root_or_child_directory_stops(self):
        for path in ['/cg', '/cg/job', '/cg/job/child']:
            f = FakeMetadata(); f.fail[('open', path)] = errno.ENOENT
            with self.assertRaises(m.ObservationError) as result: f.observe()
            self.assertEqual(result.exception.errno, errno.ENOENT)
            self.assertEqual(f.counts[('open', path)], 1)

    def test_root_control_disappearing_after_open_is_not_proof_of_absence(self):
        f = FakeMetadata(); f.controls['/cg/memory.max'] = b'max'
        f.fail[('read', '/cg/memory.max')] = errno.ENOENT
        with self.assertRaises(m.ObservationError) as result: f.observe()
        self.assertEqual(result.exception.errno, errno.ENOENT)
        self.assertEqual(result.exception.read_class, 'cgroup-v2-depth-0-memory.max')

    def test_only_root_scalar_enoent_is_accepted(self):
        for path in ['/cg/memory.stat', '/cg/cgroup.controllers', '/cg/job/memory.max', '/cg/job/child/memory.current']:
            with self.subTest(path=path):
                f = FakeMetadata(); del f.controls[path]
                with self.assertRaises(m.ObservationError) as result:
                    f.observe()
                self.assertEqual(result.exception.errno, errno.ENOENT)
        for name in ['memory.max', 'memory.current']:
            f = FakeMetadata(); f.controls['/cg/'+name] = b'max' if name.endswith('max') else b'0'
            with self.assertRaises(m.ObservationError): f.observe()

    def test_root_scalar_non_enoent_never_means_unlimited(self):
        for error in [errno.EACCES, errno.EPERM, errno.EIO, errno.ELOOP]:
            f = FakeMetadata(); f.fail[('open', '/cg/memory.max')] = error
            with self.assertRaises(m.ObservationError) as result: f.observe()
            self.assertEqual(result.exception.errno, error)

    def test_unsupported_runner_stops_before_any_read(self):
        for env in [{}, dict(HOSTED, RUNNER_ENVIRONMENT='self-hosted'), dict(HOSTED, RUNNER_OS='Windows')]:
            f = FakeMetadata()
            with self.assertRaises(m.ObservationError): f.observe(env)
            self.assertEqual(f.calls, [])

    def test_hidden_nested_v1_and_ambiguous_mounts_rejected(self):
        variants = ['1 0 0:1 /hidden /cg rw - cgroup2 cgroup rw\n',
                    '1 0 0:1 / /cg rw - cgroup2 cgroup rw\n2 1 0:2 / /cg/hidden rw - tmpfs none rw\n',
                    '1 0 0:1 / /cg rw - cgroup cgroup rw\n',
                    '1 0 0:1 / /cg rw - cgroup2 cgroup rw\n2 1 0:2 / /other rw - cgroup2 cgroup rw\n']
        for mount in variants:
            f = FakeMetadata(); f.text['/proc/self/mountinfo'] = mount
            with self.assertRaises(m.ObservationError): f.observe()

    def test_membership_encoding_and_v1_rejected(self):
        for membership in ['1:memory:/job\n', '0::/job/../escape\n', '0::/job\\040hidden\n', '0::/job//child\n']:
            f = FakeMetadata(); f.text['/proc/self/cgroup'] = membership
            with self.assertRaises(m.ObservationError): f.observe()

    def test_migration_or_namespace_change_stops(self):
        for path in ['/proc/self/cgroup', '/proc/self/mountinfo', '/proc/self/ns/cgroup']:
            f = FakeMetadata(); f.migrate = path
            with self.assertRaises(m.ObservationError): f.observe()
        f = FakeMetadata(); f.change_inode = True
        with self.assertRaises(m.ObservationError): f.observe()

    def test_nonregular_alias_and_child_symlink_rejected(self):
        f = FakeMetadata(); f.nonregular.add('/cg/memory.stat')
        with self.assertRaises(m.ObservationError): f.observe()
        f = FakeMetadata(); f.resolve_as = '/other'
        with self.assertRaises(m.ObservationError): f.observe()
        f = FakeMetadata(); f.fail[('open', '/cg/job')] = errno.ELOOP
        with self.assertRaises(m.ObservationError) as result: f.observe()
        self.assertEqual(result.exception.errno, errno.ELOOP)

    def test_oversize_malformed_and_incomplete_controls_rejected(self):
        for name, value in [('/cg/job/memory.max', b'1'*129), ('/cg/job/memory.current', b'-1'),
                            ('/cg/memory.stat', b'x'*65537), ('/cg/memory.stat', b'anon 1\n'),
                            ('/cg/memory.stat', b'anon 1\nanon 1\n'), ('/cg/cgroup.controllers', b'cpu')]:
            f = FakeMetadata(); f.controls[name] = value
            with self.assertRaises(m.ObservationError): f.observe()


if __name__ == '__main__':
    unittest.main()
