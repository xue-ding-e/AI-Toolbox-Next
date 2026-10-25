"""Only static/synthetic driver tests; no Cargo, browser, network or live profiles."""
import importlib.util
import hashlib
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
                env = driver.isolated_env(Path(temporary) / 'profile', {'rustc': '/synthetic/rustc', 'rustdoc': '/synthetic/rustdoc'}, Path(temporary) / 'short-tmp')
            self.assertNotIn('GITHUB_TOKEN', env)
            self.assertNotIn('OPENAI_API_KEY', env)
            self.assertNotIn('CODEX_SMOKE_LOCAL_FIXTURE', env)
            self.assertTrue(env['HOME'].startswith(temporary))
            self.assertTrue(env['CARGO_HOME'].startswith(temporary))
            self.assertEqual(env['RUSTC'], '/synthetic/rustc')
            self.assertEqual(env['TMPDIR'], str(Path(temporary) / 'short-tmp'))

    def test_private_short_tmp_is_owned_unique_and_removed(self):
        with driver.short_private_tmpdir() as first:
            with driver.short_private_tmpdir() as second:
                self.assertNotEqual(first, second)
                self.assertEqual(first.stat().st_uid, os.getuid())
                self.assertEqual(first.stat().st_mode & 0o777, 0o700)
                self.assertLessEqual(len(os.fsencode(first)), 24)
            self.assertFalse(second.exists())
            self.assertTrue(first.exists())
        self.assertFalse(first.exists())

    def test_short_tmp_leaves_budget_for_exact_wsl_fixture_socket_shape(self):
        # Path-budget assertion only, not a native socket bind or Rust test.
        with driver.short_private_tmpdir() as root:
            with tempfile.TemporaryDirectory(prefix='.tmp', dir=root) as fixture:
                source = Path(fixture) / 'source'
                source.mkdir()
                path = source / '.ai-toolbox-owner-0000000000000001-0000000000000002.sock'
                self.assertLess(len(os.fsencode(path)), 108)
                self.assertFalse(path.exists())
        old_root = '/home/runner/work/AI-Toolbox-Next/AI-Toolbox-Next/private-full-profile/tmpdir'
        old_path = Path(old_root) / '.tmpABCDEF' / 'source' / '.ai-toolbox-owner-0000000000000001-0000000000000002.sock'
        self.assertGreaterEqual(len(os.fsencode(old_path)), 108)

    def test_validation_exception_cleans_only_owned_tmp(self):
        for error_type in [RuntimeError, InterruptedError, KeyboardInterrupt]:
            with self.subTest(error=error_type.__name__), driver.short_private_tmpdir() as unrelated:
                marker = unrelated / 'must-remain'
                marker.write_text('unrelated synthetic owner')
                seen = []
                def failing(root):
                    seen.append(root)
                    (root / 'synthetic-child-output').write_text('fixture')
                    raise error_type('synthetic validation error')
                with patch.object(driver, 'run_validation', side_effect=failing):
                    with self.assertRaisesRegex(error_type, 'synthetic validation error'):
                        driver.main()
                self.assertEqual(len(seen), 1)
                self.assertFalse(seen[0].exists())
                self.assertEqual(marker.read_text(), 'unrelated synthetic owner')

    def test_cargo_real_failed_summary_is_not_a_compile_failure(self):
        content = ('thread fixture panicked: path must be shorter than SUN_LEN\n'
                   'test result: FAILED. 3101 passed; 1 failed; 13 ignored; 0 measured; 0 filtered out;\n'
                   'error: test failed, to rerun pass `--lib`\n')
        self.assertEqual(driver.classify_rust(101, content, []), 'test-failure')
        self.assertEqual(driver.classify_focused(101, content, ['fixture']), 'test-failure')
        with self.assertRaises(AssertionError):
            driver.classify_rust(0, content, ['fixture'])

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

    def test_permanent_regressions_are_in_the_full_product_inventory(self):
        self.assertEqual([len(case['tests']) for case in driver.MANIFEST['focused'].values()], [30, 31])
        regressions = driver.MANIFEST['regressions']
        self.assertEqual(len(regressions['tests']), 3)
        names = [name for case in driver.MANIFEST['focused'].values() for name in case['tests']]
        self.assertEqual(len(names), len(set(names)))
        self.assertTrue(set(regressions['tests']) < set(names))
        self.assertEqual(set(regressions['function_sha256']), set(regressions['tests']))
        self.assertEqual(regressions['red_run_id'], 40223996205)
        self.assertNotIn('probe', driver.MANIFEST)

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

    def test_new_workflow_pins_one_product_without_an_overlay_checkout(self):
        root = HERE.parents[1]
        workflow = (root / '.github/workflows/maintenance-validation.yml').read_text().split('  codex-usage-440-full-product:', 1)[1]
        self.assertIn(driver.REF, workflow)
        self.assertIn('ref: ' + driver.MANIFEST['product_sha'], workflow)
        self.assertIn('product evidence "$EXPECTED_VALIDATION_SHA" 2>&1', workflow)
        self.assertNotIn('path: probes', workflow)
        self.assertNotIn('ref: ' + driver.MANIFEST['regressions']['red_probe_sha'], workflow)
        self.assertNotIn('ed843373628fed574c20d61ffa259a01bd0c60ee', workflow)
        source = (HERE / 'verify-codex-usage-440-full.py').read_text()
        self.assertIn('focused(product_lib["binary"], MANIFEST["regressions"]', source)
        self.assertNotIn('rust-probe-compile', source)

    def test_product_source_rejects_wrong_sha_tree_or_dirty_checkout(self):
        base_answers = [driver.MANIFEST['product_sha'], driver.MANIFEST['product_tree'], '']
        for index in range(3):
            answers = list(base_answers)
            answers[index] = 'unexpected'
            with patch.object(driver.subprocess, 'check_output', side_effect=answers):
                with self.assertRaises(AssertionError):
                    driver.verify_source(Path('/synthetic'))

    def source_fixture(self):
        function = '    #[tokio::test]\n    async fn followup_regression() {\n        assert_eq!(1, 1);\n    }'
        source = 'mod tests {\n    #[test]\n    fn adjacent() {}\n}\nmod usage_credential_tests {\n' + function + '\n}\n'
        focused = {
            'usage-credentials': {'prefix': 'usage::', 'tests': ['usage::followup_regression']},
            'neighboring-accounts': {'prefix': 'tests::', 'tests': ['tests::adjacent']},
        }
        regressions = {'prefix': 'usage::followup_', 'tests': ['usage::followup_regression'],
                       'function_sha256': {'usage::followup_regression': hashlib.sha256(function.encode()).hexdigest()}}
        return source, {'focused': focused, 'regressions': regressions}

    def test_focused_source_requires_exact_inventory_and_unchanged_assertions(self):
        source, manifest = self.source_fixture()
        with patch.dict(driver.MANIFEST, manifest):
            driver.verify_focused_source(source)
            for changed in [source.replace('#[test]', ''),
                            source.replace('    fn adjacent() {}', '    fn adjacent() {}\n    #[test]\n    fn extra() {}'),
                            source.replace('assert_eq!(1, 1);', 'assert_eq!(1, 2);'),
                            source.replace('followup_regression()', 'renamed_regression()')]:
                with self.assertRaises(AssertionError):
                    driver.verify_focused_source(changed)

    def test_regressions_cannot_be_removed_from_the_selected_product_tests(self):
        source, manifest = self.source_fixture()
        manifest['regressions']['tests'] = []
        with patch.dict(driver.MANIFEST, manifest), self.assertRaises(AssertionError):
            driver.verify_focused_source(source)

    def test_required_stages_include_regressions_and_reject_missing_duplicate_or_failed(self):
        stages = driver.required_stage_names()
        self.assertEqual(len(stages), 41)
        self.assertIn('rust-product-regressions-inventory', stages)
        self.assertIn('rust-product-regressions', stages)
        self.assertIn('rust-default-tests-and-docs', stages)
        self.assertNotIn('rust-probe-compile', stages)
        results = [{'stage': name, 'passed': True} for name in stages]
        driver.verify_required_stages(results)
        for changed in [results[:-1], results + [results[0]],
                        [dict(result, passed=False) if result['stage'] == 'rust-product-regressions' else result for result in results]]:
            with self.assertRaises(AssertionError):
                driver.verify_required_stages(changed)

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
