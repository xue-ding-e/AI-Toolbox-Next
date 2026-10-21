import hashlib,json,os,signal,subprocess,sys,time
from pathlib import Path
BASE='7450fc2d19fcd1c50832f2674dc0841b5bc2c43b'
validation=Path(__file__).resolve().parents[1]
source=Path(os.environ['GITHUB_WORKSPACE'])/'source'
evidence=Path(os.environ['RUNNER_TEMP'])/'ssh-race-evidence';evidence.mkdir()
manifest=json.loads((validation/'validation/manifest.json').read_text())
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=source,text=True).strip()==BASE
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=validation,text=True).strip()==os.environ['GITHUB_SHA']
assert os.environ['GITHUB_REF']=='refs/heads/validation/ssh-request-race'
assert set(subprocess.check_output(['git','diff','--name-only',BASE,'HEAD'],cwd=validation,text=True).splitlines())==set(manifest['validationFiles'])
patch=validation/'validation/candidate.diff'
assert hashlib.sha256(patch.read_bytes()).hexdigest()==manifest['patchSha256']
subprocess.run(['git','apply','--check',str(patch)],cwd=source,check=True)
subprocess.run(['git','apply',str(patch)],cwd=source,check=True)
(evidence/'candidate.diff').write_bytes(patch.read_bytes())
(evidence/'source-identities.json').write_text(json.dumps({'base':BASE,'validation':os.environ['GITHUB_SHA'],'patchSha256':manifest['patchSha256']}))
subprocess.run(['git','add','--intent-to-add','scripts/verify-ssh-request-race.mjs','web/test/features/settings/sshRequest'],cwd=source,check=True)
subprocess.run(['git','diff','--check'],cwd=source,check=True)
lock=hashlib.sha256((source/'pnpm-lock.yaml').read_bytes()).hexdigest()

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

assert run('install',['pnpm','install','--frozen-lockfile'],420)==0
component=source/'web/features/settings/components/SSHSyncModal.tsx'
fixed=component.read_bytes()
original=subprocess.check_output(['git','show',BASE+':web/features/settings/components/SSHSyncModal.tsx'],cwd=source)
assert subprocess.check_output(['git','hash-object','--stdin'],input=original,cwd=source).decode().strip()=='17895f6f88a7e075a32b30a86da0857f7bc034d2'
def report(directory):
 files=list(directory.glob('ssh-request-race-*/ssh-request-cases.json'))
 assert len(files)==1,'Missing completed case report, not product RED'
 return json.loads(files[0].read_text())
try:
 component.write_bytes(original)
 red=evidence/'red';red.mkdir()
 assert run('red',['pnpm','run','test:ssh-request-race','--output',str(red)],240)==1
 r=report(red)
 assert len(r['results'])==7 and r['validRed'] and r['fixtureErrors']==0,r
 required={'B result survives late A success','late A rejection preserves pending B spinner'}
 assert required <= {x['name'] for x in r['results'] if x.get('classification')=='product-invariant'},r
finally:component.write_bytes(fixed)
green=evidence/'green';green.mkdir()
assert run('green',['pnpm','run','test:ssh-request-race','--output',str(green)],240)==0
r=report(green);assert len(r['results'])==7 and r['passed']==7 and r['fixtureErrors']==0,r
assert run('tsc',['pnpm','exec','tsc','--noEmit'],240)==0
assert hashlib.sha256((source/'pnpm-lock.yaml').read_bytes()).hexdigest()==lock
# Include only source evidence and bounded diagnostics, never browser profiles.
(evidence/'tested.diff').write_bytes(subprocess.check_output(['git','diff','--binary'],cwd=source))
for folder in [red,green]:
 import shutil
 for child in folder.glob('ssh-request-race-*'):
  for item in child.iterdir():
   if item.name not in {'ssh-request-cases.json','results.json','failure.html','failure.png'}:
    if item.is_dir():shutil.rmtree(item)
    else:item.unlink()
