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

# Parse both sealed sources with the same interpreter. ast.dump's representation
# changes across Python releases (3.12 adds type_params; 3.13 omits empty fields),
# so a digest produced by another interpreter is not a portable semantic seal.
# The historical text is never imported or executed, and cannot be substituted
# by a caller-supplied baseline/hash. The workflow separately seals these files.
AUDITED_BASELINE_SHA = 'c4230e5646ccc6678a706e60e0f689dab663f7683c31e1eb176702f71f0994e8'
AUDITED_BASELINE_FILE = 'audited-baseline-f617d51.py.txt'
AUDITED_FUNCTION_COUNTS = {
    'deadline': 1, 'pid_start': 1, 'admit_memory_budget': 1, 'check_rss': 1,
    'sample_dependency_budget': 1, 'check_storage_health': 1, 'check_disk_budgets': 1,
    'check_output_budget': 1, 'process_table': 1, 'owned_members': 1, 'observe_exit': 1,
    'cleanup': 2, 'run': 1, 'read_regular': 1, 'tree_snapshot': 1, 'content_records': 1,
    'verify_source': 1, 'arm_final_budget': 1, 'interrupt': 1,
}


def assert_reviewed_functions_unchanged(candidate, baseline, manifest):
    d.require(manifest == {
        'source_commit': 'f617d51b69a5db5dd522640f8952b8177cecfbc4',
        'source_file': AUDITED_BASELINE_FILE,
        'source_sha256': AUDITED_BASELINE_SHA,
        'functions': AUDITED_FUNCTION_COUNTS,
    }, 'audited baseline manifest changed')
    d.require(hashlib.sha256(baseline).hexdigest() == AUDITED_BASELINE_SHA,
              'audited historical source hash changed')

    def functions(source):
        observed = {}
        for node in ast.parse(source).body:
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name in AUDITED_FUNCTION_COUNTS:
                observed.setdefault(node.name, []).append(ast.dump(node, include_attributes=False))
        d.require({name: len(bodies) for name, bodies in observed.items()} == AUDITED_FUNCTION_COUNTS,
                  'audited function missing, duplicated, or renamed')
        return observed

    d.require(functions(candidate) == functions(baseline.decode('utf-8')),
              'audited function AST changed')


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
        assert_reviewed_functions_unchanged(
            (HERE / 'validate.py').read_text(), (HERE / AUDITED_BASELINE_FILE).read_bytes(),
            json.loads((HERE / 'audited-functions.json').read_text()))
        self.assertEqual(d.RSS_LIMIT, 9 * 1024**3 // 2)
        self.assertEqual(d.FINALIZE_RESERVE, 120)
        self.assertEqual(d.LOG_LIMIT, 16 * 1024**2)
        self.assertEqual(d.TOTAL_LOG_LIMIT, 16 * 1024**2)

    def audit(self, source=None, baseline=None, manifest=None):
        assert_reviewed_functions_unchanged(
            (HERE / 'validate.py').read_text() if source is None else source,
            (HERE / AUDITED_BASELINE_FILE).read_bytes() if baseline is None else baseline,
            json.loads((HERE / 'audited-functions.json').read_text()) if manifest is None else manifest)

    def test_changed_baseline_rejected_even_when_manifest_hash_is_changed_to_match(self):
        baseline = (HERE / AUDITED_BASELINE_FILE).read_bytes() + b'\n# unreviewed substitution\n'
        with self.assertRaisesRegex(RuntimeError, 'historical source hash'): self.audit(baseline=baseline)
        manifest = json.loads((HERE / 'audited-functions.json').read_text())
        manifest['source_sha256'] = hashlib.sha256(baseline).hexdigest()
        with self.assertRaisesRegex(RuntimeError, 'baseline manifest'): self.audit(baseline=baseline, manifest=manifest)

    def test_unknown_baseline_and_reduced_function_inventory_rejected(self):
        for field, value in [('source_commit', '0' * 40), ('source_file', 'another-baseline.py'),
                             ('functions', {**AUDITED_FUNCTION_COUNTS, 'cleanup': 1})]:
            manifest = json.loads((HERE / 'audited-functions.json').read_text()); manifest[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(RuntimeError, 'baseline manifest'):
                self.audit(manifest=manifest)

    def test_semantic_budget_identity_cleanup_and_seal_mutations_rejected(self):
        source = (HERE / 'validate.py').read_text()
        for before, after in [('>=6*1024**3', '>=1*1024**3'), ('rss<=RSS_LIMIT', 'rss<=RSS_LIMIT*2'),
                              ('os.WNOWAIT', '0'), ('signal.SIGKILL', 'signal.SIGTERM'),
                              ("digest(data)==record['sha256']", 'True')]:
            with self.subTest(mutation=before):
                self.assertIn(before, source)
                with self.assertRaisesRegex(RuntimeError, 'AST changed'):
                    self.audit(source=source.replace(before, after))

    def test_missing_renamed_duplicate_and_async_functions_rejected(self):
        source = (HERE / 'validate.py').read_text()
        mutations = [source.replace('def deadline():', 'def unreviewed_deadline():'),
                     source.replace('def cleanup(', 'def unreviewed_cleanup(', 1),
                     source + '\ndef deadline():\n    pass\n',
                     source.replace('def deadline():', 'async def deadline():')]
        for changed in mutations:
            with self.subTest(source_tail=changed[-60:]), self.assertRaises(RuntimeError): self.audit(source=changed)

    def test_comment_only_candidate_change_preserves_exact_ast_comparison(self):
        # No semantic fields are normalized or dropped to make versions agree.
        self.audit(source=(HERE / 'validate.py').read_text() + '\n# harmless review note\n')

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
