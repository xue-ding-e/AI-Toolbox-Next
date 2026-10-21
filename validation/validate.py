import hashlib,json,os,signal,subprocess,sys,time,shutil
from pathlib import Path
BASE='4bbbc8c477f0cfc1fe61080f1161f1b9b8882a83'
validation=Path(__file__).resolve().parents[1]
source=Path(os.environ['GITHUB_WORKSPACE'])/'source'
evidence=Path(os.environ['RUNNER_TEMP'])/'grok-fetch-evidence';evidence.mkdir(exist_ok=True)
manifest=json.loads((validation/'validation/manifest.json').read_text())
assert manifest['base']==BASE
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=source,text=True).strip()==BASE
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=validation,text=True).strip()==os.environ['GITHUB_SHA']
assert os.environ['GITHUB_REF']=='refs/heads/validation/grok-fetched-models'
assert set(subprocess.check_output(['git','diff','--name-only',BASE,'HEAD'],cwd=validation,text=True).splitlines())==set(manifest['validationFiles'])
patch=validation/'validation/candidate.diff'
assert hashlib.sha256(patch.read_bytes()).hexdigest()==manifest['patchSha256']
subprocess.run(['git','apply','--check',str(patch)],cwd=source,check=True)
subprocess.run(['git','apply',str(patch)],cwd=source,check=True)
subprocess.run(['git','add','-N','--',*manifest['newFiles']],cwd=source,check=True)
assert set(subprocess.check_output(['git','diff','--name-only'],cwd=source,text=True).splitlines())==set(manifest['productFiles'])
subprocess.run(['git','diff','--check'],cwd=source,check=True)
# No rustfmt: exact reviewed bytes, no unrelated formatting of old code.
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
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



# Validation-only source maps; restore the existing runner before product hash checks.
runner=source/'scripts/lib/browser-fixture.mjs'
runner_original=runner.read_bytes()
old=b"outDir: path.join(artifactRoot, 'dist'),"
assert runner_original.count(old)==1
runner.write_bytes(runner_original.replace(old,old+b' sourcemap: true,'))
(evidence/'diagnostic-runner.sha256').write_text(sha(runner))
folders=[]
component=source/'web/features/coding/grok/pages/GrokPage.tsx'
fixed=component.read_bytes()
original=subprocess.check_output(['git','show',BASE+':web/features/coding/grok/pages/GrokPage.tsx'],cwd=source)
assert subprocess.check_output(['git','hash-object','--stdin'],input=original,cwd=source).decode().strip()=='6a3de4a0ee61a6315edda26efd9e67f097148a8f'
def report(folder):
 files=list(folder.glob('grok-fetched-models-*/grok-fetched-cases.json'));assert len(files)==1,'Missing completed case report, not product RED'
 return json.loads(files[0].read_text())
try:
 assert run('install',['pnpm','install','--frozen-lockfile'],420)==0
 red=evidence/'red';red.mkdir();folders.append(red)
 try:
  component.write_bytes(original)
  assert run('red',['pnpm','run','test:grok-fetched-models','--output',str(red)],300)==1
  data=report(red)
  assert len(data['results'])==4 and data['validRed'] and data['passed']==1 and data['failed']==3 and data['fixtureErrors']==0,data
 finally:component.write_bytes(fixed)
 green=evidence/'green';green.mkdir();folders.append(green)
 assert run('green',['pnpm','run','test:grok-fetched-models','--output',str(green)],300)==0
 data=report(green);assert len(data['results'])==4 and data['passed']==4 and data['failed']==0 and data['fixtureErrors']==0,data
 assert run('unit',['node','--import','./scripts/register-node-ts-extension-loader.mjs','--test','./web/test/features/coding/grok/utils/grokFetchedModels.test.ts'],90)==0
 unit=(evidence/'unit.log').read_text();assert 'tests 8' in unit and 'pass 8' in unit and 'fail 0' in unit,unit
 assert run('tsc',['pnpm','exec','tsc','--noEmit'],240)==0
 runner.write_bytes(runner_original)
 assert {p:sha(source/p) for p in locks}==locks
 assert {p:sha(source/p) for p in manifest['productFiles']}==identities['files']
 assert set(subprocess.check_output(['git','diff','--name-only'],cwd=source,text=True).splitlines())==set(manifest['productFiles'])
 subprocess.run(['git','diff','--check'],cwd=source,check=True)
 untracked=subprocess.check_output(['git','ls-files','--others','--exclude-standard'],cwd=source,text=True).splitlines()
 (evidence/'untracked.json').write_text(json.dumps(untracked));assert untracked==[]
 (evidence/'tested.diff').write_bytes(subprocess.check_output(['git','diff','--binary'],cwd=source))
 (evidence/'result.json').write_text(json.dumps({'redExpectedFailures':3,'redControlsPassed':1,'greenPassed':4,'unitPassed':8,'fullTsc':True,'candidateFilesUnchangedAfterTests':True,'locksUnchanged':True,'trackedDiffScopeVerified':True,'untrackedNonIgnored':untracked}))
finally:
 runner.write_bytes(runner_original)
 for directory in folders:
  for folder in directory.glob('grok-fetched-models-*'):
   for item in folder.iterdir():
    if item.name=='dist':
     assets=item/'assets'
     if assets.exists():
      for asset in assets.iterdir():
       if asset.is_file() and (asset.name.endswith('.js') or asset.name.endswith('.js.map')):shutil.copy2(asset,folder/asset.name)
    allowed=item.name in {'grok-fetched-cases.json','results.json','failure.html','failure.png'} or (item.name.startswith('case-') and (item.name.endswith('-failure.html') or item.name.endswith('-failure.png')))
    if not allowed and not (item.is_file() and (item.name.endswith('.js') or item.name.endswith('.js.map'))):
     if item.is_dir():shutil.rmtree(item)
     else:item.unlink()
