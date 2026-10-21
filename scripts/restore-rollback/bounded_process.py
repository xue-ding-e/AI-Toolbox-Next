"""Linux CI watchdog: one owned process group, bounded wait and cleanup evidence."""
import json, os, pathlib, signal, subprocess, sys, time
limit=float(sys.argv[1]); log=pathlib.Path(sys.argv[2]); command=sys.argv[3:]
def members(group):
    result=[]
    for item in pathlib.Path('/proc').iterdir():
        if not item.name.isdigit(): continue
        try:
            fields=(item/'stat').read_text().rsplit(')',1)[1].split()
            if int(fields[2])==group and fields[0]!='Z': result.append(int(item.name))
        except (FileNotFoundError,ProcessLookupError): pass
    return result
with log.open('wb') as output:
    p=subprocess.Popen(command,stdout=output,stderr=subprocess.STDOUT,start_new_session=True)
    reason=None
    deadline=time.monotonic()+limit
    while p.poll() is None:
        if time.monotonic()>=deadline: reason='timeout'; break
        if log.stat().st_size>32*1024*1024: reason='log-limit'; break
        time.sleep(.1)
    remaining=members(p.pid)
    if remaining and reason is None: reason='surviving-descendants'
    if reason or remaining:
        for sig in (signal.SIGTERM,signal.SIGKILL):
            if not members(p.pid): break
            try: os.killpg(p.pid,sig)
            except ProcessLookupError: pass
            until=time.monotonic()+5
            while members(p.pid) and time.monotonic()<until: time.sleep(.1)
    try: code=p.wait(timeout=5)
    except subprocess.TimeoutExpired: code=None
    remaining=members(p.pid)
log.with_suffix(log.suffix+'.process.json').write_text(json.dumps({'pid':p.pid,'reason':reason,'exit':code,'survivors':remaining}))
if remaining or code is None: raise SystemExit('owned process exit unconfirmed; outer job timeout remains safeguard')
if reason or code: raise SystemExit(1)
