"""Bounded, read-only-service CI validation. This file is a draft, not a run."""

import hashlib
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import traceback
from pathlib import Path

BASE = 'f3a84498b5e192f1e78cb2a5312be012095de76c'
FIRST_HARNESS = '939db77845e62e4a1a628a243094e095d2178518'
PREVIOUS_HARNESS = '32d414b613dbb7cc96b7e6492516c96cf15a0f43'
REPOSITORY = 'xue-ding-e/AI-Toolbox-Next'
REF = 'refs/heads/validation/claude-desktop-official-edit'
VALIDATION_FILES = {
    '.github/workflows/maintenance-validation.yml',
    'validation/candidate.diff',
    'validation/manifest.json',
    'validation/validate.py',
}
HELPER = 'web/test/features/coding/claudedesktop/utils/officialRestore.test.ts'
FIXTURE = 'web/test/features/coding/claudedesktop/officialEdit/'
validation = Path(__file__).resolve().parents[1]
source = Path(os.environ['GITHUB_WORKSPACE']) / 'source'
runner_temp = Path(os.environ['RUNNER_TEMP']).resolve(strict=True)
evidence = runner_temp / 'official-edit-evidence'
evidence.mkdir(exist_ok=False)
raw = Path(tempfile.mkdtemp(prefix='official-edit-owned-', dir=runner_temp))
deadline = time.monotonic() + 1500
LOG_LIMIT = 32 * 1024 * 1024
FILE_LIMIT = 32 * 1024 * 1024
ARTIFACT_LIMIT = 256 * 1024 * 1024
manifest = json.loads((validation / 'validation/manifest.json').read_text())
harness_hashes = None
source_applied = False
owned_processes_clean = True


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(name, value):
    path = evidence / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def git(*args, cwd=source):
    return subprocess.check_output(['git', *args], cwd=cwd, timeout=20)


def regular(path, root, limit=FILE_LIMIT):
    assert path.is_file() and not path.is_symlink(), str(path)
    assert path.resolve(strict=True).is_relative_to(root.resolve(strict=True))
    assert path.stat().st_size <= limit, ('evidence-file-limit', str(path))
    return path


def load(path):
    return json.loads(regular(path, raw).read_text())


def export(path, name):
    regular(path, raw)
    destination = evidence / name
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(path.read_bytes())


def members(group):
    # /proc stat fields after the last ')' are state, ppid, and pgrp.
    # Zombies cannot execute, so do not misclassify pending reaping as a leak.
    result = []
    for item in Path('/proc').iterdir():
        if not item.name.isdigit():
            continue
        try:
            fields = (item / 'stat').read_text().rsplit(')', 1)[1].split()
            if int(fields[2]) == group and fields[0] != 'Z':
                result.append(int(item.name))
        except (FileNotFoundError, ProcessLookupError):
            pass
    return sorted(result)


