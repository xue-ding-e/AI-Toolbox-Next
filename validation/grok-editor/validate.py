#!/usr/bin/env python3
"""One attempt only. Source preparation; do not execute without reviewed slot approval.
Supervisor identity/WNOWAIT/pidfd/reap code is reused from the reviewed root-recovery
baseline driver. Only an aggregate RSS observation is added to its run loop.
No RLIMIT_AS or browser flag changes. CI uses a fresh frozen official-registry dependency installation, not a local donor.
"""
from contextlib import contextmanager
import hashlib, json, os, re, selectors, shutil, signal, stat, subprocess, sys, tempfile, time
from pathlib import Path
ROOT=Path(os.environ['GITHUB_WORKSPACE'])
SOURCE=ROOT/'source'
DONOR=None  # Fresh CI installation, no shared donor
REVIEW=ROOT/'harness/validation/grok-editor'
BASE='aaafdea703c00be61f53a4982f4972e351fd5223'
LOG_LIMIT=TOTAL_LOG_LIMIT=16*1024*1024
FINALIZE_RESERVE=120
RECORDS=[]; TOTAL_LOG_BYTES=0; ACTIVE=None; EVIDENCE=None; PHASE='grok'; VARIANT=''
JOB_DEADLINE=None; FINALIZING=False; FIXTURE=None
CLEANUP_DEADLINE=None; FIXTURE_STARTED=False; TIMINGS=[]
PRIVATE=None; PRIVATE_ID=None; SOURCE_IDENTITIES={}; SCAN_AT=0; PEAK_RSS=0; PREP_DEADLINE=None

def require(ok, message):
    if not ok: raise RuntimeError(message)

def digest(data): return hashlib.sha256(data).hexdigest()

def write_json(name,value):
    dest=EVIDENCE/name
    tmp=dest.with_suffix(dest.suffix+'.tmp')
    tmp.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
    tmp.replace(dest)

def check_fixture_budget(): pass  # No original Rust fixture is involved.

def deadline():
    require(time.monotonic()<JOB_DEADLINE-(5 if FINALIZING else FINALIZE_RESERVE),'overall deadline reserve')
    if PREP_DEADLINE is not None: require(time.monotonic()<PREP_DEADLINE,'preparation deadline')

def check_rss(members):
    global PEAK_RSS
    rss=int(Path('/proc/self/statm').read_text().split()[1])*os.sysconf('SC_PAGE_SIZE')
    for pid,identity in members.items():
        try:
            current=process_table().get(pid)
            if current and current['start']==identity['start']:
                rss+=int(Path(f'/proc/{pid}/statm').read_text().split()[1])*os.sysconf('SC_PAGE_SIZE')
        except (FileNotFoundError,ProcessLookupError): pass
    PEAK_RSS=max(PEAK_RSS,rss)
    require(rss<=3*1024**3,'sampled aggregate RSS exceeds 3GiB')

def check_disk_budgets():
    global SCAN_AT
    deadline()
    if time.monotonic()-SCAN_AT<0.5: return
    SCAN_AT=time.monotonic()
    check_rss({})
    st=PRIVATE.lstat()
    require((st.st_dev,st.st_ino)==PRIVATE_ID and stat.S_ISDIR(st.st_mode),'private root identity changed')
    require(shutil.disk_usage(PRIVATE).free>=512*1024**2,'disk low-water 512MiB')
    used=sum(p.stat().st_size for root in (PRIVATE/'store', PRIVATE/'s/node_modules') if root.exists() for p in root.rglob('*') if not p.is_symlink() and p.is_file())
    require(used<=3*1024**3,'private dependency/store 3GiB budget')
    size=0; entries=0
    # Private generated output only. Never follow browser symlinks or read targets.
    for name in ('a','t','h','c','x'):
        p=PRIVATE/name
        for parent,dirs,files,fd in os.fwalk(p,follow_symlinks=False):
            for entry in dirs+files:
                item=os.stat(entry,dir_fd=fd,follow_symlinks=False)
                entries+=1
                if stat.S_ISREG(item.st_mode): size+=item.st_size
                elif stat.S_ISLNK(item.st_mode): size+=item.st_size
                else: require(stat.S_ISDIR(item.st_mode) or stat.S_ISSOCK(item.st_mode),'unexpected private output type')
                require(entries<=100000 and size<=256*1024**2,'private output 256MiB/100000 entries budget')

