"""Synthetic private files and injected process tables only; no proc or children."""
import copy
import errno
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from batch_lifecycle import BatchLifecycle, LifecycleError, canonical


def member(start, parent):
    return {'start': start, 'ppid': parent, 'pgid': 100, 'sid': 100, 'state': 'S'}


class BatchLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.session = 'a' * 32
        self.wrapper = SimpleNamespace(pid=100)
        self.root_member = member(10, 1)
        self.members = {100: self.root_member}
        self.known = {100: 10}
        self.snapshot = Mock(side_effect=lambda *_: copy.deepcopy(self.members))
        self.table = Mock(side_effect=lambda: copy.deepcopy(self.members))
        self.records = []
        self.control = BatchLifecycle(self.root, self.session, self.snapshot, self.table,
                                      lambda records: self.records.append(copy.deepcopy(records)))

    def tearDown(self):
        self.control.close()
        self.temporary.cleanup()

    def prime(self, pid=201, start=20):
        self.known[pid] = start
        self.control.observe(self.wrapper, self.known, {**self.members, pid: member(start, 100)})

    def request(self, index=0, pid=201, code=0):
        return {'schema': 1, 'session': self.session, 'index': index, 'pid': pid, 'exitCode': code}

    def publish(self, request=None, raw=None):
        request = self.request() if request is None else request
        target = self.root / f'request-{self.control.index}.json'
        target.write_bytes(canonical(request) if raw is None else raw)
        target.chmod(0o600)

    def observe(self):
        self.control.observe(self.wrapper, self.known, self.members)

    def complete_two(self, first_code=0):
        for index in (0, 1):
            self.prime(pid=201 + index, start=20 + index)
            self.publish(self.request(index=index, pid=201 + index, code=first_code if index == 0 else 0))
            self.observe()

    def test_two_exact_generations_complete_with_only_live_wrapper_ownership_root(self):
        self.complete_two()
        self.control.assert_complete()
        self.assertEqual([record['pid'] for record in self.control.records], [201, 202])
        self.assertEqual([record['startTicks'] for record in self.control.records], [20, 21])
        self.assertTrue(all(record['status'] == 'ack-published' for record in self.control.records))
        self.assertEqual([args.args[0] for args in self.snapshot.call_args_list], [100, 100])
        self.assertEqual(self.table.call_count, 2)

    def test_no_checkpoint_never_acknowledges_or_resets_an_external_budget(self):
        self.prime()
        for _ in range(4): self.observe()
        self.assertEqual(self.control.index, 0)
        self.assertFalse(list(self.root.iterdir()))
        self.snapshot.assert_not_called(); self.table.assert_not_called()

    def test_nonzero_exit_may_ack_cleanup_but_can_never_complete_as_green(self):
        self.complete_two(first_code=1)
        self.assertEqual(self.control.index, 2)
        with self.assertRaisesRegex(LifecycleError, 'nonzero-exit'): self.control.assert_complete()

    def test_missing_second_handshake_and_direct_zero_exit_do_not_complete(self):
        with self.assertRaisesRegex(LifecycleError, 'incomplete'): self.control.assert_complete()
        self.prime(); self.publish(); self.observe()
        with self.assertRaisesRegex(LifecycleError, 'incomplete'): self.control.assert_complete()

    def test_unobserved_fast_child_is_unknown_not_inferred_dead_from_missing_pid(self):
        self.publish()
        with self.assertRaisesRegex(LifecycleError, 'never-observed'): self.observe()
        self.snapshot.assert_not_called()
        self.assertFalse((self.root / 'ack-0.json').exists())

    def test_stale_members_snapshot_cannot_hide_current_owned_descendant(self):
        self.prime(); self.publish()
        self.snapshot.side_effect = lambda *_: {**self.members, 301: member(30, 1)}
        with self.assertRaisesRegex(LifecycleError, 'descendants-remain'): self.observe()
        self.assertEqual(self.snapshot.call_count, 1)
        self.assertFalse((self.root / 'ack-0.json').exists())

    def test_reaped_batch_pid_is_not_reused_as_scan_root_and_reuse_refuses_ack(self):
        self.prime(); self.publish()
        self.table.side_effect = lambda: {**self.members, 201: member(999, 1)}
        with self.assertRaisesRegex(LifecycleError, 'pid-reused'): self.observe()
        self.assertEqual(self.snapshot.call_args.args[0], 100)
        self.assertFalse((self.root / 'ack-0.json').exists())

    def test_zombie_batch_is_not_claimed_reaped(self):
        self.prime(); self.publish()
        self.table.side_effect = lambda: {**self.members, 201: {**member(20, 100), 'state': 'Z'}}
        with self.assertRaisesRegex(LifecycleError, 'still-present'): self.observe()

    def test_unknown_wrapper_or_changed_generation_refuses_checkpoint(self):
        self.prime(); self.publish()
        with self.assertRaisesRegex(LifecycleError, 'wrapper-identity'): self.control.observe(self.wrapper, self.known, {})
        with self.assertRaisesRegex(LifecycleError, 'wrapper-generation'):
            self.control.observe(self.wrapper, {**self.known, 100: 11}, {100: member(11, 1)})

    def test_second_direct_child_before_ack_is_overlap_not_new_batch(self):
        self.prime()
        with self.assertRaisesRegex(LifecycleError, 'overlap'):
            self.control.observe(self.wrapper, {**self.known, 202: 21}, {100: self.root_member, 202: member(21, 100)})

    def test_owned_observation_denial_has_no_retry_or_alternate_path(self):
        self.prime(); self.publish()
        self.snapshot.side_effect = PermissionError(errno.EACCES, 'synthetic')
        with self.assertRaisesRegex(LifecycleError, 'batch-owned-observation; errno=13'): self.observe()
        self.assertEqual(self.snapshot.call_count, 1); self.table.assert_not_called()

    def test_process_identity_observation_denial_has_no_ack(self):
        self.prime(); self.publish()
        self.table.side_effect = PermissionError(errno.EACCES, 'synthetic')
        with self.assertRaisesRegex(LifecycleError, 'batch-owned-observation; errno=13'): self.observe()
        self.assertEqual(self.table.call_count, 1)
        self.assertFalse((self.root / 'ack-0.json').exists())

    def test_preexisting_ack_unknown_entry_and_out_of_order_request_rejected(self):
        for name in ('ack-0.json', 'request-1.json', 'arbitrary.json'):
            with self.subTest(name=name):
                p = self.root / name; p.write_bytes(b'{}\n'); p.chmod(0o600)
                with self.assertRaisesRegex(LifecycleError, 'unexpected-entry-or-order'): self.observe()
                p.unlink()

    def test_symlink_nonregular_and_oversized_request_rejected_before_read(self):
        p = self.root / 'request-0.json'
        p.symlink_to('missing')
        with self.assertRaisesRegex(LifecycleError, 'type-or-size'): self.observe()
        p.unlink(); p.mkdir()
        with self.assertRaisesRegex(LifecycleError, 'type-or-size'): self.observe()
        p.rmdir(); p.write_bytes(b'x' * 1025)
        with self.assertRaisesRegex(LifecycleError, 'type-or-size'): self.observe()

    def test_duplicate_extra_wrong_session_index_pid_exit_and_noncanonical_json_rejected(self):
        self.prime()
        invalid = [canonical({**self.request(), 'extra': 1}),
                   canonical({**self.request(), 'session': 'b' * 32}),
                   canonical({**self.request(), 'index': 1}),
                   canonical({**self.request(), 'pid': 999}),
                   canonical({**self.request(), 'pid': True}),
                   canonical({**self.request(), 'pid': 2**53}),
                   canonical({**self.request(), 'exitCode': None}),
                   canonical({**self.request(), 'exitCode': -15}),
                   canonical({**self.request(), 'exitCode': True}),
                   canonical({**self.request(), 'exitCode': 256}),
                   canonical(self.request()).replace(b'"schema":1', b'"schema":1,"schema":1'),
                   json.dumps(self.request(), indent=2).encode(), b'[]\n', b'{bad']
        for raw in invalid:
            with self.subTest(raw=raw):
                self.publish(raw=raw)
                with self.assertRaises(LifecycleError): self.observe()
                self.assertFalse((self.root / 'ack-0.json').exists())

    def test_request_read_denial_and_disappearance_after_lookup_are_not_pending(self):
        self.prime(); self.publish()
        original = os.open
        for code in (errno.EACCES, errno.ENOENT):
            calls = []
            def blocked(name, *args, **kwargs):
                if name == 'request-0.json':
                    calls.append(name); raise OSError(code, 'synthetic')
                return original(name, *args, **kwargs)
            with self.subTest(code=code), patch('batch_lifecycle.os.open', side_effect=blocked):
                with self.assertRaisesRegex(LifecycleError, 'batch-control-read'): self.observe()
            self.assertEqual(calls, ['request-0.json'])

    def test_changed_control_directory_identity_rejected(self):
        old = self.root.with_name(self.root.name + '-old')
        self.root.rename(old); self.root.mkdir(mode=0o700)
        try:
            with self.assertRaisesRegex(LifecycleError, 'directory-identity'): self.observe()
        finally:
            self.root.rmdir(); old.rename(self.root)

    def test_atomic_ack_cannot_overwrite_a_racing_existing_file(self):
        self.prime(); self.publish()
        def racing_snapshot(*_):
            (self.root / 'ack-0.json').write_bytes(b'preexisting')
            (self.root / 'ack-0.json').chmod(0o600)
            return self.members
        self.snapshot.side_effect = racing_snapshot
        with self.assertRaisesRegex(LifecycleError, 'ack-publication'): self.observe()
        self.assertEqual((self.root / 'ack-0.json').read_bytes(), b'preexisting')
        self.assertEqual(self.control.index, 0)

    def test_final_missing_or_changed_checkpoint_cannot_pass(self):
        self.complete_two()
        p = self.root / 'request-0.json'; original = p.read_bytes()
        p.write_bytes(original + b' ')
        with self.assertRaises(LifecycleError): self.control.assert_complete()
        p.unlink()
        with self.assertRaisesRegex(LifecycleError, 'final-inventory'): self.control.assert_complete()

    def test_staging_hardlink_must_be_unlinked_before_request_identity_is_sealed(self):
        self.prime()
        temporary = self.root / '.request-0.tmp'
        temporary.write_bytes(canonical(self.request())); temporary.chmod(0o600)
        os.link(temporary, self.root / 'request-0.json')
        self.observe()
        self.snapshot.assert_not_called()
        self.assertFalse((self.root / 'ack-0.json').exists())
        temporary.unlink()
        self.observe()
        self.assertTrue((self.root / 'ack-0.json').exists())

    def test_wrong_mode_and_invalid_observed_generation_refuse_ack(self):
        self.prime(); self.publish()
        (self.root / 'request-0.json').chmod(0o644)
        with self.assertRaisesRegex(LifecycleError, 'type-or-size'): self.observe()
        (self.root / 'request-0.json').unlink()
        self.control.active = None
        with self.assertRaisesRegex(LifecycleError, 'generation-unknown'):
            self.control.observe(self.wrapper, {**self.known, 201: -1}, {**self.members, 201: member(-1, 100)})


if __name__ == '__main__':
    unittest.main()