def run(name, args, seconds, extra_env=None):
    global owned_processes_clean
    remaining_budget = deadline - time.monotonic() - 15
    assert remaining_budget > 0, 'Shared 25-minute validator budget exhausted'
    seconds = min(seconds, remaining_budget)
    log_path = evidence / (name + '.log')
    start = time.monotonic()
    with log_path.open('wb') as log:
        process = subprocess.Popen(
            args, cwd=source, stdout=log, stderr=subprocess.STDOUT,
            start_new_session=True, env={**os.environ, **(extra_env or {})},
        )
        reason = None
        survivors = None
        cleanup_error = None
        try:
            end = start + seconds
            while process.poll() is None:
                if time.monotonic() >= end:
                    reason = 'timeout'
                    break
                if log_path.stat().st_size > LOG_LIMIT:
                    reason = 'log-limit'
                    break
                time.sleep(.1)
            if log_path.stat().st_size > LOG_LIMIT:
                reason = reason or 'log-limit'
            if members(process.pid):
                reason = reason or 'surviving-descendants'
        finally:
            # Successful cleanup never upgrades a timeout/leak to a passing run.
            try:
                for sig in (signal.SIGTERM, signal.SIGKILL):
                    if process.poll() is not None and not members(process.pid):
                        break
                    try:
                        os.killpg(process.pid, sig)
                    except ProcessLookupError:
                        pass
                    until = time.monotonic() + 5
                    while (process.poll() is None or members(process.pid)) and time.monotonic() < until:
                        time.sleep(.1)
                survivors = members(process.pid)
            except Exception as error:
                cleanup_error = type(error).__name__ + ': ' + str(error)
            finally:
                code = process.poll()
                owned_processes_clean = owned_processes_clean and cleanup_error is None and code is not None and survivors == []
                size = log_path.stat().st_size
                save(name + '-process.json', {
                    'pid': process.pid, 'exitCode': code, 'reason': reason,
                    'survivors': survivors, 'cleanupError': cleanup_error,
                    'seconds': round(time.monotonic() - start, 3),
                    'effectiveTimeoutSeconds': seconds,
                    'logBytesBeforeTruncation': size, 'logSoftLimit': LOG_LIMIT,
                })
                # Keep bounded failure evidence even if a command bursts output.
                if size > LOG_LIMIT:
                    with log_path.open('r+b') as output:
                        output.truncate(LOG_LIMIT)
    assert cleanup_error is None and code is not None and survivors == [], 'Owned process exit unconfirmed; outer job timeout remains safeguard'
    assert reason is None, (name, reason)
    return code


def check_harness():
    assert git('rev-parse', 'HEAD', cwd=validation).decode().strip() == os.environ['GITHUB_SHA']
    assert git('show', '-s', '--format=%P', 'HEAD', cwd=validation).decode().strip() == PREVIOUS_HARNESS, 'Exactly one parent at the reviewed run-2 harness'
    assert git('show', '-s', '--format=%P', PREVIOUS_HARNESS, cwd=validation).decode().strip() == FIRST_HARNESS, 'Run-2 harness must have exactly the reviewed run-1 harness as parent'
    assert git('show', '-s', '--format=%P', FIRST_HARNESS, cwd=validation).decode().strip() == BASE, 'Run-1 harness must have exactly the fixed base as parent'
    assert git('rev-list', '--count', BASE + '..HEAD', cwd=validation).decode().strip() == '3', 'Only the three explicitly reviewed harness commits are allowed'
    assert set(git('diff', '--name-only', BASE, FIRST_HARNESS, cwd=validation).decode().splitlines()) == VALIDATION_FILES
    assert set(git('diff', '--name-only', FIRST_HARNESS, PREVIOUS_HARNESS, cwd=validation).decode().splitlines()) == VALIDATION_FILES
    assert set(git('diff', '--name-only', PREVIOUS_HARNESS, 'HEAD', cwd=validation).decode().splitlines()) == VALIDATION_FILES
    assert set(git('diff', '--name-only', BASE, 'HEAD', cwd=validation).decode().splitlines()) == VALIDATION_FILES
    assert git('status', '--porcelain', cwd=validation) == b''
    if harness_hashes is not None:
        assert {p: sha(validation / p) for p in VALIDATION_FILES} == harness_hashes


def check_source(label):
    assert git('rev-parse', 'HEAD').decode().strip() == BASE
    assert set(git('diff', 'HEAD', '--name-only').decode().splitlines()) == set(manifest['identities'])
    assert git('diff', '--name-only') == b'', 'Candidate worktree must equal the staged index'
    subprocess.run(['git', 'diff', '--cached', '--check'], cwd=source, check=True, timeout=20)
    files = {}
    for name, identity in manifest['identities'].items():
        path = source / name
        regular(path, source)
        files[name] = sha(path)
        assert files[name] == identity['sha256'] and path.stat().st_size == identity['bytes']
        assert git('ls-files', '-s', '--', name).decode().startswith('100644 ')
    locks = {p: sha(source / p) for p in manifest['lockfiles']}
    assert locks == manifest['lockfiles']
    untracked = git('ls-files', '--others', '--exclude-standard').decode().splitlines()
    save('untracked-' + label + '.json', untracked)
    assert untracked == []
    ignored = git('ls-files', '--others', '--ignored', '--exclude-standard').decode().splitlines()
    assert all(p.startswith('node_modules/') for p in ignored), 'Unexpected ignored output outside node_modules'
    check_harness()
    assert (evidence / 'tested.diff').read_bytes() == git('diff', '--cached', '--binary', 'HEAD')
    save('source-identities-' + label + '.json', {
        'base': BASE, 'files': files, 'locks': locks,
        'patchSha256': sha(validation / 'validation/candidate.diff'),
        'ignoredDependencyFiles': len(ignored),
    })