def process_table():
    """PID identities include start ticks, preventing PID-reuse cleanup mistakes."""
    result = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            raw = (entry / "stat").read_text()
            fields = raw[raw.rindex(")") + 2:].split()
            result[int(entry.name)] = {
                "state": fields[0], "ppid": int(fields[1]),
                "pgid": int(fields[2]), "sid": int(fields[3]),
                "start": int(fields[19]),
            }
        except (FileNotFoundError, ProcessLookupError):
            pass
    return result

def owned_members(leader, known):
    table = process_table()
    # Same session/group plus previously observed descendants, including those
    # that changed group/session. This does not promise containment of hostile
    # descendants that escape before their first observation; runner job owns
    # that outer boundary in CI. This local run observes Chromium descendants,
    # but does not claim hard containment of an unseen escaping daemon. A local
    # survivor or uncertain cleanup is failure, not successful validation.
    changed = True
    while changed:
        changed = False
        for pid, item in table.items():
            parent = table.get(item["ppid"])
            known_parent = parent and known.get(item["ppid"]) == parent["start"]
            if item["sid"] == leader or item["pgid"] == leader or known_parent:
                if pid not in known:
                    known[pid] = item["start"]
                    changed = True
    return {
        pid: item for pid, item in table.items()
        if known.get(pid) == item["start"] and item["state"] != "Z"
    }

def observe_exit(child):
    """Observe without reaping, retaining the leader PID/session generation.

    A zombie leader cannot have its numeric PID reused. Keep it until the final
    ownership scan has finished, so matching sid/pgid cannot admit an unrelated
    new session merely because the old leader PID was reallocated.
    """
    result = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    if result is None:
        return None
    if result.si_code == os.CLD_EXITED:
        return result.si_status
    require(result.si_code in {os.CLD_KILLED, os.CLD_DUMPED}, "unexpected child wait status")
    return -result.si_status

def cleanup(child, known):
    """Bounded TERM then KILL, only verified owned identities; always reap leader."""
    for sig, seconds in [(signal.SIGTERM, 2), (signal.SIGKILL, 3)]:
        members = owned_members(child.pid, known)
        for pid, item in members.items():
            # Recheck identity immediately before signaling this specific PID.
            current = process_table().get(pid)
            if current and current["start"] == item["start"]:
                try:
                    fd = os.pidfd_open(pid)
                    try:
                        verified = process_table().get(pid)
                        if verified and verified["start"] == item["start"]:
                            signal.pidfd_send_signal(fd, sig)
                    finally:
                        os.close(fd)
                except ProcessLookupError:
                    pass
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            observe_exit(child)
            if not owned_members(child.pid, known):
                break
            time.sleep(0.05)
    # This final ownership scan occurs while leader PID is still reserved.
    # No sid/pgid scan is permitted after the unique final reap below.
    remaining = owned_members(child.pid, known)
    try:
        child.wait(timeout=1)
    except subprocess.TimeoutExpired:
        remaining["leader_not_reaped"] = {"pid": child.pid}
    return remaining

