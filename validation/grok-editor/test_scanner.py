"""Synthetic metadata-only controls. No package install, browser or product test."""
import errno
import importlib.util
import os
from pathlib import Path
import shutil
import tempfile
import stat
from types import SimpleNamespace
import unittest
from unittest.mock import patch

os.environ.setdefault('GITHUB_WORKSPACE',str(Path(__file__).parent))
spec=importlib.util.spec_from_file_location('driver',Path(__file__).with_name('validate.py'))
driver=importlib.util.module_from_spec(spec); spec.loader.exec_module(driver)

class ScannerTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(); self.root=Path(self.tmp.name)
        for p in ['store','s/node_modules','e']: (self.root/p).mkdir(parents=True)
        driver.PRIVATE=self.root; st=self.root.stat(); driver.PRIVATE_ID=(st.st_dev,st.st_ino)
        driver.EVIDENCE=self.root/'e'; driver.DEPENDENCY_INSTALL_STARTED=True
        driver.DEPENDENCY_INSTALL_ACTIVE=True; driver.deadline=lambda:None
        driver.DEPENDENCY_SAMPLES={'samples':0,'enoent_misses':0,'last_samples':[]}
    def tearDown(self): self.tmp.cleanup()
    def sample(self):
        driver.sample_dependency_budget()
        return driver.DEPENDENCY_SAMPLES['last_samples'][-1]
    def mutation(self,kind,active=True):
        driver.DEPENDENCY_INSTALL_ACTIVE=active
        item=self.root/'store/transient'; item.mkdir(); (item/'payload').write_bytes(b'abc')
        real=os.stat; done=False
        def observed(name,*a,**kw):
            nonlocal done
            if name=='transient' and not done:
                done=True
                if kind=='rename': item.rename(self.root/'store/renamed')
                else: shutil.rmtree(item)
            return real(name,*a,**kw)
        with patch.object(driver.os,'stat',side_effect=observed): return self.sample()
    def test_active_rename_is_sampled_miss_not_verification(self):
        r=self.mutation('rename'); self.assertEqual(r['enoent_misses'],1)
        self.assertTrue(r['sampling_completed']); self.assertTrue(r['active_install'])
        self.assertEqual((self.root/'store/renamed/payload').read_bytes(),b'abc')
    def test_active_disappearance_is_sampled_miss(self):
        self.assertEqual(self.mutation('remove')['enoent_misses'],1)
    def test_post_install_disappearance_still_fails(self):
        with self.assertRaises(FileNotFoundError): self.mutation('remove',False)
        self.assertFalse(driver.DEPENDENCY_SAMPLES['last_samples'][-1]['sampling_completed'])
    def test_symlinks_not_followed(self):
        with tempfile.TemporaryDirectory() as outside:
            target=Path(outside); (target/'secret').write_bytes(b'outside')
            (self.root/'store/link').symlink_to(target,target_is_directory=True)
            (self.root/'store/broken').symlink_to(target/'missing')
            with patch.object(driver.os,'readlink',side_effect=AssertionError('must not read target')):
                r=self.sample()
            self.assertEqual(r['regular_bytes'],0); self.assertEqual(r['entries'],2)
    def test_permission_and_io_errors_not_swallowed(self):
        (self.root/'store/item').touch(); real=os.stat
        for code in [errno.EACCES,errno.EIO]:
            def observed(name,*a,**kw):
                if name=='item': raise OSError(code,'synthetic')
                return real(name,*a,**kw)
            with patch.object(driver.os,'stat',side_effect=observed), self.assertRaises(OSError) as caught:
                self.sample()
            self.assertEqual(caught.exception.errno,code)
    def test_directory_replaced_by_symlink_fails(self):
        child=self.root/'store/child'; child.mkdir(); real=os.open; done=False
        def observed(name,flags,*a,**kw):
            nonlocal done
            if name=='child' and not done:
                done=True; child.rmdir(); child.symlink_to(self.root/'e',target_is_directory=True)
            return real(name,flags,*a,**kw)
        with patch.object(driver.os,'open',side_effect=observed), self.assertRaises(OSError): self.sample()
    def test_metadata_size_budget_still_fails(self):
        # Sparse synthetic file: metadata accounting only, no 3GiB allocation/read.
        with (self.root/'store/large').open('wb') as f: f.truncate(3*1024**3+1)
        with self.assertRaisesRegex(RuntimeError,'3GiB'): self.sample()
    def test_elapsed_budget_still_fails(self):
        with patch.object(driver.time,'monotonic',side_effect=[0,11,11]), self.assertRaisesRegex(RuntimeError,'10s'):
            self.sample()
    def test_entry_limit_still_fails(self):
        class Entries:
            def __enter__(self): return (SimpleNamespace(name='sample') for _ in range(300001))
            def __exit__(self,*args): return False
        with patch.object(driver.os,'scandir',return_value=Entries()), \
             patch.object(driver.os,'stat',return_value=SimpleNamespace(st_mode=stat.S_IFLNK)), \
             patch.object(driver.time,'monotonic',return_value=0), \
             self.assertRaisesRegex(RuntimeError,'300000'):
            self.sample()
    def test_final_empty_scan_elapsed_is_not_success(self):
        clock=[0]
        real=driver.os.scandir
        count=[0]
        class EmptyScan:
            def __enter__(self): return iter(())
            def __exit__(self,*args):
                clock[0]=11
                return False
        def observed(fd):
            count[0]+=1
            return EmptyScan() if count[0]==2 else real(fd)
        with patch.object(driver.os,'scandir',side_effect=observed), \
             patch.object(driver.time,'monotonic',side_effect=lambda:clock[0]), \
             self.assertRaisesRegex(RuntimeError,'10s'):
            self.sample()
        self.assertFalse(driver.DEPENDENCY_SAMPLES['last_samples'][-1]['sampling_completed'])
    def test_depth_limit_still_fails(self):
        node=self.root/'store'
        for _ in range(129):
            node=node/'d'; node.mkdir()
        with self.assertRaisesRegex(RuntimeError,'depth exceeded128'):
            self.sample()
        self.assertFalse(driver.DEPENDENCY_SAMPLES['last_samples'][-1]['sampling_completed'])
    def test_regular_directory_inode_replacement_fails(self):
        child=self.root/'store/child'; child.mkdir(); real=os.open; done=False
        def observed(name,flags,*a,**kw):
            nonlocal done
            if name=='child' and not done:
                done=True
                child.rename(self.root/'store/original-retained')
                child.mkdir()
            return real(name,flags,*a,**kw)
        with patch.object(driver.os,'open',side_effect=observed), \
             self.assertRaisesRegex(RuntimeError,'directory identity changed'):
            self.sample()
        self.assertFalse(driver.DEPENDENCY_SAMPLES['last_samples'][-1]['sampling_completed'])
    def test_active_open_stage_enoent_is_counted(self):
        child=self.root/'store/child'; child.mkdir(); real=os.open; done=False
        def observed(name,flags,*a,**kw):
            nonlocal done
            if name=='child' and not done:
                done=True; child.rmdir()
            return real(name,flags,*a,**kw)
        with patch.object(driver.os,'open',side_effect=observed): r=self.sample()
        self.assertEqual(r['enoent_misses'],1)
        self.assertTrue(r['sampling_completed'])
    def test_before_install_has_no_dependency_claim(self):
        driver.DEPENDENCY_INSTALL_STARTED=False; driver.sample_dependency_budget()
        self.assertEqual(driver.DEPENDENCY_SAMPLES['samples'],0)

if __name__=='__main__': unittest.main()
