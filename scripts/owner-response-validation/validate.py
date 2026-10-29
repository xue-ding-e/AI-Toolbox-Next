#!/usr/bin/env python3
"""Review-only gate: seven synthetic response cases plus two named controls.
Only a test overlay is applied; no Tauri, product change, lock resolution,
real profile, retry, release or business-readiness claim is allowed.
"""
import hashlib, json, os, re, shutil, signal, stat, sys, tempfile, time
from pathlib import Path
sys.dont_write_bytecode = True
import supervisor as s
BASE = 'bfc6acd74c34d462c957956737d69960f6454f7b'
PATCH = '7573822dbe6fdb73d07397a33182b2050b1b1dd19316710224dc5b0dad6a62b7'
ORIGINAL_BLOB = '479c8490ad2e794387ea58764947e8f4667f9217'
CANDIDATE = '725cd14be503cb995031ba30a2d7564f75c649f8ce12282082d852c06083dca1'
REPO = 'xue-ding-e/AI-Toolbox-Next'
REF = 'refs/heads/validation/owner-status-response-20261029'
PREFIX = 'scripts/owner-response-validation/'
TARGET = 'crates/ai-toolbox-automation/tests/owner_console.rs'
MANIFEST = 'crates/ai-toolbox-automation/Cargo.toml'
TESTS = [
 'actual_console_status_response_binding_and_redaction_after_valid_hello',
 'actual_console_reads_real_owner_without_touching_database_or_lease',
 'incompatible_protocol_profile_and_correlation_are_rejected_without_raw_echo',
]
CONTROL = {'.github/workflows/maintenance-validation.yml', *(PREFIX+n for n in ['validate.py','supervisor.py','candidate.patch','seal.json'])}
def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def require(ok,msg): s.require(ok,msg)
def snapshot(root):
    # Track file bytes, executable/mode bits and symlink targets, not just
    # regular files. Do not traverse or read symlink targets outside the tree.
    result={}
    for p in sorted(root.rglob('*')):
        if '.git' in p.relative_to(root).parts: continue
        st=p.lstat(); name=str(p.relative_to(root)); mode=stat.S_IMODE(st.st_mode)
        if stat.S_ISLNK(st.st_mode): result[name]=['link',mode,os.readlink(p)]
        elif stat.S_ISREG(st.st_mode): result[name]=['file',mode,digest(p)]
        elif stat.S_ISDIR(st.st_mode): result[name]=['dir',mode]
        else: raise RuntimeError('unexpected source object: '+name)
    return result