def run(label, command, timeout, cwd, env, allowed_codes=(0,)):
    """Nonblocking pipe drain plus owned process observation with hard deadlines."""
    global ACTIVE, TOTAL_LOG_BYTES
    if not FIXTURE_STARTED and not FINALIZING:
        signal.setitimer(signal.ITIMER_REAL,max(0.001,JOB_DEADLINE-time.monotonic()-FINALIZE_RESERVE))
    if VARIANT:
        label = VARIANT + "-" + label
    require(re.fullmatch(r"[a-z0-9-]+", label), "unsafe log label")
    output_paths = {key: EVIDENCE / f"{PHASE}-{label}.{key}.log" for key in ("stdout", "stderr")}
    require(not any(path.exists() for path in output_paths.values()), "duplicate command label")
    started = time.monotonic()
    requested_timeout = timeout
    if JOB_DEADLINE is not None:
        timeout = min(timeout, JOB_DEADLINE - time.monotonic() - (10 if FINALIZING else FINALIZE_RESERVE))
    require(timeout > 0, "job evidence/cleanup reserve reached")
    record = {"label": label, "argv": command, "timeout_seconds": timeout,
              "requested_timeout_seconds": requested_timeout,
              "exit_code": None, "reason": "not_started", "survivors": {},
              "log_bytes_seen": 0, "logs": {key: path.name for key, path in output_paths.items()}}
    child = None
    known = {}
    selector = selectors.DefaultSelector()
    files = {}
    reason = "exception"
    try:
        files = {key: path.open("wb") for key, path in output_paths.items()}
        child = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                 start_new_session=True)
        ACTIVE = child.pid
        record["pid"] = child.pid
        initial = process_table().get(child.pid)
        if initial:
            known[child.pid] = initial["start"]
        for stream, key in [(child.stdout, "stdout"), (child.stderr, "stderr")]:
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, key)
        deadline = started + timeout
        while True:
            check_disk_budgets()
            if FIXTURE is not None:
                check_fixture_budget()
            exit_code = observe_exit(child)
            members = owned_members(child.pid, known)
            check_rss(members)
            if exit_code is not None and members:
                reason = "surviving_descendants_after_parent_exit"
                break
            if time.monotonic() >= deadline:
                reason = "timeout"
                break
            for key, _mask in selector.select(timeout=0.05):
                try:
                    data = os.read(key.fileobj.fileno(), 65536)
                except BlockingIOError:
                    continue
                if not data:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                remaining = min(LOG_LIMIT - record["log_bytes_seen"], TOTAL_LOG_LIMIT - TOTAL_LOG_BYTES)
                files[key.data].write(data[:max(0, remaining)])
                record["log_bytes_seen"] += len(data)
                TOTAL_LOG_BYTES += len(data)
            if record["log_bytes_seen"] > LOG_LIMIT or TOTAL_LOG_BYTES > TOTAL_LOG_LIMIT:
                reason = "log_soft_limit_exceeded"
                break
            if exit_code is not None and not selector.get_map():
                reason = "completed" if exit_code in allowed_codes else "nonzero_exit"
                break
    finally:
        try:
            if child is not None:
                try:
                    record["survivors"] = cleanup(child, known)
                    record["exit_code"] = child.returncode
                except BaseException as cleanup_error:
                    reason = "cleanup_failed"
                    record["survivors"] = {"unknown": "cleanup could not be confirmed"}
                    record["cleanup_error"] = f"{type(cleanup_error).__name__}: {cleanup_error}"
        finally:
            ACTIVE = None
            record["reason"] = reason
            record["duration_seconds"] = round(time.monotonic() - started, 3)
            for key in list(selector.get_map().values()):
                key.fileobj.close()
            selector.close()
            for file in files.values():
                file.close()
            RECORDS.append(record)
            write_json(f"{PHASE}-commands.json", RECORDS)
    if not FIXTURE_STARTED and not FINALIZING:
        signal.setitimer(signal.ITIMER_REAL,max(0.001,JOB_DEADLINE-time.monotonic()-FINALIZE_RESERVE))
    require(reason == "completed" and not record["survivors"], f"{label}: {reason}; see command evidence")
    return output_paths["stdout"].read_bytes()

# Only the timer envelope is task-specific; identity/signaling/reap algorithm stays audited.
_audited_cleanup = cleanup
def cleanup(child, known):
    if FIXTURE_STARTED:
        arm_final_budget()
    else:
        signal.setitimer(signal.ITIMER_REAL,max(0.001,min(60,JOB_DEADLINE-time.monotonic())))
    return _audited_cleanup(child, known)

