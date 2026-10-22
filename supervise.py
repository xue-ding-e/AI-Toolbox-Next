#!/usr/bin/env python3
"""One owner, one active phase process group; no nested process supervisors."""
import json
import os
from pathlib import Path
import selectors
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent
OUT = Path(os.environ['RUNNER_TEMP']) / 'magpie-evidence'
OUT.mkdir(exist_ok=True)
WORK = tempfile.mkdtemp(prefix='capsule-oracle.', dir=os.environ['RUNNER_TEMP'])
os.environ['CAPSULE_WORK'] = WORK
(OUT / 'work-directory.txt').write_text(WORK + '\n')
LIMIT = 16 * 1024 * 1024
active = None
interrupted = None


def members(pgid):
    # Read process metadata only, never cmdline/environment or other content.
    result = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            rest = (entry / 'stat').read_text().rsplit(')', 1)[1].split()
            if int(rest[2]) == pgid and rest[0] != 'Z':
                result.append(int(entry.name))
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            pass
    return sorted(result)


def clean(proc):
    errors = []
    def send(sig):
        try:
            os.killpg(proc.pid, sig)
        except ProcessLookupError:
            pass
        except OSError as exc:
            errors.append(type(exc).__name__)
    send(signal.SIGTERM)
    until = time.monotonic() + 2
    while time.monotonic() < until:
        proc.poll()  # reap the direct child; leader exit is not group completion
        if not members(proc.pid):
            break
        time.sleep(0.05)
    send(signal.SIGKILL)
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        errors.append('direct_child_not_reaped')
    until = time.monotonic() + 2
    survivors = members(proc.pid)
    while survivors and time.monotonic() < until:
        time.sleep(0.05)
        survivors = members(proc.pid)
    return {'owned_pgid': proc.pid, 'non_z_survivors': survivors, 'errors': errors}


def on_signal(number, _frame):
    global interrupted
    interrupted = number
    # Main loop remains the sole cleanup owner, including repeated signals.


signal.signal(signal.SIGTERM, on_signal)
signal.signal(signal.SIGINT, on_signal)
phases = [('prepare', 60), ('rust-toolchain', 180), ('resolve', 120),
          ('fetch', 180), ('green', 240), ('race', 240), ('oracle', 240), ('verify', 60)]
smoke = len(sys.argv) == 2 and sys.argv[1] in ('--smoke-timeout', '--smoke-signal', '--smoke-leader-exit')
if len(sys.argv) > 1 and not smoke:
    raise SystemExit('Only fixed local cleanup smoke modes are supported')
if smoke:
    phases = [('smoke', 30 if sys.argv[1] == '--smoke-signal' else 2)]
    if sys.argv[1] == '--smoke-leader-exit':
        os.environ['SMOKE_LEADER_EXIT'] = '1'
overall = time.monotonic() + (40 if smoke else 23 * 60)
suite = {'exit_code': None, 'completed_phases': [], 'cleanup_scope': 'owned phase groups only'}
try:
    for name, budget in phases:
        command = [sys.executable, str(ROOT / 'smoke-child.py')] if smoke else ['bash', str(ROOT / 'validate.sh'), name]
        status = {'phase': name, 'command': command, 'exit_code': None, 'log_limit_hit': False}
        start = time.monotonic()
        deadline = min(start + budget, overall)
        try:
            if interrupted or start >= overall:
                status['exit_code'] = 128 + interrupted if interrupted else 124
                suite['exit_code'] = status['exit_code']
                break
            active = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                      start_new_session=True)
            status['owned_pgid'] = active.pid
            ready = selectors.DefaultSelector()
            ready.register(active.stdout, selectors.EVENT_READ)
            total = 0
            with (OUT / (name + '.log')).open('wb') as log:
                while True:
                    if interrupted:
                        status['exit_code'] = 128 + interrupted
                        break
                    if time.monotonic() >= deadline:
                        status['exit_code'] = 124
                        break
                    if not ready.select(min(0.2, max(0, deadline - time.monotonic()))):
                        continue
                    data = os.read(active.stdout.fileno(), 65536)
                    if not data:
                        # EOF does not guarantee process exit: keep the same deadline.
                        while active.poll() is None and not interrupted and time.monotonic() < deadline:
                            time.sleep(0.05)
                        status['exit_code'] = (128 + interrupted if interrupted else
                            active.returncode if active.returncode is not None else 124)
                        break
                    log.write(data[:max(0, LIMIT-total)])
                    total += len(data)
                    if total > LIMIT:
                        status['log_limit_hit'] = True
                        status['exit_code'] = 125
                        break
            ready.close()
        except Exception as exc:
            status['runner_error'] = type(exc).__name__
            status['exit_code'] = 125
        finally:
            if active is not None:
                status['cleanup'] = clean(active)
                active.stdout.close()
                active = None
                if status['cleanup']['non_z_survivors'] or status['cleanup']['errors']:
                    status['exit_code'] = 125
            status['elapsed_seconds'] = time.monotonic() - start
            (OUT / (name + '.json')).write_text(json.dumps(status, indent=2) + '\n')
        suite['completed_phases'].append(name)
        if status['exit_code'] != 0:
            suite['exit_code'] = status['exit_code']
            break
    if suite['exit_code'] is None:
        suite['exit_code'] = 128 + interrupted if interrupted else 0
except Exception as exc:
    suite['exit_code'] = 125
    suite['runner_error'] = type(exc).__name__
finally:
    if active is not None:
        suite['emergency_cleanup'] = clean(active)
    (OUT / 'suite.json').write_text(json.dumps(suite, indent=2) + '\n')
    (OUT / 'harness.exit').write_text(str(suite['exit_code']) + '\n')
raise SystemExit(suite['exit_code'])
