#!/usr/bin/env python3
"""One attempt only. Source preparation; do not execute without reviewed slot approval.
Supervisor identity/WNOWAIT/pidfd/reap code is reused from the reviewed root-recovery
baseline driver. Only an aggregate RSS observation is added to its run loop.
No RLIMIT_AS or browser flag changes. CI uses a fresh frozen official-registry dependency installation, not a local donor.
"""
from contextlib import contextmanager
import errno, hashlib, json, os, re, selectors, shutil, signal, stat, subprocess, sys, tempfile, time
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
DEPENDENCY_INSTALL_STARTED=False; DEPENDENCY_INSTALL_ACTIVE=False
DEPENDENCY_SAMPLES={'samples':0,'enoent_misses':0,'last_samples':[],'assurance':'sampled metadata only; not full verification or quota'}
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

RSS_LIMIT=9*1024**3//2
MEMORY_PHASE='preparation'
MEMORY_DIAGNOSTICS={'metric':'sum of sampled RSS; shared pages may be counted multiple times; not PSS',
                    'limit_bytes':RSS_LIMIT,'initial':None,'phase_peaks':{},'recent_samples':[]}

def memory_available():
    fields={}
    for line in Path('/proc/meminfo').read_text().splitlines():
        key,_,value=line.partition(':')
        if key in ('MemTotal','MemAvailable'):
            parts=value.split()
            require(len(parts)==2 and parts[1]=='kB','unexpected memory unit')
            fields[key]=int(parts[0])*1024
    require(set(fields)=={'MemTotal','MemAvailable'},'memory capacity unavailable')
    return fields

def parse_v2_layout(cgroup_text, mount_text):
    lines=cgroup_text.splitlines()
    require(len(lines)==1 and lines[0].startswith('0::'),'unsupported v1/mixed cgroup hierarchy')
    relative=lines[0][3:]
    def safe_path(value):
        require(value.startswith('/') and '\\' not in value and '\x00' not in value,'unsupported cgroup path encoding')
        require(value=='/' or all(p not in ('','.','..') for p in value[1:].split('/')),'cgroup path escape')
        return value
    safe_path(relative)
    mounts=[]
    for line in mount_text.splitlines():
        left,sep,right=line.partition(' - ')
        require(bool(sep),'invalid mountinfo')
        fields=left.split(); kind=right.split()[0]
        require(kind!='cgroup','unsupported mixed/v1 cgroup mount')
        if kind=='cgroup2':
            require(len(fields)>=6,'invalid cgroup2 mountinfo')
            require(fields[3]=='/','hidden cgroup ancestors')
            mounts.append(safe_path(fields[4]))
    require(len(mounts)==1,'ambiguous or missing cgroup2 mount')
    for line in mount_text.splitlines():
        fields=line.split(' - ',1)[0].split()
        point=safe_path(fields[4])
        require(not point.startswith(mounts[0]+'/'),'nested mount hides cgroup ancestry')
    return mounts[0],relative

def bounded_control_read(fd,name):
    child=os.open(name,os.O_RDONLY|os.O_NOFOLLOW,dir_fd=fd)
    try:
        require(stat.S_ISREG(os.fstat(child).st_mode),'nonregular cgroup control')
        data=os.read(child,129)
        require(len(data)<=128,'oversized cgroup control')
        return data.decode('ascii').strip()
    finally: os.close(child)

def read_root_memory_stat(fd):
    child=os.open('memory.stat',os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK,dir_fd=fd)
    try:
        require(stat.S_ISREG(os.fstat(child).st_mode),'nonregular root memory.stat')
        data=bytearray()
        while True:
            deadline()
            part=os.read(child,min(4096,65537-len(data)))
            if not part: break
            data.extend(part)
            require(len(data)<=65536,'root memory.stat exceeds 64KiB')
        text=data.decode('ascii'); fields={}
        for line in text.splitlines():
            parts=line.split()
            require(len(parts)==2 and bool(re.fullmatch(r'[a-z][a-z0-9_]*',parts[0])) and
                    bool(re.fullmatch(r'[0-9]+',parts[1])),'invalid root memory.stat record')
            require(parts[0] not in fields,'duplicate root memory.stat record')
            fields[parts[0]]=int(parts[1])
        require({'anon','file','kernel_stack','pagetables'}<=set(fields),'incomplete root memory.stat')
        return len(fields)
    finally: os.close(child)

