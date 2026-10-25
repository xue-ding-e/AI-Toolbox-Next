"""Synthetic seal complexity/negative controls; no product or browser execution."""
from test_scanner import driver
import test_scanner
from pathlib import Path
from unittest.mock import patch
import errno
import os
import unittest

class LinearSealTests(unittest.TestCase):
    def setUp(self):
        # Reuse only the temporary-tree setup, not the active-install tests:
        # this suite deliberately changes the fixture to a quiescent seal.
        # The original scanner suite still runs every active-install assertion.
        test_scanner.ScannerTests.setUp(self)
        driver.DEPENDENCY_INSTALL_ACTIVE=False
        driver.ACTIVE=None
        self.rootdeps=self.root/'s/node_modules'
        for name in ('a','t','h','c','x'): (self.root/name).mkdir()

    def tearDown(self):
        test_scanner.ScannerTests.tearDown(self)

    def snapshot(self):
        # Resource readings are deterministic in synthetic tests; production
        # health/deadline calls are not removed from the candidate.
        with patch.object(driver,'check_storage_health'):
            return driver.tree_snapshot(self.rootdeps)

    def test_entry_work_scales_linearly_and_store_is_sampled_once(self):
        for n in (8,16,32):
            root=self.rootdeps/f'n{n}'; root.mkdir()
            for i in range(n):
                d=root/str(i); d.mkdir(); (d/'data').write_bytes(b'xyz')
            real_walk=driver.os.walk; real_read=driver.read_regular
            visits=[]; reads=[]
            def counted_walk(*a,**kw):
                for entry in real_walk(*a,**kw): visits.append(entry[0]); yield entry
            def counted_read(path): reads.append(path); return real_read(path)
            with patch.object(driver.os,'walk',side_effect=counted_walk), \
                 patch.object(driver,'read_regular',side_effect=counted_read), \
                 patch.object(driver,'sample_dependency_budget',wraps=driver.sample_dependency_budget) as sample, \
                 patch.object(driver,'check_disk_budgets',side_effect=AssertionError('nested global scan')):
                result=self.snapshot()
            # Earlier trees remain: exact linear cumulative work, no timing claim.
            total=sum(v for v in (8,16,32) if v<=n)
            groups=(8,16,32).index(n)+1
            self.assertEqual(len(reads),total)
            self.assertEqual(len(visits),1+groups+total)
            self.assertEqual(len(result),1+groups+2*total)
            self.assertEqual(sample.call_count,1)
            self.assertEqual(sample.call_args.args,([('store',)],))
            for path in reads: self.assertEqual(result[str(path.relative_to(self.rootdeps))]['sha256'],driver.digest(b'xyz'))

    def test_walk_permission_error_cannot_silently_omit_subtree(self):
        def broken(root,**kw):
            kw['onerror'](PermissionError(errno.EACCES,'synthetic'))
            yield
        with patch.object(driver.os,'walk',side_effect=broken), self.assertRaises(PermissionError): self.snapshot()

    def test_disappearing_file_fails_strict_seal(self):
        p=self.rootdeps/'gone'; p.write_bytes(b'x')
        real=driver.read_regular
        def disappear(path): path.unlink(); return real(path)
        with patch.object(driver,'read_regular',side_effect=disappear), self.assertRaises(FileNotFoundError): self.snapshot()

    def test_changed_bytes_change_final_seal(self):
        p=self.rootdeps/'data'; p.write_bytes(b'first')
        before=self.snapshot(); p.write_bytes(b'other')
        self.assertNotEqual(self.snapshot(),before)

    def test_regular_identity_change_is_rejected(self):
        p=self.rootdeps/'data'; p.write_bytes(b'first'); real=driver.read_regular
        def replace(path):
            new=path.with_name('replacement'); new.write_bytes(b'other'); new.replace(path)
            return real(path)
        with patch.object(driver,'read_regular',side_effect=replace), self.assertRaisesRegex(RuntimeError,'identity changed'): self.snapshot()

    def test_combined_store_plus_modules_budget_rejected(self):
        (self.rootdeps/'data').write_bytes(b'x')
        sample={'sampling_completed':True,'entries':0,'regular_bytes':3*1024**3}
        with patch.object(driver,'sample_dependency_budget',return_value=sample), self.assertRaisesRegex(RuntimeError,'3GiB'): self.snapshot()

    def test_combined_entry_limit_rejected(self):
        (self.rootdeps/'data').write_bytes(b'x')
        sample={'sampling_completed':True,'entries':300000,'regular_bytes':0}
        with patch.object(driver,'sample_dependency_budget',return_value=sample), self.assertRaisesRegex(RuntimeError,'300000'): self.snapshot()

    def test_full_byte_read_still_obeys_deadline(self):
        (self.rootdeps/'data').write_bytes(b'x')
        def expired(): raise RuntimeError('synthetic deadline')
        real=driver.read_regular
        def expire_read(path):
            with patch.object(driver,'deadline',side_effect=expired): return real(path)
        with patch.object(driver,'read_regular',side_effect=expire_read), self.assertRaisesRegex(RuntimeError,'deadline'): self.snapshot()

    def test_periodic_scan_interval_begins_at_completion(self):
        clock=[10.0]; driver.SCAN_AT=0
        def sample(): clock[0]+=0.87
        with patch.object(driver.time,'monotonic',side_effect=lambda:clock[0]), \
             patch.object(driver,'check_storage_health'), patch.object(driver,'check_output_budget'), \
             patch.object(driver,'sample_dependency_budget',side_effect=sample) as observed:
            driver.check_disk_budgets(); self.assertEqual(driver.SCAN_AT,10.87)
            driver.check_disk_budgets(); self.assertEqual(observed.call_count,1)
            clock[0]+=0.501; driver.check_disk_budgets(); self.assertEqual(observed.call_count,2)

if __name__=='__main__': unittest.main()
