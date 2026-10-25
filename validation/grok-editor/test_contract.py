"""Pure report/seal contract tests; no proc, browser, network or product calls."""
import ast
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
os.environ.setdefault('GITHUB_WORKSPACE', str(HERE))
spec = importlib.util.spec_from_file_location('contract_driver', HERE / 'validate.py')
d = importlib.util.module_from_spec(spec)
spec.loader.exec_module(d)


def editor():
    return {'passed': 6, 'failed': 0, 'pageExceptions': [], 'results': [
        {'scenario': name, 'passed': True, 'observations': [
            {'label': 'final', 'state': {'routing': {'unexpected': []}}}]} for name in d.EDITOR_CASES]}


def catalog():
    return {'passed': 4, 'failed': 0, 'fixtureErrors': 0, 'globalPageExceptions': [],
            'validRed': False, 'results': [{'name': name, 'passed': True} for name in d.CATALOG_CASES]}


class ReportContractTests(unittest.TestCase):
    def test_exact_six_and_four_successes(self):
        d.verify_editor_report(editor())
        d.verify_catalog_report(catalog())

    def test_zero_missing_duplicate_and_unexpected_identities_rejected(self):
        for factory, verify, key in [(editor, d.verify_editor_report, 'scenario'),
                                     (catalog, d.verify_catalog_report, 'name')]:
            for kind in ['zero', 'missing', 'duplicate', 'unknown']:
                with self.subTest(kind=kind, key=key):
                    report = factory()
                    if kind == 'zero': report['results'] = []
                    elif kind == 'missing': report['results'].pop()
                    elif kind == 'duplicate': report['results'][-1] = copy.deepcopy(report['results'][0])
                    else: report['results'][-1][key] = 'unexpected'
                    with self.assertRaises(RuntimeError): verify(report)

    def test_counts_and_per_case_verdicts_must_agree(self):
        for factory, verify in [(editor, d.verify_editor_report), (catalog, d.verify_catalog_report)]:
            for key, value in [('passed', 0), ('failed', 1)]:
                report = factory(); report[key] = value
                with self.assertRaises(RuntimeError): verify(report)
            report = factory(); report['results'][0]['passed'] = False
            with self.assertRaises(RuntimeError): verify(report)
            report = factory(); report['results'][0]['passed'] = 1
            with self.assertRaises(RuntimeError): verify(report)

    def test_exceptions_and_valid_red_do_not_count_as_green(self):
        report = editor(); report['pageExceptions'] = ['synthetic exception']
        with self.assertRaises(RuntimeError): d.verify_editor_report(report)
        for key, value in [('globalPageExceptions', ['synthetic']), ('fixtureErrors', 1), ('validRed', True)]:
            report = catalog(); report[key] = value
            with self.assertRaises(RuntimeError): d.verify_catalog_report(report)

    def test_missing_duplicate_or_nonzero_router_observation_rejected(self):
        for observations in [[], [{'label': 'final', 'state': {'routing': {'unexpected': ['navigate']}}}],
                             editor()['results'][0]['observations'] * 2]:
            report = editor(); report['results'][0]['observations'] = observations
            with self.assertRaises(RuntimeError): d.verify_editor_report(report)

    def test_both_reports_required_and_duplicates_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            first = root / 'grok-editor-synthetic'; first.mkdir()
            second = root / 'grok-fetched-models-synthetic'; second.mkdir()
            with patch.object(d, 'read_regular', side_effect=lambda path: (path.read_bytes(), None)):
                with self.assertRaises(RuntimeError): d.verify_fixture_reports(root)
                (first / 'grok-editor-cases.json').write_text(json.dumps(editor()))
                with self.assertRaises(RuntimeError): d.verify_fixture_reports(root)
                (second / 'grok-fetched-cases.json').write_text(json.dumps(catalog()))
                d.verify_fixture_reports(root)
                extra = root / 'grok-editor-duplicate'; extra.mkdir()
                (extra / 'grok-editor-cases.json').write_text(json.dumps(editor()))
                with self.assertRaises(RuntimeError): d.verify_fixture_reports(root)

    def test_source_manifest_hash_and_exact_product_identity(self):
        raw = (HERE / 'source.json').read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), d.SOURCE_SEAL)
        seal = json.loads(raw)
        self.assertEqual(seal['base'], d.BASE)
        self.assertEqual(seal['base'], '059545a2d3e1145f7758e374e7d09e176f43caa3')
        self.assertEqual(seal['tree'], 'ea8fbc8849aa489ef2f853296ad52bd05f3c55ca')
        self.assertEqual(seal['files']['package.json']['sha256'], d.PACKAGE_SHA)
        self.assertEqual(seal['files']['pnpm-lock.yaml']['sha256'], d.LOCK_SHA)
        for path in ['scripts/verify-grok-editor-and-catalog.mjs', 'scripts/verify-grok-editor-readback.mjs',
                     'scripts/verify-grok-fetched-models.mjs', 'web/features/coding/grok/pages/GrokPage.tsx']:
            self.assertIn(path, seal['files'])

    def test_reviewed_threshold_cleanup_and_seal_function_bodies_unchanged(self):
        expected = json.loads((HERE / 'audited-functions.json').read_text())['functions']
        observed = {}
        for node in ast.parse((HERE / 'validate.py').read_text()).body:
            if isinstance(node, ast.FunctionDef) and node.name in expected:
                observed.setdefault(node.name, []).append(hashlib.sha256(ast.dump(node, include_attributes=False).encode()).hexdigest())
        self.assertEqual(observed, expected)
        self.assertEqual(d.RSS_LIMIT, 9 * 1024**3 // 2)
        self.assertEqual(d.FINALIZE_RESERVE, 120)
        self.assertEqual(d.LOG_LIMIT, 16 * 1024**2)
        self.assertEqual(d.TOTAL_LOG_LIMIT, 16 * 1024**2)

    def test_observation_error_records_safe_classification_without_retry(self):
        error = d.ObservationError('synthetic-control-open', 13, 'metadata read failed')
        with patch.object(d, 'observe_visible_v2', side_effect=error) as read, patch.object(d, 'write_json') as write:
            with self.assertRaisesRegex(RuntimeError, 'unavailable'): d.observe_memory()
        self.assertEqual(read.call_count, 1)
        self.assertEqual(write.call_count, 1)
        self.assertEqual(d.MEMORY_DIAGNOSTICS['capacity_error']['errno'], 13)
        self.assertEqual(d.MEMORY_DIAGNOSTICS['capacity_error']['read_class'], 'synthetic-control-open')


if __name__ == '__main__':
    unittest.main()
