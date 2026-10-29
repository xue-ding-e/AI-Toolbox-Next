import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

def assert_tracked_clean(repo):
    # Compare both index and worktree against HEAD: staged edits are also drift.
    changed = subprocess.check_output(
        ['git', '-C', str(repo), 'diff', 'HEAD', '--name-only'], text=True, timeout=15
    ).strip()
    assert not changed, 'Tracked source changed relative to HEAD'


def main():
    control, product, evidence = map(lambda s: Path(s).resolve(), sys.argv[1:4])
    m = json.loads((control / 'manifest.json').read_text())
    assert m['executionAuthorized'] is True, 'Draft is not authorized for execution'
    assert os.environ['GITHUB_ACTIONS'] == 'true'
    assert os.environ['RUNNER_ENVIRONMENT'] == 'github-hosted'
    assert os.environ['GITHUB_EVENT_NAME'] == 'workflow_dispatch'
    assert os.environ['GITHUB_REPOSITORY'] == m['repository']
    assert os.environ['GITHUB_REPOSITORY_ID'] == m['repositoryId']
    assert os.environ['GITHUB_REF'] == m['controlRef']
    assert os.environ['GITHUB_SHA'] == os.environ['EXPECTED_CONTROL_SHA']
    assert os.environ['GITHUB_WORKFLOW_SHA'] == os.environ['EXPECTED_CONTROL_SHA']

    def git(*args):
        return subprocess.check_output(['git', '-C', str(product), *args], text=True, timeout=15).strip()

    assert subprocess.check_output(['git', '-C', str(control / '../../..'), 'rev-parse', 'HEAD'], text=True, timeout=15).strip() == os.environ['EXPECTED_CONTROL_SHA']
    assert subprocess.check_output(['rustc', '--version'], text=True, timeout=15).startswith('rustc ' + m['rust'] + ' ')
    assert git('rev-parse', 'HEAD') == m['source']
    assert git('rev-parse', 'HEAD^{tree}') == m['tree']
    assert_tracked_clean(product)
    assert_tracked_clean((control / '../../..').resolve())
    for f in m['files']:
        assert hashlib.sha256((product / f['path']).read_bytes()).hexdigest() == f['candidateSha256']
    for path, expected in m['lockedFiles'].items():
        assert hashlib.sha256((product / path).read_bytes()).hexdigest() == expected
    assert json.loads((product / 'tauri/tauri.conf.json').read_text())['version'] == m['version']
    evidence.mkdir(parents=True, exist_ok=True)
    for f in m['formatFragments']:
        data = (product / f['path']).read_bytes()
        fragment = data[f['offset']:f['offset'] + f['length']]
        assert hashlib.sha256(fragment).hexdigest() == f['sha256']
        assert hashlib.sha256(data[:f['offset']]).hexdigest() == f['prefixSha256']
        assert hashlib.sha256(data[f['offset'] + f['length']:]).hexdigest() == f['suffixSha256']
        with tempfile.TemporaryDirectory(prefix='oauth-fmt-', dir=os.environ['RUNNER_TEMP']) as temp:
            p = Path(temp) / 'fragment.rs'; p.write_bytes(fragment)
            subprocess.run(['rustfmt', '--edition', '2021', '--check', str(p)], check=True, timeout=30)
    (evidence / 'source.json').write_text(json.dumps({'source':m['source'], 'tree':m['tree'],
        'control':os.environ['EXPECTED_CONTROL_SHA'], 'lockedFiles':m['lockedFiles'],
        'formatFragments':m['formatFragments'], 'scope':'Exact product source and changed-fragment format; no compilation or OAuth success implied'}, indent=2)+'\n')


if __name__ == '__main__':
    main()