INVENTORY_SCRIPT = r'''
import fs from 'node:fs';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const require = createRequire(process.cwd() + '/package.json');
const ts = require('typescript');
const file = process.argv[1];
const root = ts.createSourceFile(file, fs.readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
assert.equal(root.parseDiagnostics.length, 0);
const names = [];
let calls = 0;
function visit(node) {
  if (ts.isCallExpression(node) && (node.expression.getText(root) === 'test' || node.expression.getText(root).startsWith('test.'))) {
    calls++;
    assert.equal(node.expression.getText(root), 'test');
    assert(ts.isExpressionStatement(node.parent) && node.parent.parent === root);
    assert.equal(node.arguments.length, 2);
    assert(ts.isStringLiteral(node.arguments[0]) && ts.isArrowFunction(node.arguments[1]));
    names.push(node.arguments[0].text);
  }
  ts.forEachChild(node, visit);
}
visit(root);
assert.equal(calls, 7);
assert.equal(new Set(names).size, 7);
console.log(JSON.stringify({file, names, count: names.length, method:'TypeScript AST; does not execute test callbacks'}));
'''


def helper_tests():
    assert run('helper-list', ['node', '--input-type=module', '-e', INVENTORY_SCRIPT, HELPER], 15) == 0
    inventory = json.loads((evidence / 'helper-list.log').read_text())
    assert inventory['names'] == manifest['helperTests'] and inventory['count'] == 7
    pattern = '^(?:' + '|'.join(re.escape(name) for name in manifest['helperTests']) + ')$'
    assert run('helper-tests', [
        'node', '--import', './scripts/register-node-ts-extension-loader.mjs',
        '--test', '--test-reporter=tap', '--test-concurrency=1',
        '--test-name-pattern=' + pattern, HELPER,
    ], 60) == 0
    lines = (evidence / 'helper-tests.log').read_text().splitlines()
    actual = [match.group(1) for line in lines if (match := re.fullmatch(r'ok \d+ - (.+)', line))]
    assert actual == manifest['helperTests'], ('Exact helper selection mismatch', actual)
    assert not any(line.startswith('not ok') or '# SKIP' in line or '# TODO' in line for line in lines)
    for exact in ['1..7', '# tests 7', '# pass 7', '# fail 0', '# cancelled 0', '# skipped 0', '# todo 0']:
        assert exact in lines, ('Missing TAP summary', exact)
    save('helper-summary.json', {'names': actual, 'listed': 7, 'passed': 7, 'failed': 0, 'skipped': 0})


def browser_root(required=True):
    paths = list(raw.glob('official-edit-review-*'))
    if not paths and not required:
        return None
    assert len(paths) == 1 and paths[0].is_dir() and not paths[0].is_symlink(), paths
    return paths[0]


