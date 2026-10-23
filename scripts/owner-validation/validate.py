#!/usr/bin/env python3
"""Pure Linux owner primitives; no GUI, database, business writes or installed app."""
import argparse,hashlib,json,os,re,selectors,signal,subprocess,sys,time,tempfile,shutil,stat
from pathlib import Path
LOG_LIMIT=16*1024*1024
TOTAL_LOG_LIMIT=64*1024*1024
RECORDS=[]
TOTAL_LOG_BYTES=0
ACTIVE=None
EVIDENCE=None
PHASE='validation'
JOB_DEADLINE=None
FIXTURE=None
FIXTURE_ID=None
LAST_SCAN=0
BASE='55f70695e0bbd6921cf49b32006ab7fa9556b4f2'
MANIFEST_SHA='73ee4aa5249fa4f317a916b70a062a5d6afeba237b2418cea4167ae22df0b7e9'
def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    result = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def require(condition, reason):
    if not condition:
        raise RuntimeError(reason)


def write_json(name, value):
    destination = EVIDENCE / name
    temporary = destination.with_suffix(destination.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    temporary.replace(destination)


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
    # that outer boundary. Owner parents spawn bounded actors in the inherited
    # session (no setsid/daemon); the separately bounded shell helper starts its
    # own group and waits/reaps it. These known fixtures are not a guarantee
    # against arbitrary unseen escaping descendants.
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
            child.poll()
            if not owned_members(child.pid, known):
                break
            time.sleep(0.05)
    try:
        child.wait(timeout=1)
    except subprocess.TimeoutExpired:
        pass
    return owned_members(child.pid, known)


def interrupted(signum, _frame):
    raise RuntimeError(f"interrupted by signal {signum}")


def run(label, command, timeout, cwd, env, allowed_codes=(0,)):
    """Nonblocking pipe drain plus owned process observation with hard deadlines."""
    global ACTIVE, TOTAL_LOG_BYTES
    require(re.fullmatch(r"[a-z0-9-]+", label), "unsafe log label")
    output_paths = {key: EVIDENCE / f"{PHASE}-{label}.{key}.log" for key in ("stdout", "stderr")}
    require(not any(path.exists() for path in output_paths.values()), "duplicate command label")
    started = time.monotonic()
    requested_timeout = timeout
    if JOB_DEADLINE is not None:
        timeout = min(timeout, JOB_DEADLINE - time.time() - 10)
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
            if FIXTURE is not None:
                check_fixture_budget()
            exit_code = child.poll()
            members = owned_members(child.pid, known)
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
                    record["exit_code"] = child.poll()
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
    require(reason == "completed" and not record["survivors"], f"{label}: {reason}; see command evidence")
    return output_paths["stdout"].read_bytes()


def artifact(output, manifest_path, target_name, target):
    matches = []
    for line in output.decode().splitlines():
        message = json.loads(line)
        if message.get("reason") != "compiler-artifact":
            continue
        if (Path(message["manifest_path"]).resolve() == manifest_path.resolve()
                and message.get("profile", {}).get("test") is True
                and message.get("target", {}).get("name") == target_name
                and set(message.get("target", {}).get("kind", [])) & {"lib", "rlib", "staticlib", "cdylib", "test"}
                and message.get("executable")):
            executable = Path(message["executable"]).resolve(strict=True)
            require(executable.is_relative_to(target) and executable.is_file()
                    and os.access(executable, os.X_OK), "test executable escaped owned target")
            matches.append(executable)
    require(len(matches) == 1, "expected exactly one matching real test artifact")
    return matches[0]


def inventory(output):
    text = output.decode()
    names = sorted(line[:-6] for line in text.splitlines() if line.endswith(": test"))
    require(len(names) == len(set(names)), "duplicate test inventory")
    require(not any(line.endswith(": benchmark") for line in text.splitlines()), "unexpected benchmark inventory")
    return names


def check_pass(output, count, filtered=None):
    text = output.decode()
    pattern = rf"^test result: ok\. {count} passed; 0 failed; 0 ignored; 0 measured; (\d+) filtered out;"
    matches = re.findall(pattern, text, re.M)
    require(len(matches) == 1, "test summary missing, ignored, or wrong pass count")
    if filtered is not None:
        require(int(matches[0]) == filtered, "unexpected filtered test count")


def check_fixture_budget():
    global LAST_SCAN
    if time.monotonic()-LAST_SCAN < .5:return
    LAST_SCAN=time.monotonic()
    root=FIXTURE.lstat();require((root.st_dev,root.st_ino)==FIXTURE_ID,'fixture root identity changed')
    total=0;count=0
    def scan_error(error):
        # TempDir deletion during sampling is normal only below our held root.
        if isinstance(error,FileNotFoundError) and Path(error.filename)!=FIXTURE:return
        raise error
    for current,dirs,files in os.walk(FIXTURE,followlinks=False,onerror=scan_error):
        for n in dirs+files:
            p=Path(current)/n
            try:s=p.lstat()
            except FileNotFoundError:continue
            count+=1
            if stat.S_ISREG(s.st_mode):total+=s.st_size
            require(count<=512 and total<=2*1024*1024,'synthetic fixture storage budget exceeded')
    require(os.statvfs(FIXTURE).f_bavail*os.statvfs(FIXTURE).f_frsize>=4*1024*1024,'runtime filesystem low space')


def safe_env(home):
    env={'PATH':os.environ['PATH'],'LANG':'C.UTF-8','LC_ALL':'C.UTF-8','RUST_BACKTRACE':'0','CARGO_BUILD_JOBS':'2','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0','CARGO_TERM_COLOR':'never','CARGO_HTTP_TIMEOUT':'30','CARGO_NET_RETRY':'1','GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':'/dev/null'}
    env['RUSTUP_HOME']=os.environ.get('RUSTUP_HOME',str(Path(os.environ['HOME'])/'.rustup'))
    for key in ['HOME','USERPROFILE','CARGO_HOME','CARGO_TARGET_DIR','TMPDIR','TMP','TEMP','XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_CACHE_HOME','AI_TOOLBOX_DATA_DIR','CODEX_HOME']:
        p=home/key;p.mkdir();env[key]=str(p)
    return env


def main():
    global EVIDENCE,JOB_DEADLINE,FIXTURE,FIXTURE_ID
    require(sys.platform=='linux' and os.geteuid()!=0,'unprivileged Linux required')
    checkout=Path(os.environ['GITHUB_WORKSPACE']).resolve();rt=Path(os.environ['RUNNER_TEMP']).resolve()
    EVIDENCE=rt/'owner-evidence';EVIDENCE.mkdir();JOB_DEADLINE=time.time()+540
    result={'ok':False,'scope':'12 pure unit plus 11 synthetic parent process tests; not desktop admission or Windows','error':None}
    source=None;env=None
    try:
        require(os.environ.get('GITHUB_REPOSITORY')=='xue-ding-e/AI-Toolbox-Next','wrong repository')
        require(os.environ.get('GITHUB_REF')=='refs/heads/validation/owner-primitives','wrong branch')
        require(os.environ.get('GITHUB_EVENT_NAME')=='workflow_dispatch','manual only')
        head=os.environ['EXPECTED_SHA'];require(re.fullmatch('[0-9a-f]{40}',head) and head==os.environ['GITHUB_SHA'],'exact SHA required')
        envroot=rt/'owner-isolation';envroot.mkdir();env=safe_env(envroot)
        def git(label,args,cwd=checkout):return run(label,['git',*args],15,cwd,env)
        require(git('parents',['rev-list','--parents','-n','1','HEAD']).decode().split()==[head,BASE],'single parent base required')
        require(not git('initial-status',['status','--porcelain']).strip(),'dirty harness checkout')
        scope=git('harness-scope',['diff','--name-only',BASE,'HEAD']).decode().splitlines()
        require(set(scope)=={'.github/workflows/maintenance-validation.yml','scripts/owner-validation/validate.py','scripts/owner-validation/candidate.json','scripts/owner-validation/candidate.patch'},'unexpected harness scope')
        mp=checkout/'scripts/owner-validation/candidate.json';require(file_digest(mp)==MANIFEST_SHA,'manifest hash mismatch');m=json.loads(mp.read_text());require(m['base']==BASE and len(m['files'])==9,'candidate base/scope mismatch')
        pp=mp.with_name('candidate.patch');require(file_digest(pp)==m['patch_sha256'],'patch hash mismatch')
        require(os.statvfs(rt).f_bavail*os.statvfs(rt).f_frsize>=2*1024**3,'build space insufficient')
        source=rt/'owner-source';git('worktree',['worktree','add','--detach',str(source),BASE]);git('apply',['apply','--index','--whitespace=error',str(pp)],source)
        require(git('candidate-diff',['diff','--cached','--binary','--full-index',BASE],source)==pp.read_bytes(),'candidate full patch mismatch')
        before={p:file_digest(source/p) for p in [*m['files'],*m['preserved']]};require(before=={**m['files'],**m['preserved']},'candidate/preserved source hash mismatch');result['before']=before
        lock=source/'crates/ai-toolbox-management/Cargo.lock';result['lock_sha256']=file_digest(lock)
        require(run('rust-version',['rustc','--version'],10,source,env).decode().startswith('rustc 1.99.1 '),'unexpected rustc version');require(run('cargo-version',['cargo','--version'],10,source,env).decode().startswith('cargo 1.99.1 '),'unexpected cargo version')
        manifest=source/'crates/ai-toolbox-management/Cargo.toml'
        output=run('compile',['cargo','test','--manifest-path',str(manifest),'--locked','--jobs','2','--lib','--test','owner_process','--no-run','--message-format=json'],240,source,env)
        require(any(json.loads(l).get('reason')=='build-finished' and json.loads(l).get('success') is True for l in output.decode().splitlines()),'compiler completion missing')
        bins={k:artifact(output,manifest,n,Path(env['CARGO_TARGET_DIR']).resolve()) for k,n in [('lib','ai_toolbox_management'),('owner','owner_process')]}
        for k,b in bins.items():require(inventory(run(k+'-list',[str(b),'--list','--test-threads=1'],15,source,env))==m['inventories'][k],'exact inventory mismatch')
        # Existing /dev/shm must really be tmpfs. No mounts, chmod repairs or fallback.
        parent=Path('/dev/shm');require(parent.is_dir() and not parent.is_symlink(),'runtime tmpfs unavailable')
        fs=run('runtime-filesystem',['stat','-f','-c','%t',str(parent)],5,source,env).decode().strip();require(fs=='1021994','runtime root is not tmpfs')
        require(os.statvfs(parent).f_bavail*os.statvfs(parent).f_frsize>=8*1024*1024,'runtime tmpfs too full')
        FIXTURE=Path(tempfile.mkdtemp(prefix='ao-',dir=parent));s=FIXTURE.lstat();FIXTURE_ID=(s.st_dev,s.st_ino);require(s.st_uid==os.geteuid() and stat.S_IMODE(s.st_mode)==0o700,'fixture identity/mode invalid')
        result['runtime']={'type':fs,'mode':oct(stat.S_IMODE(s.st_mode)),'root':str(FIXTURE),'identity':list(FIXTURE_ID),'budget_bytes':2*1024*1024}
        tenv=env.copy();tenv.update(TMPDIR=str(FIXTURE),TMP=str(FIXTURE),TEMP=str(FIXTURE),CARGO_NET_OFFLINE='true')
        # Resource limit inherited by test actors; never applies to linker output.
        launch=[sys.executable,'-c','import os,resource,sys; resource.setrlimit(resource.RLIMIT_FSIZE,(16384,16384)); os.execv(sys.argv[1],sys.argv[1:])']
        out=run('lib-tests',launch+[str(bins['lib']),'--test-threads=1','--show-output'],60,source,tenv);check_pass(out,12,0)
        out=run('owner-tests',launch+[str(bins['owner']),'--test-threads=1','--show-output'],180,source,tenv);txt=out.decode();require(len(re.findall(r'^test result: ok\. 11 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;',txt,re.M))==1,'owner pass count mismatch')
        require(len(re.findall(r'^verified actor execution: ',txt,re.M))==13,'expected thirteen internally verified actor executions')
        check_fixture_budget();require(not list(FIXTURE.iterdir()),'fixture leaked artifacts despite test success')
        result.update(ok=True,ordinary_passed=23,ignored_actor_entry=1,actual_actor_invocations=13,binaries={k:file_digest(b) for k,b in bins.items()})
    except BaseException as e:result['error']=f'{type(e).__name__}: {e}'
    finally:
        try:
            if source and source.exists() and 'before' in result:
                result['after']={p:file_digest(source/p) for p in result['before']};require(result['after']==result['before'],'source changed during validation')
                run('final-working-diff',['git','diff','--exit-code'],10,source,env)
                final_patch=run('final-staged-diff',['git','diff','--cached','--binary','--full-index',BASE],10,source,env);require(digest(final_patch)==m['patch_sha256'],'staged source changed')
                # No untracked product build artifacts are accepted; target and runtime are outside source.
                status=run('final-untracked',['git','ls-files','--others','--exclude-standard'],10,source,env);require(not status.strip(),'unexpected source files')
            if FIXTURE:
                s=FIXTURE.lstat();require((s.st_dev,s.st_ino)==FIXTURE_ID,'fixture identity changed at cleanup')
                require(all(not r['survivors'] for r in RECORDS),'cleanup cannot remove fixture while survivors unknown')
                shutil.rmtree(FIXTURE);result['runtime_removed']=not FIXTURE.exists();FIXTURE=None
            result['cleanup_confirmed']=all(not r['survivors'] for r in RECORDS)
        except BaseException as e:
            result['ok']=False;result['cleanup_error']=f'{type(e).__name__}: {e}'
        result['commands']=RECORDS;write_json('validation-result.json',result);print(json.dumps(result),flush=True)
    return 0 if result['ok'] else 1

if __name__=='__main__':
    for sig in [signal.SIGTERM,signal.SIGINT]:signal.signal(sig,interrupted)
    sys.exit(main())
