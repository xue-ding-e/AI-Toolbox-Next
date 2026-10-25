"""Source-only synthetic guard controls; no browser, install or real workload."""
import importlib.util
import os
from pathlib import Path
import unittest
from unittest.mock import patch

os.environ.setdefault('GITHUB_WORKSPACE',str(Path(__file__).parent))
spec=importlib.util.spec_from_file_location('memory_driver',Path(__file__).with_name('validate.py'))
d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
G=1024**3

class MemoryGuardTests(unittest.TestCase):
    def setUp(self):
        d.PEAK_RSS=0;d.MEMORY_PHASE='synthetic'
        d.MEMORY_DIAGNOSTICS={'initial':None,'phase_peaks':{},'recent_samples':[]}
        self.output=patch.object(d,'write_json');self.output.start();self.addCleanup(self.output.stop)
    def admit(self,total=6*G,available=5*G):
        with patch.object(d,'observe_memory',return_value={'effective_capacity_bytes':total,'effective_headroom_bytes':available}):d.admit_memory_budget()
    def sample(self,residents,available=5*G):
        table={pid:{'start':pid+10} for pid in residents}
        def statm(path):
            pid=int(str(path).split('/')[2]);return f'0 {residents[pid]//4096} 1'
        with patch.object(d.os,'getpid',return_value=1),patch.object(d.os,'sysconf',return_value=4096), \
             patch.object(d,'pid_start',side_effect=lambda pid:table[pid]['start']),patch.object(d.Path,'read_text',statm), \
             patch.object(d,'observe_memory',return_value={'effective_capacity_bytes':6*G,'effective_headroom_bytes':available}):
            d.check_rss({pid:v for pid,v in table.items() if pid!=1})
    def test_exact_admission_thresholds(self):self.admit()
    def test_low_total_fails(self):
        with self.assertRaisesRegex(RuntimeError,'capacity'):self.admit(total=6*G-1)
    def test_low_initial_available_fails(self):
        with self.assertRaisesRegex(RuntimeError,'initial effective'):self.admit(available=5*G-1)
    def test_old_observed_peak_is_admitted_as_rss_not_pss(self):
        self.sample({1:512*1024**2,2:3*G})
        self.assertEqual(d.PEAK_RSS,3.5*G)
    def test_exact_limit_and_over_limit(self):
        self.sample({1:d.RSS_LIMIT})
        with self.assertRaisesRegex(RuntimeError,'4.5GiB'):self.sample({1:d.RSS_LIMIT,2:4096})
    def test_runtime_low_water_preserves_failure_evidence(self):
        with self.assertRaisesRegex(RuntimeError,'low-water'):self.sample({1:4096},available=512*1024**2-1)
        self.assertEqual(d.MEMORY_DIAGNOSTICS['phase_peaks']['synthetic']['aggregate_rss_bytes'],4096)
    def test_shared_pages_not_subtracted(self):
        self.sample({1:4096,2:4096})
        peak=d.MEMORY_DIAGNOSTICS['phase_peaks']['synthetic']
        self.assertEqual(peak['aggregate_rss_bytes'],8192)
        self.assertEqual(sum(p['shared_resident_bytes'] for p in peak['processes']),8192)
    def test_details_bounded_but_all_processes_counted(self):
        for _ in range(18):self.sample({pid:4096 for pid in range(1,70)})
        last=d.MEMORY_DIAGNOSTICS['recent_samples'][-1]
        self.assertEqual(last['aggregate_rss_bytes'],69*4096)
        self.assertEqual(len(last['processes']),64);self.assertTrue(last['process_details_truncated'])
        self.assertEqual(len(d.MEMORY_DIAGNOSTICS['recent_samples']),16)
    def test_unavailable_observation_fails_closed(self):
        with patch.object(d,'observe_visible_v2',side_effect=d.ObservationError('proc-meminfo',13,'synthetic denied')):
            with self.assertRaisesRegex(RuntimeError,'unavailable'): d.observe_memory()
        self.assertEqual(d.MEMORY_DIAGNOSTICS['capacity_error']['errno'],13)
        self.assertEqual(d.MEMORY_DIAGNOSTICS['capacity_error']['read_class'],'proc-meminfo')

if __name__=='__main__':unittest.main()
