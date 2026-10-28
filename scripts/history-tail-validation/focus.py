#!/usr/bin/env python3
"""Fresh sealed-overlay history-tail verification; never runs on import.

The original resource supervisor is byte-identical to the successful reviewed
40674882098 driver. Its old overlay/frontend/full-suite entry point is NOT called.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
import time

HERE = Path(__file__).resolve().parent
SHA = '60067a8a8581845c01dc5535712b924eb5d46129'
TREE = 'a465079b58647e7cbe0f0b864d724d8cab440536'
OVERLAY_TREE = 'd8836377a79bc898f13d8b5acb30850b26e11278'
OVERLAY_BLOB = 'd35074b9f6db894a596912bd0095797a5f6954d9'
PATCH_HASH = 'ac7888ac23f0749ac16f24aae1136df95ed9bdc912f5dad4f6ef907df237ab85'
PARENT = '7ebffaf24844eb01e91cc3745f6c5167b67c8339'
FORMAT_BASELINE = SHA
BRANCH = 'refs/heads/validation/history-tail-preservation-20261028'
TEST = 'tauri/src/coding/codex/history_sync.rs'
TEST_HASH = '7002b5bbdd63007abb30bcdaa4f326ac6af6467cc5dca740344c4304b5bf23d2'
HELPER_BLOB = '4dbf928a7206ba01ea9ad46173317c6c4c9ff5a8'
raw = (HERE / 'resource_control.py').read_bytes()
assert hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest() == HELPER_BLOB
spec = importlib.util.spec_from_file_location('reviewed_resource_control', HERE / 'resource_control.py')
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)
contract = json.loads((HERE / 'contract.json').read_text())


def own_exited():
    return h.ACTIVE is None and h.RESOURCE_SAMPLER is None and all(not x['survivors'] for x in h.RECORDS)


def identity(path):
    s = path.lstat()
    h.require(path.is_file() and not path.is_symlink(), 'not a regular artifact')
    return {'dev': s.st_dev, 'ino': s.st_ino, 'size': s.st_size, 'mode': s.st_mode, 'sha256': h.file_digest(path)}


def verify_seal():
    seal = json.loads((HERE / 'seal.json').read_text())
    for name, digest in seal.items():
        p = HERE.parent.parent / name if name.startswith('.github/') else HERE / name
        h.require(not p.is_symlink() and h.file_digest(p) == digest, 'control seal differs: ' + name)


def git(source, env, label, *args):
    return h.run(label, ['git', *args], 30, source, env)


def verify_base(source, env, label):
    h.require(git(source, env, label+'-head', 'rev-parse', 'HEAD').decode().strip() == SHA, 'wrong subject HEAD')
    h.require(git(source, env, label+'-tree', 'rev-parse', 'HEAD^{tree}').decode().strip() == TREE, 'wrong subject tree')
    h.require(git(source, env, label+'-parent', 'rev-parse', 'HEAD^').decode().strip() == PARENT, 'subject parent differs')
    h.require(not git(source, env, label+'-status', 'status', '--porcelain', '--untracked-files=all').strip(), 'subject not clean')
    h.require(h.file_digest(source / TEST) == 'e81cb84153ec83c1377908481814db65b1e6f46fef86167286606c6f127f7799', 'baseline bytes differ')

def apply_overlay(source, env):
    patch = HERE / 'candidate.patch'
    h.require(h.file_digest(patch) == PATCH_HASH, 'reviewed patch differs')
    git(source, env, 'overlay-check', 'apply', '--check', '--index', str(patch))
    git(source, env, 'overlay-apply', 'apply', '--index', str(patch))
    verify_overlay(source, env, 'applied')

def verify_overlay(source, env, label):
    h.require(git(source, env, label+'-head', 'rev-parse', 'HEAD').decode().strip() == SHA, 'base HEAD changed')
    h.require(git(source, env, label+'-overlay-tree', 'write-tree').decode().strip() == OVERLAY_TREE, 'wrong complete overlay tree')
    paths = git(source, env, label+'-changed-paths', 'diff', '--cached', '--name-only', SHA).decode().splitlines()
    h.require(paths == [TEST], 'unexpected staged change')
    git(source, env, label+'-worktree', 'diff', '--exit-code')
    h.require(not git(source, env, label+'-untracked', 'ls-files', '--others', '--exclude-standard').strip(), 'untracked source')
    h.require(h.file_digest(source / TEST) == TEST_HASH, 'candidate bytes differ')
    h.require(git(source, env, label+'-blob', 'rev-parse', ':'+TEST).decode().strip() == OVERLAY_BLOB, 'candidate blob differs')
    diff = git(source, env, label+'-diff', 'diff', '--cached', '--binary', SHA)
    (h.EVIDENCE / (label+'-overlay.diff')).write_bytes(diff)


def run_suite(label, binary, prefix, names, ignored, source, env, full_inventory, exact=False):
    before = identity(binary)
    filters = ([prefix] if prefix else []) + (['--exact'] if exact else [])
    listed = h.inventory(h.run(label+'-list', [str(binary), *filters, '--list'], 30, source, env))
    skipped = h.inventory(h.run(label+'-ignored-list', [str(binary), *filters, '--ignored', '--list'], 30, source, env))
    h.require(listed == sorted(names + ignored) and skipped == sorted(ignored), 'actual selected inventory mismatch')
    h.require(identity(binary) == before, 'binary changed while enumerating')
    out = h.run(label+'-tests', [str(binary), *filters, '--test-threads=1', '--show-output'], 600, source, env)
    blocks = h.libtest_blocks(out)
    h.require(len(blocks) == 1, 'ambiguous parent summaries')
    b = blocks[0]
    expected = sorted([(n, 'ok') for n in names] + [(n, 'ignored') for n in ignored])
    h.require(sorted(b['entries']) == expected, 'missing/duplicate/nonpassing actual parent result')
    h.require(b['status'] == 'ok' and b['passed'] == len(names) and b['ignored'] == len(ignored)
              and b['failed'] == b['measured'] == 0 and b['listed_run_count'] == len(expected)
              and b['filtered'] == len(full_inventory) - len(expected), 'actual summary differs')
    h.require(identity(binary) == before, 'binary changed during tests')
    return {'artifact': before, 'actual_full_inventory_count': len(full_inventory), 'selected': listed,
            'ignored_names': skipped, 'result': b, 'nested_children_are_not_additional_parent_passes': True}


def validate(source, env):
    result = {'ok': False, 'tests_passed': False, 'format_passed': False, 'whole_suite_executed': False,
              'base_sha': SHA, 'base_tree': TREE, 'overlay_tree': OVERLAY_TREE, 'overlay_blob': OVERLAY_BLOB,
              'patch_sha256': PATCH_HASH, 'scope': 'Synthetic Linux history metadata sync only; not clean baseline or transport validation'}
    tested = dist = dist_id = asset_id = None
    try:
        verify_base(source, env, 'fresh')
        apply_overlay(source, env)
        tested = h.snapshot_source(source, env, 'before')
        h.write_json('validation-source-before.json', tested)
        for tool in ('rustc', 'cargo'):
            h.require(h.run(tool+'-version', [tool, '--version'], 15, source, env).decode().startswith(tool+' 1.99.1 '), 'wrong toolchain')
        h.run('rustfmt-version', ['rustfmt', '--version'], 15, source, env)
        baseline = h.EVIDENCE / 'baseline-history-sync.rs'
        baseline.write_bytes(git(source, env, 'baseline-source', 'show', FORMAT_BASELINE+':'+TEST))
        h.require(h.file_digest(baseline) == 'e81cb84153ec83c1377908481814db65b1e6f46fef86167286606c6f127f7799', 'baseline format input differs')
        fmt = {}
        for label, path in [('baseline', baseline), ('candidate', source / TEST)]:
            h.run('rustfmt-'+label, ['rustfmt', '--edition', '2021', '--check', str(path)], 30, source, env, allowed_codes=(0,1))
            fmt[label] = {'exit': h.RECORDS[-1]['exit_code'], 'logs': h.RECORDS[-1]['logs']}
        result['format'] = fmt
        result['format_passed'] = fmt['candidate']['exit'] == 0
        result['format_note'] = 'Any existing/new formatting diff remains separate and unwaived; no autoformat performed'
        rust_env = h.create_runtime_fixture(source, env)
        dist = source / 'dist'
        h.require(not dist.exists() and not dist.is_symlink(), 'existing compile asset refused')
        dist.mkdir(mode=0o700)
        st = dist.lstat(); dist_id = (st.st_dev, st.st_ino)
        with (dist / 'index.html').open('xb'): pass
        asset_id = identity(dist / 'index.html')
        h.write_json('validation-compile-asset.json', {'sha256': h.digest(b''), 'purpose': 'empty compile-only asset, NOT frontend build/native acceptance'})
        h.ARCHIVES_ALLOWED = True
        h.STAGE = 'compile'
        out = h.run('compile', contract['compile']['argv'], 1680, source, rust_env)
        records = [json.loads(line) for line in out.decode().splitlines() if line.strip()]
        h.require([r for r in records if r.get('reason') == 'build-finished'] == [{'reason':'build-finished','success':True}], 'fresh Cargo completion absent')
        artifacts = [r for r in records if r.get('reason') == 'compiler-artifact' and r.get('executable') and r.get('profile',{}).get('test')]
        h.require(len(artifacts) == 1, 'wrong actual binary count')
        binaries = {}
        expected = {'ai_toolbox_lib':'tauri/src/lib.rs'}
        for item in artifacts:
            name = item['target']['name']
            h.require(name in expected and name not in binaries, 'wrong/duplicate Cargo target')
            h.require(Path(item['target']['src_path']).resolve() == (source / expected[name]).resolve(), 'wrong target source')
            path = Path(item['executable']).resolve(strict=True)
            h.require(path.is_relative_to(h.TARGET_ROOT) and os.access(path, os.X_OK), 'artifact escaped target')
            binaries[name] = path
        for name, binary in binaries.items():
            before = identity(binary)
            all_names = h.inventory(h.run(name.replace('_','-')+'-full-list', [str(binary), '--list'], 30, source, rust_env))
            h.require(identity(binary) == before, 'binary changed during full list')
            selection = contract['lib_suite']
            exact_name = contract['exact_test']
            result['exact_new_test'] = run_suite('exact-new-test', binary, exact_name, [exact_name], [], source, rust_env, all_names, exact=True)
            result['history_sync'] = run_suite('history-sync', binary, selection['filter'], selection['expected_normal'], [], source, rust_env, all_names)
        result['tests_passed'] = True
        result['ok'] = result['format_passed']
        result['outcome'] = 'focused_tests_and_format_passed' if result['ok'] else 'focused_tests_passed_format_differences_need_review'
    except BaseException as error:
        result['error'] = str(error)
        result['stage'] = h.STAGE
        result['outcome'] = 'incomplete_or_failure_requires_diagnosis_not_automatic_retry'
    finally:
        h.FINALIZING = True
        errors = []
        try:
            h.require(own_exited(), 'unknown process/sampler exit; retain fixture and asset')
            if dist_id is not None:
                st = dist.lstat()
                h.require(not dist.is_symlink() and (st.st_dev,st.st_ino)==dist_id, 'compile dir changed')
                if asset_id is not None:
                    h.require(identity(dist/'index.html') == asset_id, 'compile asset changed')
                    (dist/'index.html').unlink()
                dist.rmdir()
            if h.FIXTURE is not None: h.remove_owned_fixture()
            result['fixture_cleanup_confirmed'] = True
        except BaseException as error: errors.append('cleanup: '+str(error))
        try:
            h.require(h.RESOURCE_SAMPLER is None, 'unconfirmed sampler')
            if tested is not None:
                after = h.snapshot_source(source, env, 'after')
                h.write_json('validation-source-after.json', after)
                h.require(after == tested, 'tracked source/locks changed')
                verify_overlay(source, env, 'final')
            h.check_disk_budgets(force=True)
        except BaseException as error: errors.append('source/resource proof: '+str(error))
        if not own_exited(): errors.append('final process exit not confirmed')
        if errors: result['ok'] = False
        result.update(finalization_errors=errors, all_owned_processes_exited=own_exited(), commands=h.RECORDS,
                      memory=h.MEMORY_OBSERVATION, cache=h.CACHE_OBSERVATION)
        h.write_json('validation-result.json', result)
    return result


def main():
    parser = argparse.ArgumentParser(); parser.add_argument('--phase', choices=['preflight','validation'], required=True)
    phase = parser.parse_args().phase
    h.require(sys.platform == 'linux' and os.geteuid()!=0, 'unprivileged Linux required')
    h.require(os.environ['GITHUB_REPOSITORY']=='xue-ding-e/AI-Toolbox-Next' and os.environ['GITHUB_REF']==BRANCH
              and os.environ['GITHUB_EVENT_NAME']=='workflow_dispatch'
              and os.environ['EXPECTED_SHA']==os.environ['GITHUB_SHA'], 'unexpected control dispatch')
    verify_seal()
    workspace = Path(os.environ['GITHUB_WORKSPACE']).resolve()
    source = workspace / 'work/source'
    h.PHASE = phase
    h.EVIDENCE = Path(os.environ['RUNNER_TEMP']).resolve() / 'history-tail-evidence'
    h.require(not h.EVIDENCE.is_symlink(), 'evidence symlink')
    h.EVIDENCE.mkdir(mode=0o700, exist_ok=True)
    clock = h.EVIDENCE / 'preflight-budget.json'
    for sig in (signal.SIGTERM,signal.SIGINT): signal.signal(sig,h.interrupted)
    if phase == 'preflight':
        h.require(not clock.exists(), 'duplicate preflight')
        h.write_json(clock.name, {'deadline_monotonic': time.monotonic()+3900})
        h.JOB_DEADLINE = json.loads(clock.read_text())['deadline_monotonic']
        env = {'PATH':os.environ['PATH'],'LANG':'C.UTF-8','GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':'/dev/null'}
        h.require(git(HERE.parent.parent, env, 'control-head', 'rev-parse', 'HEAD').decode().strip() == os.environ['GITHUB_SHA'], 'control checkout SHA differs')
        verify_base(source, env, 'preflight')
        return
    h.require(not (h.EVIDENCE/'validation-result.json').exists(), 'duplicate validation refused')
    h.JOB_DEADLINE = json.loads(clock.read_text())['deadline_monotonic']
    h.RUN_ROOT = workspace/'work'; h.SOURCE_ROOT = source
    env = h.clean_env(h.RUN_ROOT/'environment')
    env.update(CARGO_REGISTRIES_CRATES_IO_PROTOCOL='sparse', CARGO_REGISTRIES_CRATES_IO_INDEX='sparse+https://index.crates.io/')
    h.CACHE_ROOT = Path(env['CARGO_HOME']); h.TARGET_ROOT = Path(env['CARGO_TARGET_DIR']).resolve()
    (h.CACHE_ROOT/'config.toml').write_text('[source.crates-io]\nregistry = "sparse+https://index.crates.io/"\n')
    result = validate(source, env)
    raise SystemExit(0 if result['ok'] else 1)


if __name__ == '__main__': main()
