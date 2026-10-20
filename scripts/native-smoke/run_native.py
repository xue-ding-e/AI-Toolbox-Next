"""Draft launcher: isolate native processes; only owned process-group cleanup."""
import json, os, pathlib, signal, subprocess, tempfile, time, shutil
binary=pathlib.Path(os.environ['NATIVE_BINARY']).resolve(strict=True)
tools=pathlib.Path(os.environ['NATIVE_TOOLS']).resolve(strict=True)
evidence=pathlib.Path(os.environ['NATIVE_EVIDENCE']).resolve(); evidence.mkdir(parents=True,exist_ok=True)
# Known host-default app registration/profile sentinels; metadata only, no credentials.
host_home=pathlib.Path.home()
host_config=pathlib.Path(os.environ.get('XDG_CONFIG_HOME',str(host_home/'.config')))
host_data=pathlib.Path(os.environ.get('XDG_DATA_HOME',str(host_home/'.local/share')))
host_cache=pathlib.Path(os.environ.get('XDG_CACHE_HOME',str(host_home/'.cache')))
watched=[host_config/'autostart',host_config/'mimeapps.list',host_data/'applications',
         host_data/'com.ai-toolbox',host_cache/'com.ai-toolbox',host_home/'.ai-toolbox']
def sentinel():
    values={}
    for base in watched:
        entries=[base]
        if base.is_dir() and not base.is_symlink(): entries.extend(base.rglob('*'))
        for item in entries:
            try:
                st=item.lstat(); values[str(item)]=[st.st_mode,st.st_size,st.st_mtime_ns]
            except FileNotFoundError: values[str(item)]=None
    return values
before_host=sentinel()
root=pathlib.Path(tempfile.mkdtemp(prefix='ai-native-profile-')).resolve()
for name in ('home','config','share','cache','runtime','tmp','data','codex','xvfb-framebuffers'):
    (root/name).mkdir(mode=0o700)
env={'PATH':'/usr/bin:/bin','LANG':'en_US.UTF-8','LC_ALL':'en_US.UTF-8',
     'SMOKE_NODE':str(pathlib.Path(os.environ['NATIVE_NODE']).resolve(strict=True)),
     'HOME':str(root/'home'),'XDG_CONFIG_HOME':str(root/'config'),'XDG_DATA_HOME':str(root/'share'),
     'XDG_CACHE_HOME':str(root/'cache'),'XDG_RUNTIME_DIR':str(root/'runtime'),'TMPDIR':str(root/'tmp'),
     'AI_TOOLBOX_DATA_DIR':str(root/'data'),'CODEX_HOME':str(root/'codex'),
     'SMOKE_ROOT':str(root),'SMOKE_BINARY':str(binary),'SMOKE_EVIDENCE':str(evidence),
     'SMOKE_SCRIPT':str(pathlib.Path(__file__).with_name('native_smoke.py')),'SMOKE_DRIVER':str(tools/'bin/tauri-driver')}
# The inner shell inherits ONLY this allowlist; xvfb/dbus add their own addresses.
command=['/usr/bin/xvfb-run','-a',f'--server-args=-screen 0 1280x900x24 -nolisten tcp -fbdir {root / "xvfb-framebuffers"}',
 '/usr/bin/dbus-run-session','--','/bin/bash','-c', '''set -euo pipefail
"$SMOKE_DRIVER" --native-host 127.0.0.1 --port 4444 --native-port 4445 > "$SMOKE_EVIDENCE/driver.log" 2>&1 &
for i in {1..50}; do
  if /usr/bin/curl --silent --fail http://127.0.0.1:4444/status > /dev/null; then break; fi
  sleep .2
done
/usr/bin/ss -ltnp > "$SMOKE_EVIDENCE/listeners.txt"
# Refuse public listening driver endpoints before launching the app.
/usr/bin/python3 - <<'CHECK'
import os,pathlib
lines=pathlib.Path(os.environ['SMOKE_EVIDENCE'],'listeners.txt').read_text().splitlines()
for port in (4444,4445):
 matches=[line for line in lines if any(part.endswith(':'+str(port)) for part in line.split()[3:4])]
 assert len(matches)==1 and '127.0.0.1:'+str(port) in matches[0], matches
CHECK
/usr/bin/python3 "$SMOKE_SCRIPT"
''']
(evidence/'isolation.json').write_text(json.dumps({'root':str(root),'allowed_environment_keys':sorted(env),'metadata_get':'https://github.com/coulsontl/ai-toolbox/releases/latest/download/latest.json'},indent=2))
def process_table():
    table={}
    for path in pathlib.Path('/proc').iterdir():
        if not path.name.isdigit(): continue
        try:
            raw=(path/'stat').read_text(); tail=raw[raw.rfind(')')+2:].split()
            table[int(path.name)]={'parent':int(tail[1]),'group':int(tail[2]),'start':tail[19]}
        except (OSError,ValueError,IndexError): pass
    return table

