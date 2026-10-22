"""Bounded local proof using only children started by this script."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

root = Path(__file__).resolve().parent
evidence = root / 'cleanup-smoke-evidence'
evidence.mkdir(exist_ok=True)
results = []
for mode, expected in [('timeout',124), ('signal',143), ('leader-exit',124)]:
    out = evidence / mode
    out.mkdir(exist_ok=False)
    env = dict(os.environ, RUNNER_TEMP=str(out))
    p = subprocess.Popen([sys.executable, str(root/'supervise.py'), '--smoke-'+mode], env=env,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    pids = []
    owned_pgid = None
    try:
        until = time.monotonic()+5
        while not (out/'smoke-pids.json').exists() and time.monotonic()<until and p.poll() is None:
            time.sleep(0.05)
        pids = json.loads((out/'smoke-pids.json').read_text())
        owned_pgid = os.getpgid(pids[-1])
        if mode == 'signal':
            p.send_signal(signal.SIGTERM)
        stdout, _ = p.communicate(timeout=15)
        (out/'supervisor-output.log').write_bytes(stdout)
        suite = json.loads((out/'magpie-evidence/suite.json').read_text())
        phase = json.loads((out/'magpie-evidence/smoke.json').read_text())
        states = {}
        for pid in pids:
            try:
                states[str(pid)] = Path('/proc', str(pid), 'stat').read_text().rsplit(')',1)[1].split()[0]
            except FileNotFoundError:
                states[str(pid)] = 'absent'
        assert p.returncode == expected and suite['exit_code'] == expected and phase['exit_code'] == expected
        assert not phase['cleanup']['non_z_survivors'] and not phase['cleanup']['errors']
        assert all(state in ('absent','Z') for state in states.values()), states
        results.append({'mode':mode,'supervisor_exit':p.returncode,'phase_exit':phase['exit_code'],
                        'owned_pgid':owned_pgid,'recorded_pid_states':states,'cleanup':phase['cleanup']})
    finally:
        if p.poll() is None:
            p.send_signal(signal.SIGTERM)
            try: p.wait(timeout=8)
            except subprocess.TimeoutExpired:
                p.kill(); p.wait()
        if owned_pgid is not None:
            try: os.killpg(owned_pgid, signal.SIGKILL)
            except ProcessLookupError: pass
(evidence/'results.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps(results))
