"""Only static/synthetic driver tests; no Cargo, browser, network or live profiles."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import subprocess
import sys
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

    def inventory(self):
        return [case['name'] for case in driver.MANIFEST['ignored']] + ['synthetic::positive']

    def test_zero_tests_rejected(self):
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, self.successful_rust_log().replace('1 passed', '0 passed'), self.inventory())

    def test_missing_executed_identity_rejected(self):
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, self.successful_rust_log().replace('test synthetic::positive ... ok', ''), self.inventory())

    def test_filtered_tests_rejected(self):
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, self.successful_rust_log().replace('0 filtered out', '1 filtered out'), self.inventory())

    def test_ignored_only_inventory_rejected(self):
        with self.assertRaises(AssertionError):
            driver.full_inventory(self.listing())

    def test_full_inventory(self):
        self.assertEqual(len(driver.full_inventory(self.listing() + '\nsynthetic::positive: test')), 14)

    def test_optimized_python_rejected(self):
        result = subprocess.run([sys.executable, '-B', '-O', str(HERE / 'verify-codex-usage-440-full.py')], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Optimized Python is unsupported', result.stderr)

    def successful_rust_log(self):
        return '\n'.join('test ' + c['name'] + ' ... ignored' for c in driver.MANIFEST['ignored']) + '\ntest synthetic::positive ... ok\ntest result: ok. 1 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out;\nDoc-tests ai_toolbox_lib\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n'

    def test_rust_full_pass(self):
        self.assertEqual(driver.classify_rust(0, self.successful_rust_log(), self.inventory()), 'passed')

    def test_no_doc_execution_rejected(self):
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, self.successful_rust_log().replace('Doc-tests', 'not-doc-tests'), self.inventory())

    def test_hidden_test_failure_rejected(self):
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, self.successful_rust_log() + 'test result: FAILED', self.inventory())

    def test_runtime_new_skip_rejected(self):
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, self.successful_rust_log() + 'test extra::test ... ignored\n', self.inventory())

    def test_compile_not_test_failure(self):
        self.assertEqual(driver.classify_rust(101, 'error[E0433]: unresolved name', []), 'compile-or-doc-compile-failure')

    def test_real_test_failure_classified(self):
        self.assertEqual(driver.classify_rust(101, 'test result: FAILED', []), 'test-failure')

    def test_unknown_failure_is_not_pass(self):
        self.assertEqual(driver.classify_rust(1, 'environment unavailable', []), 'environment-or-unclassified-failure')

    def test_sealed_manifest_matches_checkout_contents(self):
        root = HERE.parents[1]
        self.assertEqual(json.loads((root / 'package.json').read_text())['scripts'], driver.MANIFEST['scripts'])
        for case in driver.MANIFEST['ignored']:
            self.assertIn(case['context'], (root / case['path']).read_text())

    def test_upstream_focused_and_probe_inventory_are_nonempty_and_distinct(self):
        self.assertEqual([len(case['tests']) for case in driver.MANIFEST['focused'].values()], [19, 31])
        self.assertEqual(len(driver.MANIFEST['probe']['tests']), 3)
        names = [name for case in driver.MANIFEST['focused'].values() for name in case['tests']]
        names += driver.MANIFEST['probe']['tests']
        self.assertEqual(len(names), len(set(names)))
        self.assertNotEqual(driver.MANIFEST['probe']['sha'], driver.MANIFEST['product_sha'])

    def test_focused_runtime_inventory_rejects_missing_duplicate_and_extra(self):
        driver.verify_focused_inventory('a: test\nb: test\n', ['a', 'b'])
        for listing in ['', 'a: test\n', 'a: test\na: test\nb: test\n', 'a: test\nb: test\nc: test\n']:
            with self.assertRaises(AssertionError):
                driver.verify_focused_inventory(listing, ['a', 'b'])
        with self.assertRaises(AssertionError):
            driver.verify_focused_inventory('', [])

    def focused_log(self):
        return 'test a ... ok\ntest b ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out;\n'

    def test_focused_success_requires_every_expected_execution(self):
        self.assertEqual(driver.classify_focused(0, self.focused_log(), ['a', 'b']), 'passed')
        variants = [
            self.focused_log().replace('test b ... ok\n', ''),
            self.focused_log().replace('test b ... ok', 'test a ... ok'),
            self.focused_log().replace('2 passed', '0 passed'),
            self.focused_log().replace('test b ... ok', 'test b ... ignored'),
            self.focused_log() + 'test result: FAILED',
        ]
        for content in variants:
            with self.assertRaises(AssertionError):
                driver.classify_focused(0, content, ['a', 'b'])
        with self.assertRaises(AssertionError):
            driver.classify_focused(0, '', [])

    def test_focused_compile_failure_is_not_probe_assertion_failure(self):
        self.assertEqual(driver.classify_focused(101, 'error[E0433]: unresolved name', ['a']), 'compile-or-doc-compile-failure')
        self.assertEqual(driver.classify_focused(101, 'test result: FAILED', ['a']), 'test-failure')
        self.assertEqual(driver.classify_focused(1, 'timeout', ['a']), 'environment-or-unclassified-failure')

    def test_new_workflow_is_pinned_to_product_and_separate_probe(self):
        root = HERE.parents[1]
        workflow = (root / '.github/workflows/maintenance-validation.yml').read_text().split('  codex-usage-440-full-product:', 1)[1]
        self.assertIn(driver.REF, workflow)
        self.assertIn('ref: ' + driver.MANIFEST['product_sha'], workflow)
        self.assertIn('ref: ' + driver.MANIFEST['probe']['sha'], workflow)
        self.assertIn('fetch-depth: 2', workflow)
        self.assertIn('product evidence "$EXPECTED_VALIDATION_SHA" probes', workflow)
        self.assertNotIn('ed843373628fed574c20d61ffa259a01bd0c60ee', workflow)

    def test_probe_source_rejects_any_changed_tree_or_non_test_delta(self):
        probe = driver.MANIFEST['probe']
        base_answers = [probe['sha'], probe['tree'], driver.MANIFEST['product_sha'], '', probe['path']]
        from hashlib import sha256
        delta = b'synthetic test-only patch'
        with patch.dict(probe, {'delta_sha256': sha256(delta).hexdigest()}):
            with patch.object(driver.subprocess, 'check_output', side_effect=base_answers + [delta]):
                driver.verify_probe_source(Path('/synthetic'))
            for index in range(5):
                answers = list(base_answers)
                answers[index] = 'unexpected'
                with patch.object(driver.subprocess, 'check_output', side_effect=answers + [delta]):
                    with self.assertRaises(AssertionError):
                        driver.verify_probe_source(Path('/synthetic'))
            with patch.object(driver.subprocess, 'check_output', side_effect=base_answers + [b'changed']):
                with self.assertRaises(AssertionError):
                    driver.verify_probe_source(Path('/synthetic'))

    def test_lib_artifact_requires_a_fresh_actual_test_executable(self):
        artifact = {'reason': 'compiler-artifact', 'profile': {'test': True},
                    'target': {'name': 'ai_toolbox_lib'}, 'executable': '/synthetic/lib-test', 'fresh': False}
        with patch.object(Path, 'read_bytes', return_value=b'synthetic binary'):
            result = driver.fresh_lib_artifact(json.dumps(artifact))
            self.assertEqual(result['binary'], '/synthetic/lib-test')
            self.assertEqual(len(result['sha256']), 64)
            for changed in [dict(artifact, fresh=True), dict(artifact, executable=None),
                            dict(artifact, profile={'test': False}), dict(artifact, target={'name': 'other'})]:
                with self.assertRaises(AssertionError):
                    driver.fresh_lib_artifact(json.dumps(changed))
            with self.assertRaises(AssertionError):
                driver.fresh_lib_artifact(json.dumps(artifact) + '\n' + json.dumps(artifact))
            with self.assertRaises(AssertionError):
                driver.fresh_lib_artifact('compiler error without an executable')


if __name__ == '__main__':
    unittest.main()
