import hashlib,json,os,signal,subprocess,sys,time,shutil,tempfile
from pathlib import Path
BASE='459b34a65039d93209c02efe10985d91b3caeda2'
validation=Path(__file__).resolve().parents[1]
source=Path(os.environ['GITHUB_WORKSPACE'])/'source'
evidence=Path(os.environ['RUNNER_TEMP'])/'ssh-capability-evidence'
evidence.mkdir(exist_ok=False)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def git(*args,cwd=source):return subprocess.check_output(['git',*args],cwd=cwd)
def save(name,value):(evidence/name).write_text(json.dumps(value,indent=2))
manifest=json.loads((validation/'validation/manifest.json').read_text())
assert manifest['base']==BASE
assert os.environ['GITHUB_REPOSITORY']=='xue-ding-e/AI-Toolbox-Next'
assert os.environ['GITHUB_EVENT_NAME']=='workflow_dispatch'
assert os.environ['GITHUB_REF']=='refs/heads/validation/ssh-capability-report'
assert git('rev-parse','HEAD').decode().strip()==BASE
assert git('rev-parse','HEAD',cwd=validation).decode().strip()==os.environ['GITHUB_SHA']
assert git('show','-s','--format=%P','HEAD',cwd=validation).decode().strip()==BASE, 'Exactly one parent, fixed main'
assert set(git('diff','--name-only',BASE,'HEAD',cwd=validation).decode().splitlines())==set(manifest['validationFiles'])
assert git('status','--porcelain',cwd=validation)==b''
assert git('status','--porcelain')==b''
patch=validation/'validation/candidate.diff'
assert sha(patch)==manifest['patchSha256']
assert sha(Path(__file__))==manifest['validatorSha256']
harness_hashes={p:sha(validation/p) for p in manifest['validationFiles']}
save('harness-identities.json',{'base':BASE,'validationCommit':os.environ['GITHUB_SHA'],'files':harness_hashes,'hashChain':'workflow pins patch, manifest, validator; manifest pins validator and patch; workflow own hash recorded, not self-pinned'})
assert len(manifest['productFiles'])==14 and len(set(manifest['productFiles']))==14
locks={p:sha(source/p) for p in ['pnpm-lock.yaml','tauri/Cargo.lock']}
for p,identity in manifest['identities'].items():
 if identity['baseBlob'] is not None:assert git('rev-parse',BASE+':'+p).decode().strip()==identity['baseBlob']
 else:assert subprocess.run(['git','cat-file','-e',BASE+':'+p],cwd=source,stderr=subprocess.DEVNULL).returncode!=0
subprocess.run(['git','apply','--check','--index',str(patch)],cwd=source,check=True)
subprocess.run(['git','apply','--index',str(patch)],cwd=source,check=True)
def check_source(label):
 assert set(git('diff','HEAD','--name-only').decode().splitlines())==set(manifest['productFiles'])
 assert git('diff','--name-only')==b'', 'Worktree must equal candidate index'
 subprocess.run(['git','diff','--cached','--check'],cwd=source,check=True)
 files={p:sha(source/p) for p in manifest['productFiles']}
 assert files=={p:v['sha256'] for p,v in manifest['identities'].items()}
 assert {p:sha(source/p) for p in locks}==locks
 unknown=git('ls-files','--others','--exclude-standard').decode().splitlines()
 save('untracked-'+label+'.json',unknown);assert unknown==[]
 assert {p:sha(validation/p) for p in manifest['validationFiles']}==harness_hashes
 save('source-identities-'+label+'.json',{'base':BASE,'files':files,'locks':locks,'patchSha256':sha(patch)})
check_source('before')
(evidence/'candidate.diff').write_bytes(patch.read_bytes())
(evidence/'tested.diff').write_bytes(git('diff','--cached','--binary','HEAD'))
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

def run(name,args,seconds,extra_env=None):
 log_path=evidence/(name+'.log')
 with log_path.open('wb') as log:
  process=subprocess.Popen(args,cwd=source,stdout=log,stderr=subprocess.STDOUT,start_new_session=True,env={**os.environ,**(extra_env or {})})
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


component=source/'web/features/settings/components/SSHSyncModal.tsx'
fixed=component.read_bytes()
original=git('show',BASE+':web/features/settings/components/SSHSyncModal.tsx')
assert hashlib.sha256(original).hexdigest()==manifest['originalModalSha256']
baseline=evidence/'baseline';baseline.mkdir()
full=evidence/'full';full.mkdir()
def read_report(parent,name):
 files=list(parent.glob('ssh-capability-*/'+name));assert len(files)==1,(name,files)
 return json.loads(files[0].read_text()),files[0].parent
def screenshots(results,folder,expected):
 entries=[r['evidence'] for r in results if r.get('evidence')]
 assert len(entries)==expected
 paths=[]
 for item in entries:
  assert item['diagnosticErrors']==[]
  assert len(item['files'])==1
  name=item['files'][0];assert Path(name).name==name and name.endswith('.png')
  path=folder/name
  assert path.is_file() and not path.is_symlink() and path.stat().st_size>8
  assert path.read_bytes().startswith(b'\x89PNG\r\n\x1a\n')
  paths.append(str(path.relative_to(evidence)))
 assert len(set(paths))==expected
 return paths
def safety(results):
 for row in results:
  assert row['passed'] is True
  assert row.get('pageExceptions',[])==[]
  observations=row['observations'];assert observations
  assert all(o['state']['forbidden']==0 for o in observations)
  if 'state' in row:assert row['state']['forbidden']==0
