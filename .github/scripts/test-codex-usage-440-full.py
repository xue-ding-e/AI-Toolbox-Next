"""Only static/synthetic driver tests; no Cargo, browser, network or live profiles."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('full_driver', HERE / 'verify-codex-usage-440-full.py')
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


class FullDriverTests(unittest.TestCase):
    def listing(self):
        return '\n'.join(case['name'] + ': test' for case in driver.MANIFEST['ignored'])

    def test_exact_ignored_inventory(self):
        driver.verify_ignored(self.listing())
        self.assertEqual(len(driver.MANIFEST['ignored']), 13)
        self.assertEqual(len(set(c['name'] for c in driver.MANIFEST['ignored'])), 13)

    def test_missing_ignore_rejected(self):
        with self.assertRaises(AssertionError):
            driver.verify_ignored('\n'.join(self.listing().splitlines()[1:]))

    def test_new_ignore_rejected(self):
        with self.assertRaises(AssertionError):
            driver.verify_ignored(self.listing() + '\nunexpected::test: test')

    def test_duplicate_ignore_rejected(self):
        with self.assertRaises(AssertionError):
            driver.verify_ignored(self.listing() + '\n' + self.listing().splitlines()[0])

    def test_full_chain_exact(self):
        scripts = driver.MANIFEST['scripts']
        self.assertEqual(scripts['test'], ' && '.join('pnpm run ' + n for n in driver.MANIFEST['test_chain']))
        self.assertTrue(all(n in scripts for n in driver.MANIFEST['test_chain']))
        self.assertEqual(scripts['build'], 'tsc && vite build')

    def test_environment_drops_credentials_and_profile_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            with patch.dict(os.environ, {'GITHUB_TOKEN': 'synthetic-forbidden', 'OPENAI_API_KEY': 'synthetic-forbidden',
                                        'CODEX_SMOKE_LOCAL_FIXTURE': '/not-a-fixture', 'HOME': '/not-the-profile'}):
                env = driver.isolated_env(Path(temporary) / 'profile', {'rustc': '/synthetic/rustc', 'rustdoc': '/synthetic/rustdoc'})
            self.assertNotIn('GITHUB_TOKEN', env)
            self.assertNotIn('OPENAI_API_KEY', env)
            self.assertNotIn('CODEX_SMOKE_LOCAL_FIXTURE', env)
            self.assertTrue(env['HOME'].startswith(temporary))
            self.assertTrue(env['CARGO_HOME'].startswith(temporary))
            self.assertEqual(env['RUSTC'], '/synthetic/rustc')

    def successful_rust_log(self):
        return '\n'.join('test ' + c['name'] + ' ... ignored' for c in driver.MANIFEST['ignored']) + '\ntest result: ok.\nDoc-tests ai_toolbox_lib\ntest result: ok.\n'

    def test_rust_full_pass(self):
        self.assertEqual(driver.classify_rust(0, self.successful_rust_log()), 'passed')

    def test_no_doc_execution_rejected(self):
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, self.successful_rust_log().replace('Doc-tests', 'not-doc-tests'))

    def test_hidden_test_failure_rejected(self):
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, self.successful_rust_log() + 'test result: FAILED')

    def test_runtime_new_skip_rejected(self):
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, self.successful_rust_log() + 'test extra::test ... ignored\n')

    def test_compile_not_test_failure(self):
        self.assertEqual(driver.classify_rust(101, 'error[E0433]: unresolved name'), 'compile-or-doc-compile-failure')

    def test_real_test_failure_classified(self):
        self.assertEqual(driver.classify_rust(101, 'test result: FAILED'), 'test-failure')

    def test_unknown_failure_is_not_pass(self):
        self.assertEqual(driver.classify_rust(1, 'environment unavailable'), 'environment-or-unclassified-failure')

    def test_sealed_manifest_matches_checkout_contents(self):
        root = HERE.parents[1]
        self.assertEqual(json.loads((root / 'package.json').read_text())['scripts'], driver.MANIFEST['scripts'])
        for case in driver.MANIFEST['ignored']:
            self.assertIn(case['context'], (root / case['path']).read_text())


if __name__ == '__main__':
    unittest.main()