def main():
    checkout = Path.cwd().resolve()
    evidence = Path(os.environ['RUNNER_TEMP'])/'owner-response-evidence'
    evidence.mkdir(mode=0o700,exist_ok=True)
    s.EVIDENCE=evidence
    s.PHASE="preflight" if sys.argv[1:] == ["--preflight"] else "validation"
    start=float((evidence/'started').read_text())
    remaining=900-(time.time()-start)
    require(0<remaining<=900,'shared 15-minute budget expired/clock invalid')
    s.JOB_DEADLINE=time.monotonic()+remaining
    for sig in [signal.SIGTERM,signal.SIGINT]: signal.signal(sig,s.interrupted)
    head=os.environ.get('GITHUB_SHA','')
    require(os.environ.get('GITHUB_REPOSITORY')==REPO and os.environ.get('GITHUB_REF')==REF
            and os.environ.get('GITHUB_EVENT_NAME')=='workflow_dispatch','repo/ref/event mismatch')
    require(re.fullmatch('[a-f0-9]{40}',head) and head==os.environ.get('EXPECTED_SHA'),'control SHA mismatch')
    # Inputs pin the reviewed control; its parent pins all source bytes.
    env={'PATH':os.environ['PATH'],'LANG':'C.UTF-8','LC_ALL':'C.UTF-8','GIT_CONFIG_NOSYSTEM':'1'}
    def run(label,args,seconds=20,cwd=checkout,env_=None):
        return s.run(label,args,seconds,cwd,env if env_ is None else env_)
    def git(label,args): return run(label,['git',*args]).decode().strip()
    require(git('head',['rev-parse','HEAD'])==head,'actual HEAD mismatch')
    require(git('parent',['rev-list','--parents','-n','1','HEAD']).split()==[head,BASE],'exact single parent required')
    require(set(git('control-paths',['diff','--name-only',BASE,head]).splitlines())==CONTROL,'control scope mismatch')
    require(not git('clean',['status','--porcelain']),'dirty checkout')
    seal=json.loads((checkout/PREFIX/'seal.json').read_text())
    for name,value in seal.items():
        require(name in CONTROL and digest(checkout/name)==value,'sealed control differs: '+name)
    require(set(seal)==CONTROL-{PREFIX+'seal.json'},'seal scope mismatch')
    require(digest(checkout/PREFIX/'candidate.patch')==PATCH,'patch hash mismatch')
    require(git('original-blob',['rev-parse',BASE+':'+TARGET])==ORIGINAL_BLOB,'source test changed')
    if sys.argv[1:] == ['--preflight']:
        s.write_json('preflight.json',{'ok':True,'source':BASE,'control':head,'patch':PATCH})
        return
    require(sys.argv[1:]==['--validate'],'unknown phase')
    root=Path(tempfile.mkdtemp(prefix='owner-response-',dir=os.environ['RUNNER_TEMP']))
    os.chmod(root,0o700)
    source=root/'source'; cache=root/'cargo'; target=root/'target'; home=root/'home'
    for p in [cache,target,home]: p.mkdir(mode=0o700)
    fixture=Path(tempfile.mkdtemp(prefix='aor-',dir='/dev/shm')); os.chmod(fixture,0o700)
    require(len(str(fixture))<35 and stat.S_IMODE(fixture.stat().st_mode)==0o700,'fixture root invalid')
    s.FIXTURE=fixture
    safe=False; passed=False
    def limits():
        total=0; count=0
        for p in root.rglob('*'):
            if p.is_file() and not p.is_symlink(): total+=p.stat().st_size; count+=1
        require(total<=4*1024**3 and count<=150000,'build/cache disk budget exceeded')
    def fixture_limits():
        total=sum(p.stat().st_size for p in fixture.rglob('*') if p.is_file() and not p.is_symlink())
        require(total<=32*1024**2,'fixture disk budget exceeded')
    # Throttle large disk scans, while process and output bounds remain continuous.
    last=[0.0]
    def throttled():
        if time.monotonic()-last[0]>3: limits(); last[0]=time.monotonic()
    s.check_disk_budgets=throttled; s.check_fixture_budget=fixture_limits
    env.update(HOME=str(home),CARGO_HOME=str(cache),CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='1',
               CARGO_INCREMENTAL='0',CARGO_NET_RETRY='0',CARGO_HTTP_TIMEOUT='30',TMPDIR=str(fixture),
               RUSTUP_HOME=os.environ['RUSTUP_HOME'],RUSTUP_TOOLCHAIN='1.99.1')
    try:
        run('source',['git','worktree','add','--detach',str(source),BASE])
        before=snapshot(source)
        run('patch-check',['git','apply','--check',str(checkout/PREFIX/'candidate.patch')],cwd=source)
        run('patch-apply',['git','apply',str(checkout/PREFIX/'candidate.patch')],cwd=source)
        require(digest(source/TARGET)==CANDIDATE,'candidate content mismatch')
        applied=snapshot(source)
        require({p for p in before.keys()|applied.keys() if before.get(p)!=applied.get(p)}=={TARGET},'overlay scope mismatch')
        require(run('rustc',['rustc','--version']).decode().startswith('rustc 1.99.1 '),'rust version drift')
        require(run('cargo',['cargo','--version']).decode().startswith('cargo 1.99.1 '),'cargo version drift')
        # Compilation emits the real integration binary and Cargo's real CLI path.
        output=run('compile',['cargo','test','--locked','--manifest-path',MANIFEST,'--test','owner_console',
                             '--no-run','--message-format=json'],480,cwd=source)
        messages=[json.loads(line) for line in output.splitlines() if line.startswith(b'{')]
        require(sum(m.get('reason')=='build-finished' and m.get('success') is True for m in messages)==1,'compile success missing')
        artifacts=[m for m in messages if m.get('reason')=='compiler-artifact' and m.get('target',{}).get('name')=='owner_console' and m.get('executable')]
        require(len(artifacts)==1,'unique integration executable missing')
        binary=Path(artifacts[0]['executable']).resolve(); require(binary.is_relative_to(target),'binary outside isolated target')
        for index,name in enumerate(TESTS):
            listed=run(f'list-{index}',[str(binary),name,'--exact','--list'],15,cwd=source).decode()
            require(re.findall(r'^(.+): test$',listed,re.M)==[name],'zero/extra selected tests')
            ignored=run(f'ignored-{index}',[str(binary),name,'--exact','--ignored','--list'],15,cwd=source).decode()
            require(not re.findall(r'^(.+): test$',ignored,re.M),'selected test ignored')
            result=run(f'test-{index}',[str(binary),name,'--exact','--test-threads=1'],120 if index==0 else 30,cwd=source).decode()
            require(re.search(r'^test '+re.escape(name)+r' \.\.\. ok$',result,re.M) and
                    re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;',result),'libtest result mismatch')
            require(not list(fixture.iterdir()),'fixture did not clean its root')
        require(snapshot(source)==applied,'source/lock bytes changed during validation')
        status=run('final-status',['git','status','--porcelain'],cwd=source).decode()
        require(status==' M '+TARGET+'\n','unexpected final source status')
        require(time.monotonic()<s.JOB_DEADLINE-10,"shared budget reached before final success")
        passed=True
    finally:
        safe=all(not r['survivors'] for r in s.RECORDS)
        empty=not list(fixture.iterdir())
        # Never recursively remove retained or uncertain fixtures. Fresh source,
        # compiler/cache and failure evidence stay on disposable runner disk.
        if safe and empty: fixture.rmdir()
        s.write_json('result.json',{'passed':passed,'source':BASE,'control':head,'patch':PATCH,
                     'tests':TESTS,'response_cases':7,'test_functions':3,'owned_processes_exited':safe,
                     'fixture_empty':empty,'fixture_removed':not fixture.exists(),
                     'source_unchanged':passed,'product_bug_claim':False,
                     'limits':'Linux synthetic CLI only; no status-stage stall, native GUI, Windows, DB readiness or writer admission'})
if __name__=='__main__':
    try: main()
    except BaseException as e:
        if s.EVIDENCE: s.write_json('error.json',{'error':type(e).__name__+': '+str(e),'passed':False})
        raise
