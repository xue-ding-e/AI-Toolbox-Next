import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
out = Path(os.environ['RUNNER_TEMP']) / 'magpie-evidence'
cases = json.loads((root / 'corpus.json').read_text())
expected = {c['name']: c for c in cases}
assert len(expected) == 40

def results(path):
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    assert len(rows) == len(expected), (path.name, len(rows))
    by_name = {r['name']: r for r in rows}
    assert set(by_name) == set(expected), path.name
    return by_name

rust = results(out / 'oracle.log')
go_green = results(out / 'go-corpus-green.jsonl')
go_race = results(out / 'go-corpus-race.jsonl')
report = []
for name, c in expected.items():
    for language, row, key in [('rust', rust[name], 'rust_accept'),
                               ('go', go_green[name], 'go_accept'),
                               ('go-race', go_race[name], 'go_accept')]:
        assert row['accepted'] == c[key], (name, language, 'classification')
        assert row['summary_hex'] == (c['summary_hex'] if c[key] else ''), (name, language, 'summary')
    report.append({'name': name, 'go_accept': c['go_accept'], 'rust_accept': c['rust_accept'],
                   'classification': 'intentional local restriction' if c['go_accept'] != c['rust_accept'] else 'agreement',
                   'policy': c['policy'], 'rust_trimmed_hex': rust[name]['trimmed_hex']})

top_tests = {'TestKnownSyntheticVector', 'TestPayloadContract', 'TestEnvelopeBoundaries',
             'TestNoncanonicalTrailingBits', 'TestSyntheticCorpusInterop'}
for phase in ('green', 'race'):
    events = [json.loads(line) for line in (out / (phase + '.log')).read_text().splitlines() if line.strip()]
    assert not any(e.get('Action') in ('fail', 'skip') for e in events), phase
    passed = [e['Test'] for e in events if e.get('Action') == 'pass' and e.get('Test')]
    assert {name for name in passed if '/' not in name} == top_tests, (phase, passed)
    assert sum(name.startswith('TestPayloadContract/') for name in passed) == 46, phase
    assert sum(name.startswith('TestEnvelopeBoundaries/') for name in passed) == 8, phase
    assert sum(name.startswith('TestSyntheticCorpusInterop/') for name in passed) == 40, phase
    assert any(e.get('Action') == 'pass' and not e.get('Test') for e in events), phase

for phase in ('prepare', 'rust-toolchain', 'resolve', 'fetch', 'green', 'race', 'oracle'):
    status = json.loads((out / (phase + '.json')).read_text())
    assert status['exit_code'] == 0 and not status['log_limit_hit'], phase
    assert not status['cleanup']['non_z_survivors'] and not status['cleanup']['errors'], phase
assert sum(r['classification'] != 'agreement' for r in report) == 4
(out / 'comparison.json').write_text(json.dumps({'model_oracle_not_product_consumer': True,
    'cases': report, 'go_frozen_top_level_tests': 4, 'harness_top_level_tests': 1,
    'all_classifications_match_expectations': True}, indent=2) + '\n')
print('40 shared fixtures classified as expected; 36 agreements, 4 documented local restrictions. Go original tests and race passed.')
