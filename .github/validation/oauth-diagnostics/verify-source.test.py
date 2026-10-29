import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('verify_source', Path(__file__).with_name('verify-source.py'))
v = importlib.util.module_from_spec(spec)
spec.loader.exec_module(v)

class TrackedIdentity(unittest.TestCase):
    def test_clean_and_staged_or_unstaged_drift_against_head(self):
        with tempfile.TemporaryDirectory(prefix='oauth-git-gate-') as root:
            root = Path(root)
            def git(*args):
                subprocess.run(['git', '-C', str(root), *args], check=True, capture_output=True)
            git('init')
            git('config', 'user.name', 'Synthetic Test')
            git('config', 'user.email', 'synthetic@example.invalid')
            f = root / 'source.rs'; f.write_text('original\n')
            git('add', 'source.rs'); git('commit', '-m', 'synthetic baseline')
            v.assert_tracked_clean(root)
            f.write_text('unstaged drift\n')
            with self.assertRaises(AssertionError): v.assert_tracked_clean(root)
            git('add', 'source.rs')
            # A worktree-index-only comparison is empty here, but the gate must reject it.
            self.assertEqual(subprocess.check_output(['git', '-C', str(root), 'diff', '--name-only']), b'')
            with self.assertRaises(AssertionError): v.assert_tracked_clean(root)
            git('reset', '--hard', 'HEAD')
            v.assert_tracked_clean(root)

if __name__ == '__main__': unittest.main()