def validate_browser():
    root = browser_root()
    preflight = load(root / 'preflight.json')
    assert preflight['status'] == 'dependencies-present' and preflight['missing'] == []
    assert preflight['validationChainError'] is None
    assert [(x['name'], x['expected'], x['actual']) for x in preflight['validationChain']] == [
        ('antd', '6.4.3', '6.4.3'), ('@rc-component/form', '1.8.6', '1.8.6'),
        ('@rc-component/async-validator', '6.0.0', '6.0.0'),
    ]
    assert preflight['counts'] == {'red': 10, 'green': 18}
    assert preflight['variants'] == ['red', 'green'] and preflight['baseline'] == BASE
    assert preflight['prepareOnly'] is False and preflight['overlayRoot'] is None and preflight['noNetwork'] is True
    assert Path(preflight['sourceRoot']).resolve() == source.resolve()
    assert Path(preflight['dependenciesRoot']).resolve() == (source / 'node_modules').resolve()
    prepared = load(root / 'prepared-manifest.json')
    assert prepared['variants'] == ['red', 'green'] and prepared['counts'] == {'red': 10, 'green': 18}
    assert prepared['limits'] == {'scenarioMs': 30000, 'verifyPerVariantMs': 480000, 'bundles': 2, 'browsers': 2}
    assert {FIXTURE + x['file']: x['sha256'] for x in prepared['harnessFiles']} == {
        p: v['sha256'] for p, v in manifest['identities'].items() if p.startswith(FIXTURE)
    }
    assert [s['variant'] for s in prepared['stages']] == ['red', 'green']
    result = load(root / 'results.json')
    assert result['status'] == 'strict-red-green-pass' and result['variants'] == ['red', 'green']
    assert result['counts'] == {'red': 10, 'green': 18} and len(result['results']) == 2
    total = 0
    for stage, outcome in zip(prepared['stages'], result['results']):
        variant = stage['variant']
        stage_root = Path(stage['root'])
        assert stage_root.resolve() == (root / variant).resolve()
        identity = stage['identity']
        assert identity['variant'] == variant and identity['baselineCommit'] == (BASE if variant == 'red' else None)
        assert {x['file']: x['sha256'] for x in identity['modules']} == manifest['sourceModules'][variant]
        expected_files = {p: v['sha256'] if variant == 'green' else v['baseSha256'] for p, v in manifest['identities'].items() if variant == 'green' or v['baseBlob'] is not None}
        assert {x['file']: x['sha256'] for x in stage['files']} == expected_files
        assert all(sha(regular(stage_root / p, raw)) == digest for p, digest in expected_files.items())
        assert sha(regular(Path(stage['fixtureDirectory']) / 'page-boundary.generated.mjs', raw)) == stage['generatedBoundarySha256']
        assert outcome['variant'] == variant and outcome['sourceIdentity'] == identity
        folder = Path(outcome['artifactRoot'])
        assert folder.parent.resolve() == root.resolve() and folder.name.startswith('official-edit-' + variant + '-')
        strict = load(folder / 'strict-results.json')
        common = load(folder / 'results.json')
        assert strict['sourceIdentity'] == identity and strict['variant'] == variant
        assert strict['diagnosticErrors'] == [], 'Missing screenshot evidence is not product RED'
        shots = strict['screenshots']
        assert [(x['case'], x['moment'], x['file']) for x in shots] == [
            (x['case'], x['moment'], x['file']) for x in manifest['successScreenshots'][variant]
        ]
        for shot in shots:
            image = regular(folder / shot['file'], folder, 8 * 1024 * 1024)
            data = image.read_bytes()
            assert data.startswith(b'\x89PNG\r\n\x1a\n') and len(data) > 24
            assert data[12:16] == b'IHDR'
            assert int.from_bytes(data[16:20], 'big') > 0 and int.from_bytes(data[20:24], 'big') > 0
            assert image.stat().st_size == shot['bytes'] and sha(image) == shot['sha256']
        rows = strict['results']
        cases = manifest['cases'][variant]
        assert [row['case'] for row in rows] == [case['id'] for case in cases]
        assert strict['count'] == outcome['count'] == len(cases)
        expected_checks = []
        for row, case in zip(rows, cases):
            assert row['verdict'] == case['verdict']
            for name in ['before', 'after']:
                snap = row[name]
                assert snap['case'] == row['case'] and snap['sourceIdentity'] == identity
                assert snap['denied'] == [] and snap['runtimeErrors'] == [] and snap['page']['errors'] == []
            expected_checks.append({'name': row['case'], 'passed': True, 'verdict': row['verdict'], 'screenshots': [s for s in shots if s['case'] == row['case']]})
            save('cases/' + variant + '-' + row['case'] + '.json', row)
        assert outcome['checks'] == common['checks'] == expected_checks
        assert common['browserExceptions'] == []
        bundles = outcome['bundles']
        assert bundles and len({x['file'] for x in bundles}) == len(bundles)
        for bundle in bundles:
            assert bundle['file'].startswith('dist/')
            assert sha(regular(folder / bundle['file'], folder)) == bundle['sha256']
        total += len(rows)
    red, green = [x['identity'] for x in prepared['stages']]
    for label in manifest['distinctSpans']:
        a = [x['sha256'] for x in red['spans'] if x['label'] == label]
        b = [x['sha256'] for x in green['spans'] if x['label'] == label]
        assert len(a) == len(b) == 1 and a != b, ('Wrong/stale extraction', label)
    assert total == 28