SOURCE_SEAL='8ef35e28c49cd179b85cd7b89cf764b2184833adf26e71e0d3d92ca4c3c865e9'
NODE=shutil.which('node')
PNPM=shutil.which('pnpm')
BROWSER='/usr/bin/google-chrome'
PACKAGE_SHA='958e0506b98b761e6da69b2f30f0dfbc3a989a3709dd240851de1578a1f3eba7'
LOCK_SHA='a71feeda32c8cfee44d686dc21d5707239b8a2bb4e368c19d5c3d23c22bf6730'

def read_regular(path):
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW)
    try:
        before=os.fstat(fd); require(stat.S_ISREG(before.st_mode),'nonregular input')
        out=bytearray()
        while True:
            deadline(); part=os.read(fd,1024*1024)
            if not part: break
            out.extend(part)
        after=os.fstat(fd)
        require((before.st_dev,before.st_ino,before.st_size,before.st_mtime_ns,before.st_ctime_ns)==
                (after.st_dev,after.st_ino,after.st_size,after.st_mtime_ns,after.st_ctime_ns),'input changed during read')
        return bytes(out),before
    finally: os.close(fd)

def tree_snapshot(root, copy_to=None):
    root_id=root.lstat(); require(stat.S_ISDIR(root_id.st_mode),'input root not real directory')
    result={'.':{'kind':'directory','mode':stat.S_IMODE(root_id.st_mode),'identity':[root_id.st_dev,root_id.st_ino],'mtime_ns':root_id.st_mtime_ns,'ctime_ns':root_id.st_ctime_ns}}; total=0
    for parent,dirs,files in os.walk(root,followlinks=False):
        deadline()
        for name in sorted(dirs+files):
            p=Path(parent)/name; rel=str(p.relative_to(root)); st=p.lstat()
            record={'mode':stat.S_IMODE(st.st_mode),'identity':[st.st_dev,st.st_ino], 'mtime_ns':st.st_mtime_ns,'ctime_ns':st.st_ctime_ns}
            target=copy_to/rel if copy_to else None
            if stat.S_ISLNK(st.st_mode):
                text=os.readlink(p)
                require(not os.path.isabs(text) and p.resolve(strict=True).is_relative_to(root.resolve()),'dependency link escapes donor')
                record.update(kind='symlink',sha256=digest(text.encode()))
                if target: target.parent.mkdir(parents=True,exist_ok=True); os.symlink(text,target)
            elif stat.S_ISDIR(st.st_mode):
                record.update(kind='directory')
                if target: target.mkdir(mode=0o700,parents=True,exist_ok=True)
            else:
                require(stat.S_ISREG(st.st_mode),'special dependency input')
                data,actual=read_regular(p); total+=len(data)
                require(total<=1024**3,'dependency payload exceeds 1GiB')
                require(actual.st_ino==st.st_ino and actual.st_dev==st.st_dev,'input identity changed')
                record.update(kind='file',sha256=digest(data))
                if target:
                    target.parent.mkdir(parents=True,exist_ok=True)
                    with target.open('xb') as f: f.write(data)
                    target.chmod(stat.S_IMODE(st.st_mode))
                    copied=target.lstat()
                    require(copied.st_nlink==1 and (copied.st_dev,copied.st_ino)!=(st.st_dev,st.st_ino),'private copy shares inode')
            result[rel]=record
            require(len(result)<=150000,'dependency entries budget')
        check_disk_budgets()
    after=root.lstat()
    require((root_id.st_dev,root_id.st_ino)==(after.st_dev,after.st_ino),'input root replaced')
    return result

def content_records(records):
    # Directory permissions are intentionally private in the new copy.
    return {p:{k:v for k,v in r.items() if k in ('kind','sha256') or (k=='mode' and r['kind']!='directory')} for p,r in records.items()}