proc=None
owned={}
cleanup_errors=[]
def track():
    if not proc: return
    table=process_table()
    changed=True
    while changed:
        changed=False
        for pid, info in table.items():
            if pid in owned: continue
            parent=table.get(info['parent'])
            parent_owned=parent is not None and owned.get(info['parent'])==parent['start']
            if pid==proc.pid or info['group']==proc.pid or parent_owned:
                owned[pid]=info['start']; changed=True

def live_owned():
    return {pid:info for pid,info in process_table().items() if owned.get(pid)==info['start']}

try:
    with (evidence/'native.log').open('w') as log:
        proc=subprocess.Popen(command,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        deadline=time.monotonic()+420
        while proc.poll() is None:
            track()
            if time.monotonic()>deadline: raise TimeoutError('Native smoke runtime exceeded 420 seconds')
            time.sleep(.1)
        track()
        result=proc.returncode
    if result: raise SystemExit(result)
finally:
    track()
    # Capture only our exclusive Xvfb framebuffer before owned-process cleanup.
    framebuffer=root/'xvfb-framebuffers'/'Xvfb_screen0'
    try:
        if framebuffer.is_file(): shutil.copyfile(framebuffer,evidence/'native-display-final.xwd')
        else: (evidence/'framebuffer-error.txt').write_text('Owned Xvfb framebuffer missing')
    except Exception as error:
        # Evidence failure must never skip process cleanup.
        try: (evidence/'framebuffer-error.txt').write_text(str(error))
        except Exception: pass
    # Signal only identities captured as descendants / members of our owned group.
    # /proc start time protects against PID reuse. No name-based global pkill.
    for sig in (signal.SIGTERM, signal.SIGKILL):
        for pid in live_owned():
            try: os.kill(pid,sig)
            except ProcessLookupError: pass
        time.sleep(1)
    if proc:
        try: proc.wait(timeout=5)
        except subprocess.TimeoutExpired: cleanup_errors.append('Launcher did not exit')
    survivors=list(live_owned())
    if survivors: cleanup_errors.append('Owned native/driver/Xvfb/dbus processes remain')
    after_host=sentinel()
    changes=[path for path in before_host.keys() | after_host.keys() if before_host.get(path)!=after_host.get(path)]
    (evidence/'host-path-sentinel.json').write_text(json.dumps({'checked_paths':[str(p) for p in watched], 'changed_paths':changes},indent=2))
    if changes: cleanup_errors.append('Host-default profile/registration paths changed')
    inventory=[]
    for path in root.rglob('*'):
        if not path.resolve().is_relative_to(root):
            cleanup_errors.append('Profile symlink escaped: '+str(path)); continue
        if path.is_file(): inventory.append(str(path.relative_to(root)))
    (evidence/'profile-paths.json').write_text(json.dumps(sorted(inventory),indent=2))
    # Only this mkdtemp-owned root is removed; no broad home cleanup.
    if root.name.startswith('ai-native-profile-') and root.parent==pathlib.Path(tempfile.gettempdir()).resolve():
        shutil.rmtree(root)
    else: cleanup_errors.append('Unexpected profile root; removal refused')
    (evidence/'cleanup.json').write_text(json.dumps({'owned_process_count':len(owned),'surviving_owned_pids':survivors,'profile_removed':not root.exists(),'errors':cleanup_errors},indent=2))
    if cleanup_errors: raise RuntimeError(cleanup_errors)