def read_v2_capacity():
    # Supported only by the reviewed GitHub-hosted Ubuntu VM job with no
    # container. Namespace/root checks supplement that deployment precondition;
    # this is not a generic proof against a hostile/containerized host.
    require(os.environ.get('GITHUB_ACTIONS')=='true' and
            os.environ.get('RUNNER_ENVIRONMENT')=='github-hosted' and
            os.environ.get('RUNNER_OS')=='Linux','unsupported non-hosted-VM memory observation')
    ns=os.readlink('/proc/self/ns/cgroup')
    require(bool(re.fullmatch(r'cgroup:\[[0-9]+\]',ns)) and
            os.readlink('/proc/1/ns/cgroup')==ns,'different or unknown cgroup namespace')
    pid1_cg=Path('/proc/1/cgroup').read_text()
    cg=Path('/proc/self/cgroup').read_text()
    mi=Path('/proc/self/mountinfo').read_text()
    mount,relative=parse_v2_layout(cg,mi)
    pid1_mount,_=parse_v2_layout(pid1_cg,mi)
    require(pid1_mount==mount,'PID1 cgroup mapping mismatch')
    root=Path(mount)
    require(root.resolve(strict=True)==root,'cgroup mount path alias')
    fd=os.open(root,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
    records=[]
    try:
        components=relative.strip('/').split('/') if relative!='/' else []
        require(len(components)<=128,'cgroup hierarchy depth exceeds128')
        # Root-first ensures no visible ancestor limit is skipped. Explicit
        # 'max' is unlimited only for that visible node, never for ancestors.
        for depth in range(len(components)+1):
            before=os.fstat(fd)
            if depth==0:
                # Kernel memory controller interfaces exist on non-root cgroups.
                # Linux 5.15/6.8 CFTYPE_NOT_ON_ROOT uses real cgroup_parent,
                # not namespace root. memory.stat has no such flag. Within
                # the supported ordinary VM, this positive interface plus
                # BOTH scalar controls absent distinguishes the real root.
                controllers=bounded_control_read(fd,'cgroup.controllers').split()
                require('memory' in controllers,'memory controller unavailable')
                stat_fields=read_root_memory_stat(fd)
                for control in ('memory.max','memory.current'):
                    try: bounded_control_read(fd,control)
                    except FileNotFoundError: pass
                    else: raise RuntimeError('cgroup root controls indicate incomplete ancestry')
                records.append({'depth':0,'cap_bytes':None,'current_bytes':None,
                                'root_exception':'memory.stat present; scalar controls absent on real kernel root',
                                'memory_stat_fields':stat_fields})
                if components:
                    child=os.open(components[0],os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW,dir_fd=fd)
                    os.close(fd);fd=child
                continue
            maximum=bounded_control_read(fd,'memory.max')
            current=bounded_control_read(fd,'memory.current')
            require(maximum=='max' or bool(re.fullmatch(r'[0-9]+',maximum)),'invalid cgroup memory.max')
            require(bool(re.fullmatch(r'[0-9]+',current)),'invalid cgroup memory.current')
            cap=None if maximum=='max' else int(maximum)
            used=int(current)
            after=os.fstat(fd)
            require((before.st_dev,before.st_ino)==(after.st_dev,after.st_ino),'cgroup directory changed')
            records.append({'depth':depth,'cap_bytes':cap,'current_bytes':used})
            if depth<len(components):
                child=os.open(components[depth],os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW,dir_fd=fd)
                os.close(fd);fd=child
    finally: os.close(fd)
    require(os.readlink('/proc/self/ns/cgroup')==ns and os.readlink('/proc/1/ns/cgroup')==ns,
            'cgroup namespace changed during observation')
    require(Path('/proc/1/cgroup').read_text()==pid1_cg,'PID1 cgroup changed during observation')
    require(Path('/proc/self/cgroup').read_text()==cg,'cgroup migration during observation')
    require(Path('/proc/self/mountinfo').read_text()==mi,'cgroup mounts changed during observation')
    return records

def effective_memory():
    host=memory_available()
    records=read_v2_capacity()
    finite=[r for r in records if r['cap_bytes'] is not None]
    return {**host,'effective_capacity_bytes':min([host['MemTotal']]+[r['cap_bytes'] for r in finite]),
            'effective_headroom_bytes':min([host['MemAvailable']]+[max(0,r['cap_bytes']-r['current_bytes']) for r in finite]),
            'visible_cgroups':records}

def observe_memory():
    try: return effective_memory()
    except (OSError,ValueError,RuntimeError) as error:
        # Do not expose control paths; retain classification before refusing.
        MEMORY_DIAGNOSTICS['capacity_error']={'type':type(error).__name__,'errno':getattr(error,'errno',None),
                                              'reason':str(error) if isinstance(error,RuntimeError) else 'control read or parse failed'}
        write_json('memory-observations.json',MEMORY_DIAGNOSTICS)
        raise RuntimeError('effective cgroup memory capacity unavailable') from error

def pid_start(pid):
    raw=Path(f'/proc/{pid}/stat').read_text()
    fields=raw[raw.rindex(')')+2:].split()
    return int(fields[19])

def admit_memory_budget():
    observed=observe_memory()
    MEMORY_DIAGNOSTICS['initial']=observed
    write_json('memory-observations.json',MEMORY_DIAGNOSTICS)
    require(observed['effective_capacity_bytes']>=6*1024**3,'effective memory capacity below 6GiB')
    require(observed['effective_headroom_bytes']>=5*1024**3,'initial effective memory headroom below 5GiB')

def check_rss(members):
    global PEAK_RSS
    # Identity and numeric memory metadata only: no argv, environment or paths.
    pagesize=os.sysconf('SC_PAGE_SIZE')
    self_pid=os.getpid()
    identities={**members,self_pid:{'start':pid_start(self_pid)}}
    observations=[]; rss=0
    for pid,identity in identities.items():
        if identity is None: continue
        try:
            if pid_start(pid)!=identity['start']: continue
            values=Path(f'/proc/{pid}/statm').read_text().split()
            if pid_start(pid)!=identity['start']: continue
            resident=int(values[1])*pagesize; shared=int(values[2])*pagesize
            rss+=resident
            observations.append({'pid':pid,'start_ticks':identity['start'],
                                 'rss_bytes':resident,'shared_resident_bytes':shared})
        except (FileNotFoundError,ProcessLookupError): pass
    available=observe_memory()
    PEAK_RSS=max(PEAK_RSS,rss)
    sample={'phase':MEMORY_PHASE,'aggregate_rss_bytes':rss,**available,
            'observed_process_count':len(observations),'processes':observations[:64],
            'process_details_truncated':len(observations)>64}
    prior=MEMORY_DIAGNOSTICS['phase_peaks'].get(MEMORY_PHASE)
    if prior is None or rss>prior['aggregate_rss_bytes']:
        MEMORY_DIAGNOSTICS['phase_peaks'][MEMORY_PHASE]=sample
    MEMORY_DIAGNOSTICS['recent_samples'].append(sample)
    MEMORY_DIAGNOSTICS['recent_samples']=MEMORY_DIAGNOSTICS['recent_samples'][-16:]
    write_json('memory-observations.json',MEMORY_DIAGNOSTICS)
    require(available['effective_headroom_bytes']>=512*1024**2,'effective memory low-water 512MiB')
    require(rss<=RSS_LIMIT,'sampled aggregate RSS exceeds 4.5GiB')

def sample_dependency_budget(parts_list=None):
    """Bounded metadata sample, never a complete seal or disk quota.

    During pnpm install only, an entry can disappear between enumeration and
    lstat/open. Count that ENOENT as an observation miss, not verified absence.
    Directory FDs and O_NOFOLLOW prevent traversing symlink replacements.
    Same-UID concurrent mutations can still make any sample incomplete.
    """
    if not DEPENDENCY_INSTALL_STARTED:
        return
    started=time.monotonic()
    sample={'entries':0,'regular_bytes':0,'enoent_misses':0,
            'active_install':DEPENDENCY_INSTALL_ACTIVE,'sampling_completed':False}
    def checkpoint():
        deadline()
        require(time.monotonic()-started<=10,'dependency metadata sample exceeded 10s')
        require(sample['entries']<=300000,'dependency metadata sample exceeded 300000 entries')
        require(sample['regular_bytes']<=3*1024**3,'private dependency/store 3GiB budget')
    def missing(error):
        if error.errno==errno.ENOENT and DEPENDENCY_INSTALL_ACTIVE:
            sample['enoent_misses']+=1
            return
        raise error
    def walk(fd,depth):
        checkpoint(); require(depth<=128,'dependency metadata depth exceeded128')
        # scandir receives an already pinned directory, never a symlink path.
        with os.scandir(fd) as iterator:
            for entry in iterator:
                sample['entries']+=1; checkpoint()
                try:
                    st=os.stat(entry.name,dir_fd=fd,follow_symlinks=False)
                except OSError as error:
                    missing(error); continue
                if stat.S_ISREG(st.st_mode):
                    sample['regular_bytes']+=st.st_size; checkpoint()
                elif stat.S_ISDIR(st.st_mode):
                    try:
                        child=os.open(entry.name,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW,dir_fd=fd)
                    except OSError as error:
                        missing(error); continue
                    try:
                        actual=os.fstat(child)
                        require((actual.st_dev,actual.st_ino)==(st.st_dev,st.st_ino),'dependency directory identity changed')
                        walk(child,depth+1)
                    finally:
                        os.close(child)
                else:
                    require(stat.S_ISLNK(st.st_mode),'unexpected dependency object type')
                    # Do not read a link target or traverse it. Full post-install
                    # tree_snapshot separately validates every internal link.
    rootfd=os.open(PRIVATE,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
    try:
        st=os.fstat(rootfd)
        require((st.st_dev,st.st_ino)==PRIVATE_ID,'private root identity changed')
        for parts in (parts_list if parts_list is not None else [('store',),('s','node_modules')]):
            fd=os.dup(rootfd)
            try:
                for part in parts:
                    checkpoint()
                    try:
                        child=os.open(part,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW,dir_fd=fd)
                    except OSError as error:
                        missing(error); break
                    os.close(fd); fd=child
                else:
                    walk(fd,0)
            finally:
                os.close(fd)
        checkpoint()
        sample['sampling_completed']=True
    finally:
        os.close(rootfd)
        sample['elapsed_seconds']=time.monotonic()-started
        DEPENDENCY_SAMPLES['samples']+=1
        DEPENDENCY_SAMPLES['enoent_misses']+=sample['enoent_misses']
        DEPENDENCY_SAMPLES['last_samples'].append(sample)
        DEPENDENCY_SAMPLES['last_samples']=DEPENDENCY_SAMPLES['last_samples'][-30:]
        write_json('dependency-budget-samples.json',DEPENDENCY_SAMPLES)
    return sample

def check_storage_health():
    deadline()
    check_rss({})
    st=PRIVATE.lstat()
    require((st.st_dev,st.st_ino)==PRIVATE_ID and stat.S_ISDIR(st.st_mode),'private root identity changed')
    require(shutil.disk_usage(PRIVATE).free>=512*1024**2,'disk low-water 512MiB')

def check_disk_budgets():
    global SCAN_AT
    deadline()
    if time.monotonic()-SCAN_AT<0.5: return
    check_storage_health()
    sample_dependency_budget()
    check_output_budget()
    # Throttle from completion, not start: a long sample must not cause an
    # uninterrupted sequence of full scans. Failed samples never advance it.
    SCAN_AT=time.monotonic()

def check_output_budget():
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
                deadline()
                require(entries<=100000 and size<=256*1024**2,'private output 256MiB/100000 entries budget')
    deadline()

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
    global ACTIVE, TOTAL_LOG_BYTES, MEMORY_PHASE
    MEMORY_PHASE=label
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
    # No active child writes during seals. Sample the separate store once and
    # account node_modules in this complete byte-reading traversal itself.
    # This remains an observation, not a quota or adversarial mutation barrier.
    require(not DEPENDENCY_INSTALL_ACTIVE and ACTIVE is None,'seal requires quiescent owned commands')
    require(root==PRIVATE/'s/node_modules','unexpected dependency seal root')
    check_storage_health(); check_output_budget()
    store=sample_dependency_budget([('store',)])
    require(store is not None and store['sampling_completed'],'store budget sample incomplete')
    entries=store['entries']; regular_bytes=store['regular_bytes']
    def walk_error(error): raise error  # Never silently omit unreadable subtrees.
    root_id=root.lstat(); require(stat.S_ISDIR(root_id.st_mode),'input root not real directory')
    result={'.':{'kind':'directory','mode':stat.S_IMODE(root_id.st_mode),'identity':[root_id.st_dev,root_id.st_ino],'mtime_ns':root_id.st_mtime_ns,'ctime_ns':root_id.st_ctime_ns}}; total=0
    for parent,dirs,files in os.walk(root,followlinks=False,onerror=walk_error):
        check_storage_health()
        require(len(Path(parent).relative_to(root).parts)<=128,
                'dependency metadata depth exceeded128')
        for name in sorted(dirs+files):
            p=Path(parent)/name; rel=str(p.relative_to(root)); st=p.lstat()
            entries+=1
            if stat.S_ISREG(st.st_mode): regular_bytes+=st.st_size
            deadline()
            require(entries<=300000,'dependency metadata sample exceeded 300000 entries')
            require(regular_bytes<=3*1024**3,'private dependency/store 3GiB budget')
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
    check_storage_health(); check_output_budget()
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
    global MEMORY_PHASE
    MEMORY_PHASE=name
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
    global PRIVATE,PRIVATE_ID,EVIDENCE,JOB_DEADLINE,PREP_DEADLINE,FINALIZING,FIXTURE_STARTED,DEPENDENCY_INSTALL_STARTED,DEPENDENCY_INSTALL_ACTIVE
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
        admit_memory_budget()
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
            DEPENDENCY_INSTALL_STARTED=True; DEPENDENCY_INSTALL_ACTIVE=True
            try:
                run('dependencies',[PNPM,'install','--frozen-lockfile','--ignore-scripts','--store-dir',str(PRIVATE/'store')],600,PRIVATE/'s',env)
            finally:
                DEPENDENCY_INSTALL_ACTIVE=False
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
        result.update(dependency_budget_samples=DEPENDENCY_SAMPLES,phase_timings=TIMINGS,commands=RECORDS,peak_sampled_rss=PEAK_RSS,elapsed_seconds=time.monotonic()-started,
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