def retain_browser_evidence():
    root = browser_root(required=False)
    if root is None:
        return
    for name in ['preflight.json', 'prepared-manifest.json', 'results.json']:
        if (root / name).exists():
            export(root / name, 'browser/' + name)
    images = []
    for variant in ['red', 'green']:
        folders = list(root.glob('official-edit-' + variant + '-*'))
        assert len(folders) <= 1
        if not folders:
            continue
        folder = folders[0]
        assert folder.is_dir() and not folder.is_symlink()
        for name in ['strict-results.json', 'strict-failure.json', 'results.json', 'failure.html', 'failure.png']:
            path = folder / name
            if path.exists():
                export(path, 'browser/' + variant + '/' + name)
                if name.endswith('.png'):
                    assert path.read_bytes().startswith(b'\x89PNG\r\n\x1a\n')
                    images.append({'variant': variant, 'file': 'browser/' + variant + '/' + name, 'kind': 'failure-diagnostic'})
        for expected in manifest['successScreenshots'][variant]:
            path = folder / expected['file']
            if path.exists():
                regular(path, folder, 8 * 1024 * 1024)
                assert path.read_bytes().startswith(b'\x89PNG\r\n\x1a\n')
                name = 'browser/' + variant + '/' + expected['file']
                export(path, name)
                images.append({**expected, 'variant': variant, 'file': name, 'sha256': sha(path), 'kind': 'reviewed-fixture-capture'})
    save('screenshots.json', {
        'files': images, 'successScreenshotContract': manifest['successScreenshots'],
        'manualVisualReview': 'pending',
    })


