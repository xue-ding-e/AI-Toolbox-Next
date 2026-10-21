#!/usr/bin/env python3
"""Run a synthetic validation phase with owned-process cleanup and bounded logs."""
import json
import os
import pathlib
import signal
import selectors
import time
import subprocess
import sys

name, duration, *command = sys.argv[1:]
evidence = pathlib.Path(os.environ['RUNNER_TEMP']) / 'magpie-evidence'
evidence.mkdir(exist_ok=True)
proc = None
status = {'phase': name, 'command': command, 'exit_code': None, 'log_limit_hit': False}
limit = 16 * 1024 * 1024


def stop():
    if proc is None:
        return
    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        pass
    # A command may have exited after starting a descendant that kept stdout.
    # The new session belongs exclusively to this phase; clean up that group too.
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    proc.wait()


def interrupted(signum, _frame):
    status['signal'] = signum
    stop()
    raise SystemExit(128 + signum)


signal.signal(signal.SIGTERM, interrupted)
signal.signal(signal.SIGINT, interrupted)
try:
    with (evidence / (name + '.log')).open('wb') as log:
        proc = subprocess.Popen(['timeout', '-k', '10s', duration, *command],
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                start_new_session=True)
        total = 0
        deadline = time.monotonic() + int(duration.removesuffix('m')) * 60 + 12
        ready = selectors.DefaultSelector()
        ready.register(proc.stdout, selectors.EVENT_READ)
        while True:
            remaining_time = deadline - time.monotonic()
            if remaining_time <= 0:
                status['exit_code'] = 124
                stop()
                break
            if not ready.select(min(1, remaining_time)):
                continue
            chunk = os.read(proc.stdout.fileno(), 65536)
            if not chunk:
                break
            remaining = limit - total
            log.write(chunk[:max(0, remaining)])
            total += len(chunk)
            if total > limit:
                status['log_limit_hit'] = True
                stop()
                break
        command_status = proc.wait()
        if status['exit_code'] is None:
            status['exit_code'] = command_status
        if status['log_limit_hit']:
            status['exit_code'] = 125
finally:
    stop()
    if proc is not None and status['exit_code'] is None:
        status['exit_code'] = proc.returncode
    (evidence / (name + '.json')).write_text(json.dumps(status, indent=2) + '\n')
    print(json.dumps(status), flush=True)
raise SystemExit(status['exit_code'] if status['exit_code'] is not None else 125)