def verify_source(base, expected, copy_to=None):
    root=base.lstat(); require(stat.S_ISDIR(root.st_mode),'source root not directory')
    identity=(root.st_dev,root.st_ino)
    require(SOURCE_IDENTITIES.setdefault(str(base),identity)==identity,'source root identity changed')
    for rel,record in expected.items():
        deadline(); require(not Path(rel).is_absolute() and '..' not in Path(rel).parts,'unsafe source relative path')
        p=base/rel
        for parent in p.parents:
            if parent==base: break
            require(stat.S_ISDIR(parent.lstat().st_mode),'source parent is not a real directory')
        require(p.resolve(strict=True).is_relative_to(base.resolve(strict=True)),'source path escapes root')
        st=p.lstat()
        if record['kind']=='symlink':
            require(stat.S_ISLNK(st.st_mode),'source type changed')
            value=os.readlink(p)
            require(not os.path.isabs(value) and p.resolve(strict=True).is_relative_to(base.resolve()),'source symlink escapes')
            data=value.encode()
        else:
            data,_=read_regular(p)
        require(digest(data)==record['sha256'] and stat.S_IMODE(st.st_mode)==record['mode'],'source seal mismatch: '+rel)
        if copy_to:
            dest=copy_to/rel; dest.parent.mkdir(parents=True,exist_ok=True)
            if record['kind']=='symlink': os.symlink(value,dest)
            else:
                with dest.open('xb') as f: f.write(data)
                dest.chmod(record['mode'])
    after=base.lstat(); require((after.st_dev,after.st_ino)==identity,'source root changed during check')

def arm_final_budget():
    # A single 120-second allowance shared by fixture-child cleanup and final seals.
    global CLEANUP_DEADLINE
    if CLEANUP_DEADLINE is None:
        CLEANUP_DEADLINE=min(JOB_DEADLINE,time.monotonic()+120)
    signal.setitimer(signal.ITIMER_REAL,max(0.001,CLEANUP_DEADLINE-time.monotonic()))

@contextmanager
def timed_phase(name):
    record={'phase':name,'started_monotonic':time.monotonic(),'state':'running'}
    TIMINGS.append(record); write_json('phase-timings.json',TIMINGS)
    try:
        yield
        record['state']='completed'
    except BaseException:
        record['state']='interrupted_or_failed'
        raise
    finally:
        record['elapsed_seconds']=time.monotonic()-record['started_monotonic']
        write_json('phase-timings.json',TIMINGS)

def interrupt(signum,frame):
    if FINALIZING and signum==signal.SIGALRM:
        # Hard finalization ceiling. An absent final verdict is failure, never success.
        os._exit(124)
    arm_final_budget()
    raise RuntimeError('supervisor interrupted/deadline: '+str(signum))

