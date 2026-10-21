import hashlib,json,os,signal,subprocess,sys,time,shutil
from pathlib import Path
BASE='1e147f9c32f33bef8652f08a049f001d5f920eca'
validation=Path(__file__).resolve().parents[1]
source=Path(os.environ['GITHUB_WORKSPACE'])/'source'
evidence=Path(os.environ['RUNNER_TEMP'])/'desktop-routing-evidence';evidence.mkdir(exist_ok=True)
manifest=json.loads((validation/'validation/manifest.json').read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
assert manifest['base']==BASE
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=source,text=True).strip()==BASE
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=validation,text=True).strip()==os.environ['GITHUB_SHA']
assert os.environ['GITHUB_REF']=='refs/heads/validation/claude-desktop-routing'
assert set(subprocess.check_output(['git','diff','--name-only',BASE,'HEAD'],cwd=validation,text=True).splitlines())==set(manifest['validationFiles'])
patch=validation/'validation/candidate.diff';assert sha(patch)==manifest['patchSha256']
subprocess.run(['git','apply','--check',str(patch)],cwd=source,check=True)
subprocess.run(['git','apply',str(patch)],cwd=source,check=True)
assert set(subprocess.check_output(['git','diff','--name-only'],cwd=source,text=True).splitlines())==set(manifest['productFiles'])
subprocess.run(['git','diff','--check'],cwd=source,check=True)
locks={p:sha(source/p) for p in ['pnpm-lock.yaml','tauri/Cargo.lock']}
identities={'base':BASE,'validation':os.environ['GITHUB_SHA'],'patchSha256':manifest['patchSha256'],'files':{p:sha(source/p) for p in manifest['productFiles']},'locks':locks}
(evidence/'source-identities.json').write_text(json.dumps(identities,indent=2))
(evidence/'candidate.diff').write_bytes(patch.read_bytes())
def members(group):
 # Linux /proc stat: state, ppid, pgrp after the closing command parenthesis.
 # Exclude zombies, which cannot execute and are awaiting their parent/reaper.
 result=[]
 for item in Path('/proc').iterdir():
  if not item.name.isdigit():continue
  try:
   fields=(item/'stat').read_text().rsplit(')',1)[1].split()
   if int(fields[2])==group and fields[0]!='Z':result.append(int(item.name))
  except (FileNotFoundError,ProcessLookupError):pass
 return sorted(result)

def run(name,args,seconds):
 log_path=evidence/(name+'.log')
 with log_path.open('wb') as log:
  process=subprocess.Popen(args,cwd=source,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  reason=None;remaining=None;cleanup_error=None
  try:
   deadline=time.monotonic()+seconds
   while process.poll() is None:
    if time.monotonic()>=deadline:reason='timeout';break
    if log_path.stat().st_size>32*1024*1024:reason='log-limit';break
    time.sleep(.1)
   if log_path.stat().st_size>32*1024*1024:reason=reason or 'log-limit'
   remaining=members(process.pid)
   if remaining:reason=reason or 'surviving-descendants'
  finally:
   # Evidence must be emitted even when cleanup itself fails. A timeout or
   # leaked descendant stays failed after successful cleanup, never becomes green.
   try:
    for sig in (signal.SIGTERM,signal.SIGKILL):
     if process.poll() is not None and not members(process.pid):break
     try:os.killpg(process.pid,sig)
     except ProcessLookupError:pass
     until=time.monotonic()+5
     while (process.poll() is None or members(process.pid)) and time.monotonic()<until:time.sleep(.1)
    remaining=members(process.pid)
   except Exception as error:
    cleanup_error=type(error).__name__+': '+str(error)
   finally:
    code=process.poll()
    (evidence/(name+'-process.json')).write_text(json.dumps({'pid':process.pid,'exitCode':code,'reason':reason,'survivors':remaining,'cleanupError':cleanup_error,'logBytes':log_path.stat().st_size,'logSoftLimit':32*1024*1024}))
  assert cleanup_error is None and code is not None and remaining==[], 'Owned process exit unconfirmed; outer job timeout remains safeguard'
  assert reason is None,(name,reason)
  return code


# Frontend reuse is explicit and byte-checked against the earlier run, not
# represented as a browser execution in this Rust-only run.
assert manifest['frontendEvidenceRun']==39561532802
assert all(sha(source/p)==digest for p,digest in manifest['frontendFileSha256'].items())
assert locks==manifest['previousLocks']
(evidence/'frontend-reuse.json').write_text(json.dumps({'run':manifest['frontendEvidenceRun'],'files':manifest['frontendFileSha256'],'sameBytes':True,'browserExecutedHere':False},indent=2))
try:
 assert run('compile',['cargo','+1.99.1','test','--locked','--manifest-path','tauri/Cargo.toml','--lib','--no-run','--message-format=json'],900)==0
 binaries=[];finished=False
 for line in (evidence/'compile.log').read_text().splitlines():
  try:result=json.loads(line)
  except json.JSONDecodeError:continue
  if result.get('reason')=='build-finished':finished=result.get('success') is True
  if result.get('reason')=='compiler-artifact' and result.get('profile',{}).get('test') and result.get('target',{}).get('name')=='ai_toolbox_lib' and result.get('executable'):binaries.append(result['executable'])
 assert finished and len(binaries)==1
 binary=Path(binaries[0]).resolve(strict=True);assert binary.is_relative_to((source/'tauri/target').resolve(strict=True)) and binary.is_file()
 tests=manifest['rustTests'];assert len(tests)==19 and len(set(tests))==19
 for full in tests:
  name=full.replace('::','_')
  assert run(name+'-list',[str(binary),'--exact',full,'--list'],15)==0
  lines=(evidence/(name+'-list.log')).read_text().splitlines();assert full+': test' in lines and '1 test, 0 benchmarks' in lines
 for full in tests:
  name=full.replace('::','_')
  assert run(name,[str(binary),'--exact',full,'--test-threads=1','--nocapture'],90)==0
  assert 'test result: ok. 1 passed;' in (evidence/(name+'.log')).read_text()

 assert {p:sha(source/p) for p in locks}==locks
 assert {p:sha(source/p) for p in manifest['productFiles']}==identities['files']
 untracked=subprocess.check_output(['git','ls-files','--others','--exclude-standard'],cwd=source,text=True).splitlines()
 (evidence/'untracked.json').write_text(json.dumps(untracked));assert untracked==[]
 (evidence/'tested.diff').write_bytes(subprocess.check_output(['git','diff','--binary'],cwd=source))
 (evidence/'result.json').write_text(json.dumps({'rustExact':19,'frontendEvidenceRun':39561532802,'frontendHashesUnchanged':True,'browserExecutedHere':False,'locksUnchanged':True,'candidateFilesUnchanged':True,'untracked':untracked}))
finally:
 (evidence/'untracked-finally.json').write_text(json.dumps(subprocess.check_output(['git','ls-files','--others','--exclude-standard'],cwd=source,text=True).splitlines()))
