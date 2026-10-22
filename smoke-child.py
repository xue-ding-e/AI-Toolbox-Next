"""Local synthetic cleanup fixture; parent and child both ignore TERM."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

signal.signal(signal.SIGTERM, signal.SIG_IGN)
if len(sys.argv) == 1:
    child = subprocess.Popen([sys.executable, __file__, 'child'])
    (Path(os.environ['RUNNER_TEMP']) / 'smoke-pids.json').write_text(json.dumps([os.getpid(), child.pid]))
    if os.environ.get('SMOKE_LEADER_EXIT') == '1':
        raise SystemExit(0)
while True:
    time.sleep(0.1)