try:
 assert run('install',['pnpm','install','--frozen-lockfile'],420)==0
 check_source('after-install')
 try:
  component.write_bytes(original)
  assert {p:sha(source/p) for p in manifest['productFiles'] if source/p!=component}=={p:v['sha256'] for p,v in manifest['identities'].items() if source/p!=component}
  assert run('baseline',['pnpm','run','test:ssh-capability','--output',str(baseline)],300,{'SSH_CAPABILITY_CHECK_MODE':'baseline'})==0
  report,folder=read_report(baseline,'ssh-capability-baseline-cases.json')
  assert report['syntheticBoundaryOnly'] is True
  rows=report['results'];assert len(rows)==2
  assert {r['name'] for r in rows}=={'baseline-unknown','baseline-missingHome'}
  safety(rows)
  for row in rows:
   assert row['classification']=='missing-new-capability-ui'
   assert row['newFeatureExpectation']=={'expected':1,'actual':0}
   assert row['existingAuthControlsPassed'] is True and row['syntheticBoundaryOnly'] is True
  common,_=read_report(baseline,'results.json')
  assert common['browserExceptions']==[] and common['checks']==rows
  baseline_images=screenshots(rows,folder,2)
 finally:component.write_bytes(fixed)
 check_source('restored-baseline')
 assert run('full',['pnpm','run','test:ssh-capability','--output',str(full)],600,{'SSH_CAPABILITY_CHECK_MODE':'full'})==0
 race,_=read_report(full,'ssh-request-cases.json')
 assert len(race['results'])==7 and race['passed']==7 and race['fixtureErrors']==0 and race['productInvariantFailures']==0
 safety(race['results'])
 capability,folder=read_report(full,'ssh-capability-cases.json')
 assert capability['syntheticBoundaryOnly'] is True and len(capability['results'])==48
 safety(capability['results'])
 common,_=read_report(full,'results.json')
 assert common['browserExceptions']==[] and len(common['checks'])==55
 assert common['checks']==race['results']+capability['results']
 full_images=screenshots(capability['results'],folder,4)
 save('screenshots.json',{'baseline':baseline_images,'representative':full_images,'diagnosticErrors':0,'manualVisualReview':'pending'})
 assert run('tsc',['pnpm','exec','tsc','--noEmit'],240)==0
 target=source/'tauri/target'
 assert Path(os.environ['CARGO_TARGET_DIR']).resolve()==target.resolve()
 assert not target.exists(), 'Fresh controlled build directory required'
 assert os.environ['CARGO_BUILD_JOBS']=='2'
 assert run('compile',['cargo','+1.99.1','test','--locked','--manifest-path','tauri/Cargo.toml','--lib','--no-run','--message-format=json'],900)==0
 binaries=[];finished=[]
 for line in (evidence/'compile.log').read_text().splitlines():
  try:record=json.loads(line)
  except json.JSONDecodeError:continue
  if record.get('reason')=='build-finished':finished.append(record.get('success'))
  if record.get('reason')=='compiler-artifact' and record.get('profile',{}).get('test') and record.get('target',{}).get('name')=='ai_toolbox_lib' and record.get('executable'):
   assert set(record['target']['kind'])=={'staticlib','cdylib','rlib'}
   assert Path(record['target']['src_path']).resolve()==(source/'tauri/src/lib.rs').resolve()
   assert Path(record['manifest_path']).resolve()==(source/'tauri/Cargo.toml').resolve()
   binaries.append(record['executable'])
 assert finished==[True] and len(binaries)==1
 binary=Path(binaries[0]).resolve(strict=True)
 assert binary.is_relative_to(target.resolve(strict=True)) and binary.is_file() and os.access(binary,os.X_OK)
 save('rust-binary.json',{'path':str(binary),'sha256':sha(binary),'target':'ai_toolbox_lib'})
 tests=manifest['rustTests'];assert len(tests)==24 and len(set(tests))==24
 for name in tests:
  stem=name.replace('::','_')
  assert run(stem+'-list',[str(binary),'--exact',name,'--list'],15)==0
  lines=(evidence/(stem+'-list.log')).read_text().splitlines()
  assert [x for x in lines if x.endswith(': test')]==[name+': test'] and '1 test, 0 benchmarks' in lines
 for name in tests:
  stem=name.replace('::','_')
  assert run(stem,[str(binary),'--exact',name,'--test-threads=1','--nocapture'],90)==0
  log=(evidence/(stem+'.log')).read_text()
  assert 'test '+name+' ... ok' in log and 'test result: ok. 1 passed; 0 failed;' in log
 check_source('after')
 assert (evidence/'tested.diff').read_bytes()==git('diff','--cached','--binary','HEAD')
 save('result.json',{'baselineContractAbsenceVerified':2,'baselineIsProductRed':False,'syntheticBoundaryOnly':True,'racePassed':7,'newCapabilityPassed':48,'browserPassed':55,'rustExactPassed':24,'fullTsc':True,'candidateAndLocksUnchanged':True,'manualVisualReview':'pending'})
finally:
 component.write_bytes(fixed)
 try:check_source('finally')
 finally:
  # Only children of directories created exclusively by this invocation.
  # Preserve bounded evidence, never upload source bundles or browser profiles.
  for parent in [baseline,full]:
   for folder in parent.iterdir():
    assert folder.is_dir() and not folder.is_symlink() and folder.name.startswith('ssh-capability-')
    for item in folder.iterdir():
     allowed=item.is_file() and not item.is_symlink() and (item.name in {'ssh-request-cases.json','ssh-capability-cases.json','ssh-capability-baseline-cases.json','results.json','failure.html','failure.png'} or (item.name.startswith(('case-','capability-','baseline-')) and item.suffix in {'.html','.png'}))
     if not allowed:
      if item.is_dir() and not item.is_symlink():shutil.rmtree(item)
      else:item.unlink()