def main():
    global harness_hashes, source_applied
    assert not sys.flags.optimize, 'Assertions must remain enabled'
    assert manifest['base'] == BASE and manifest['repository'] == REPOSITORY and manifest['ref'] == REF
    assert manifest['harnessHistory'] == {'base': BASE, 'firstCommit': FIRST_HARNESS, 'previousCommit': PREVIOUS_HARNESS, 'commitCountAboveBase': 3}
    assert set(manifest['validationFiles']) == VALIDATION_FILES and len(manifest['validationFiles']) == 4
    assert len(manifest['identities']) == 13
    assert os.environ['GITHUB_REPOSITORY'] == REPOSITORY
    assert os.environ['GITHUB_EVENT_NAME'] == 'workflow_dispatch' and os.environ['GITHUB_REF'] == REF
    assert os.environ.get('HUSKY') == '0'
    assert not any(os.environ.get(name) for name in ['AI_OFFICIAL_EDIT_OVERLAY_ROOT', 'AI_OFFICIAL_EDIT_SOURCE_ROOT', 'AI_OFFICIAL_EDIT_DEPENDENCIES_ROOT'])
    assert git('rev-parse', 'HEAD').decode().strip() == BASE and git('status', '--porcelain') == b''
    check_harness()
    patch = validation / 'validation/candidate.diff'
    assert sha(patch) == manifest['patchSha256'] and sha(Path(__file__)) == manifest['validatorSha256']
    harness_hashes = {p: sha(validation / p) for p in VALIDATION_FILES}
    save('harness-identities.json', {
        'base': BASE, 'validationCommit': os.environ['GITHUB_SHA'], 'files': harness_hashes,
        'harnessHistory': manifest['harnessHistory'],
        'hashChain': 'Workflow pins patch, manifest and validator; manifest pins patch, validator, all 13 candidate files and both locks. Workflow is recorded, not self-pinned.',
    })
    assert {p: sha(source / p) for p in manifest['lockfiles']} == manifest['lockfiles']
    for name, identity in manifest['identities'].items():
        if identity['baseBlob'] is None:
            assert subprocess.run(['git', 'cat-file', '-e', BASE + ':' + name], cwd=source, stderr=subprocess.DEVNULL, timeout=20).returncode != 0
        else:
            assert git('rev-parse', BASE + ':' + name).decode().strip() == identity['baseBlob']
            assert sha(source / name) == identity['baseSha256']
    subprocess.run(['git', 'apply', '--check', '--index', str(patch)], cwd=source, check=True, timeout=20)
    subprocess.run(['git', 'apply', '--index', str(patch)], cwd=source, check=True, timeout=20)
    source_applied = True
    (evidence / 'reviewed-candidate.diff').write_bytes(patch.read_bytes())
    (evidence / 'tested.diff').write_bytes(git('diff', '--cached', '--binary', 'HEAD'))
    # The reviewed input uses unified-diff syntax; Git emits additional blob and
    # mode headers. File identity, exact changed paths and index/worktree equality
    # prove semantic equality. Save both, without claiming byte-equal formatting.
    check_source('before')
    assert run('toolchain', ['bash', '-euo', 'pipefail', '-c', 'test "$(node --version)" = v22.23.3; test "$(pnpm --version)" = 9.15.0; node --version; pnpm --version; google-chrome --version; dpkg-query -W fonts-noto-cjk; fc-match -f "%{family}\\n" ":lang=zh-cn"'], 20) == 0
    assert 'Noto' in (evidence / 'toolchain.log').read_text()
    assert run('install', ['pnpm', 'install', '--frozen-lockfile'], 180) == 0
    check_source('after-install')
    helper_tests()
    assert run('tsc', ['pnpm', 'exec', 'tsc', '--noEmit'], 240) == 0
    check_source('before-browser')
    assert run('browser', [
        'pnpm', 'run', 'test:claude-desktop-official-edit', '--red-green',
        '--browser', '/usr/bin/google-chrome', '--output', str(raw),
    ], 1080) == 0
    validate_browser()
    check_source('after')
    save('result.json', {
        'status': 'passed', 'baselineScenarios': 10, 'baselineExpectedDataLossRed': 7,
        'baselineControlsPassed': 3, 'greenScenariosPassed': 18, 'helperExactPassed': 7,
        'fullTsc': True, 'candidateAndLocksUnchanged': True,
        'scope': 'Real React Form and ImportModal; fresh AST-extracted Page callbacks/builders; not full Page, Rust, DB, app runtime, real SSH or real credentials.',
        'screenshotEvidenceVerified': 4, 'manualVisualReview': 'pending',
    })


try:
    main()
except BaseException as error:
    save('result.json', {'status': 'failed', 'error': type(error).__name__ + ': ' + str(error), 'traceback': traceback.format_exc()})
    raise
finally:
    failures = []
    try:
        retain_browser_evidence()
    except Exception as error:
        failures.append('evidence: ' + repr(error))
    try:
        if source_applied:
            check_source('finally')
    except Exception as error:
        failures.append('identity: ' + repr(error))
    # Remove only this invocation's exclusively-created raw directory, and only
    # after every owned process group is confirmed stopped. Never glob /tmp.
    if owned_processes_clean:
        assert raw.parent == runner_temp and raw.name.startswith('official-edit-owned-') and not raw.is_symlink()
        shutil.rmtree(raw)
    else:
        failures.append('Owned process exit unconfirmed; raw scratch left for runner teardown')
    total_bytes = sum(p.stat().st_size for p in evidence.rglob('*') if p.is_file())
    if total_bytes > ARTIFACT_LIMIT:
        failures.append('Artifact total exceeds 256 MiB')
    save('cleanup.json', {'ownedProcessesClean': owned_processes_clean, 'rawRemoved': not raw.exists(), 'artifactBytes': total_bytes, 'errors': failures})
    if failures:
        save('result.json', {'status': 'failed', 'finalizationErrors': failures})
        raise RuntimeError('; '.join(failures))