def main():
    global PRIVATE,PRIVATE_ID,EVIDENCE,JOB_DEADLINE,PREP_DEADLINE,FINALIZING,FIXTURE_STARTED
    started=time.monotonic(); JOB_DEADLINE=min(started+1380,started+int(os.environ['SHARED_DEADLINE_EPOCH'])-time.time()); PREP_DEADLINE=None
    for sig in (signal.SIGTERM,signal.SIGINT,signal.SIGALRM): signal.signal(sig,interrupt)
    # One shared deadline covers source preparation, dependency installation, fixture and finalization.
    signal.setitimer(signal.ITIMER_REAL,max(0.001,JOB_DEADLINE-time.monotonic()-FINALIZE_RESERVE))
    require(hasattr(os,'pidfd_open') and hasattr(signal,'pidfd_send_signal') and hasattr(os,'WNOWAIT'),'Linux supervisor unavailable')
    require(shutil.disk_usage('/tmp').free>=2*1024**3,'initial free space below 2GiB')
    with (ROOT/'grok-attempt.started').open('x') as f: f.write('one reviewed attempt only\n')
    PRIVATE=Path(tempfile.mkdtemp(prefix='ge-',dir='/tmp'))
    st=PRIVATE.lstat(); PRIVATE_ID=(st.st_dev,st.st_ino)
    require(len(os.fsencode(PRIVATE/'t'))+60<108,'private temporary socket path bound')
    require(stat.S_IMODE(st.st_mode)==0o700 and st.st_uid==os.getuid(),'private root ownership/mode')
    # Pin the root identity throughout execution; never remove an unverified tree.
    root_fd=os.open(PRIVATE,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
    EVIDENCE=PRIVATE/'e'; EVIDENCE.mkdir(mode=0o700)
    for name in ('s','a','t','h','c','x'): (PRIVATE/name).mkdir(mode=0o700)
    source_dir=(PRIVATE/'s').lstat(); SOURCE_IDENTITIES[str(PRIVATE/'s')]=(source_dir.st_dev,source_dir.st_ino)
    result={'ok':False,'phase':'preparation','private_root':str(PRIVATE),'process_cleanup':'not_started','source_final':'not_checked','donor_final':'not_checked','private_dependencies_final':'not_checked'}
    donor_before=None; source_records=None; private_before=None
    try:
        seal=read_regular(REVIEW/'source.json')[0]
        require(digest(seal)==SOURCE_SEAL,'source manifest hash')
        source_records=json.loads(seal)['files']
        require(subprocess.check_output(['git','-C',str(SOURCE),'rev-parse','HEAD'],timeout=5).decode().strip()==BASE,'base changed')
        require(digest(read_regular(SOURCE/'package.json')[0])==PACKAGE_SHA,'package mismatch')
        require(digest(read_regular(SOURCE/'pnpm-lock.yaml')[0])==LOCK_SHA,'lock mismatch')
        with timed_phase('source-copy-and-verification'):
            verify_source(SOURCE,source_records,PRIVATE/'s')
        env={'PATH':os.environ['PATH'],'HOME':str(PRIVATE/'h'),'TMPDIR':str(PRIVATE/'t'),'TMP':str(PRIVATE/'t'),'TEMP':str(PRIVATE/'t'),'XDG_CACHE_HOME':str(PRIVATE/'c'),'XDG_CONFIG_HOME':str(PRIVATE/'h/config'),'XDG_RUNTIME_DIR':str(PRIVATE/'x'),'LANG':'C.UTF-8','LC_ALL':'C.UTF-8','CI':'true','HUSKY':'0','npm_config_registry':'https://registry.npmjs.org/','npm_config_userconfig':'/dev/null','npm_config_globalconfig':'/dev/null'}
        require('NODE_PATH' not in env and NODE and PNPM,'runtime missing')
        result['phase']='version-check'
        require(run('node-version',[NODE,'--version'],5,PRIVATE/'s',env).strip()==b'v24.19.0','Node version drift')
        require(run('pnpm-version',[PNPM,'--version'],5,PRIVATE/'s',env).strip()==b'9.15.0','pnpm version drift')
        result['browser_version']=run('browser-version',[BROWSER,'--version'],5,PRIVATE/'s',env).decode().strip()
        require(result['browser_version'].startswith('Google Chrome '),'unexpected preinstalled browser')
        require(os.geteuid()!=0 and os.uname().machine=='x86_64','requires non-root Linux x86_64 runner with existing Chrome sandbox')
        result['phase']='locked-dependencies'
        with timed_phase('fresh-locked-install'):
            run('dependencies',[PNPM,'install','--frozen-lockfile','--ignore-scripts','--store-dir',str(PRIVATE/'store')],600,PRIVATE/'s',env)
        require(digest(read_regular(PRIVATE/'s/node_modules/.pnpm/lock.yaml')[0])==LOCK_SHA,'installed lock drift')
        native=PRIVATE/'s/node_modules/.pnpm/@esbuild+linux-x64@0.27.7/node_modules/@esbuild/linux-x64'
        require(json.loads(read_regular(native/'package.json')[0])['version']=='0.27.7' and os.access(native/'bin/esbuild',os.X_OK),'locked esbuild optional binary unavailable; no install-script fallback')
        with timed_phase('installed-dependency-seal'):
            private_before=tree_snapshot(PRIVATE/'s/node_modules')
        write_json('dependency-preparation.json',{'entries':len(private_before),'content_seal':digest(json.dumps(content_records(private_before),sort_keys=True).encode()),'method':'fresh frozen official registry installation in task-only store; no shared donor'})
        result['phase']='six-case-fixture'
        FIXTURE_STARTED=True
        signal.setitimer(signal.ITIMER_REAL,max(0.001,min(420,JOB_DEADLINE-time.monotonic()-FINALIZE_RESERVE)))
        run('grok-editor',[NODE,'scripts/verify-grok-editor-readback.mjs','--browser',BROWSER,'--output',str(PRIVATE/'a')],420,PRIVATE/'s',env)
        reports=list((PRIVATE/'a').glob('grok-editor-*/grok-editor-cases.json'))
        require(len(reports)==1,'expected exactly one case report')
        report=json.loads(read_regular(reports[0])[0]); expected=['readback','cancel','save','pending','retry','remount']
        require([x['scenario'] for x in report['results']]==expected and report['passed']==6 and report['failed']==0 and report['pageExceptions']==[],'six-case verdict failed')
        require(all(x['passed'] is True for x in report['results']),'case not passed')
        for case in report['results']:
            final=[o['state'] for o in case['observations'] if o['label']=='final']
            require(len(final)==1 and final[0]['routing']['unexpected']==[], 'unexpected routing operation or missing safety observation')
        result['ok']=True
    except BaseException as error:
        result['error']=type(error).__name__+': '+str(error)
    finally:
        PREP_DEADLINE=None; FINALIZING=True
        arm_final_budget()
        # run() attempted cleanup/reaping; only the recorded survivor result can confirm it.
        result['process_cleanup']=('not_started' if not RECORDS else ('confirmed' if all(not r['survivors'] for r in RECORDS) else 'incomplete'))
        try:
            require(time.monotonic()<JOB_DEADLINE,'overall budget exhausted')
            held=os.fstat(root_fd); current=PRIVATE.lstat()
            require((held.st_dev,held.st_ino)==PRIVATE_ID==(current.st_dev,current.st_ino),'private root identity lost')
            if source_records:
                with timed_phase('final-original-source-seal'):
                    verify_source(SOURCE,source_records)
                result['original_source_final']='unchanged'
                if (PRIVATE/'s/package.json').exists():
                    with timed_phase('final-private-source-seal'):
                        verify_source(PRIVATE/'s',source_records)
                result['source_final']='unchanged'
            result['donor_final']='not_applicable_fresh_ci_install'
            if private_before is not None:
                with timed_phase('final-private-dependency-full-seal'):
                    require(tree_snapshot(PRIVATE/'s/node_modules')==private_before,'private dependency cache changed')
                result['private_dependencies_final']='unchanged'
            check_disk_budgets()
        except BaseException as error:
            result['ok']=False; result['finalization_error']=type(error).__name__+': '+str(error)
        result.update(phase_timings=TIMINGS,commands=RECORDS,peak_sampled_rss=PEAK_RSS,elapsed_seconds=time.monotonic()-started,
                      retained_private_files=True,scope='six real-component browser cases with strict IPC and fail-closed router mocks; no real router initialization/navigation/share/deep-link/native/disk persistence or continuous gate claim')
        if result['process_cleanup']!='confirmed': result['ok']=False
        write_json('result.json',result)
        shutil.copytree(EVIDENCE,ROOT/'evidence',dirs_exist_ok=True)
        exported=ROOT/'evidence/artifacts'; exported.mkdir(exist_ok=True)
        for directory in (PRIVATE/'a').glob('grok-editor-*'):
            if directory.is_dir() and not directory.is_symlink():
                for item in directory.iterdir():
                    if not item.is_symlink() and item.is_file() and item.suffix in ('.json','.png','.html','.bin','.log'):
                        shutil.copyfile(item,exported/item.name)
        os.close(root_fd); signal.setitimer(signal.ITIMER_REAL,0)
        print(json.dumps({'ok':result['ok'],'evidence':str(EVIDENCE/'result.json'),'phase':result['phase']}))
    return 0 if result['ok'] else 1

if __name__=='__main__': sys.exit(main())
