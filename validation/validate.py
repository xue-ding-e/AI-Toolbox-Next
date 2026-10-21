import hashlib,json,os,signal,subprocess,sys,time,shutil
from pathlib import Path
BASE='268a567b93fecfd4c5f889a13b6d1c56f86e3c92'
validation=Path(__file__).resolve().parents[1]
source=Path(os.environ['GITHUB_WORKSPACE'])/'source'
evidence=Path(os.environ['RUNNER_TEMP'])/'skills-group-evidence';evidence.mkdir(exist_ok=True)
manifest=json.loads((validation/'validation/manifest.json').read_text())
assert manifest['base']==BASE
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=source,text=True).strip()==BASE
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=validation,text=True).strip()==os.environ['GITHUB_SHA']
assert os.environ['GITHUB_REF']=='refs/heads/validation/skills-group-mode'
assert set(subprocess.check_output(['git','diff','--name-only',BASE,'HEAD'],cwd=validation,text=True).splitlines())==set(manifest['validationFiles'])
patch=validation/'validation/candidate.diff'
assert hashlib.sha256(patch.read_bytes()).hexdigest()==manifest['patchSha256']
subprocess.run(['git','apply','--check',str(patch)],cwd=source,check=True)
subprocess.run(['git','apply',str(patch)],cwd=source,check=True)
subprocess.run(['git','add','--intent-to-add','scripts/verify-skills-group-mode.mjs','web/test/features/coding/skills/groupMode'],cwd=source,check=True)
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


browser=evidence/'browser';browser.mkdir()
try:
 assert run('install',['pnpm','install','--frozen-lockfile'],420)==0
 assert run('browser',['pnpm','run','test:skills-group-mode','--output',str(browser)],300)==0
 reports=list(browser.glob('skills-group-mode-*/skills-group-cases.json'));assert len(reports)==1
 report=json.loads(reports[0].read_text());assert len(report['results'])==9 and report['passed']==9 and report['failed']==0,report
 assert run('tsc',['pnpm','exec','tsc','--noEmit'],240)==0
 assert run('grouping-unit',['node','--import','./scripts/register-node-ts-extension-loader.mjs','--test','web/test/features/coding/skills/utils/skillGrouping.test.ts'],90)==0
 unit=(evidence/'grouping-unit.log').read_text();assert '# tests 14' in unit and '# pass 14' in unit and '# fail 0' in unit
 # Existing debug lib test entry, not release/custom-protocol or app startup.
 assert run('compile',['cargo','+1.99.1','test','--locked','--manifest-path','tauri/Cargo.toml','--lib','--no-run','--message-format=json'],900)==0
 binaries=[];finished=False
 for line in (evidence/'compile.log').read_text().splitlines():
  try:entry=json.loads(line)
  except json.JSONDecodeError:continue
  if entry.get('reason')=='build-finished':finished=entry.get('success') is True
  if entry.get('reason')=='compiler-artifact' and entry.get('profile',{}).get('test') and entry.get('target',{}).get('name')=='ai_toolbox_lib' and entry.get('executable'):binaries.append(entry['executable'])
 assert finished and len(binaries)==1,binaries
 binary=Path(binaries[0]).resolve(strict=True);target=(source/'tauri/target').resolve(strict=True)
 assert binary.is_relative_to(target) and binary.is_file()
 tests=['coding::skills::adapter::tests::skill_preferences_group_mode_defaults_to_auto', 'coding::skills::adapter::tests::skill_preferences_group_mode_keeps_explicit_choices', 'coding::skills::adapter::tests::skill_preferences_payload_normalizes_group_mode', 'coding::skills::skill_store::tests::group_mode_setting_round_trips_and_normalizes', 'coding::skills::skill_store::tests::group_mode_preserves_other_preferences_after_reopen', 'coding::skills::adapter::tests::skill_preferences_payload_normalizes_default_view_mode', 'coding::skills::skill_store::tests::legacy_group_migration_preserves_source_metadata_and_survives_reopen', 'coding::skills::skill_store::tests::legacy_group_migration_reuses_existing_case_insensitive_registry', 'coding::skills::skill_store::tests::legacy_group_migration_does_not_invent_custom_groups_for_repository_sources', 'coding::skills::skill_store::tests::non_path_setting_write_does_not_create_central_repo_path', 'coding::skills::skill_store::tests::non_path_setting_write_removes_legacy_central_repo_path']
 assert len(tests)==11 and len(set(tests))==11
 (evidence/'rust-inventory.json').write_text(json.dumps(tests,indent=2))
 for full in tests:
  name=full.replace('::','_')
  assert run(name+'-list',[str(binary),'--exact',full,'--list'],15)==0
  lines=(evidence/(name+'-list.log')).read_text().splitlines()
  assert full+': test' in lines and '1 test, 0 benchmarks' in lines,lines
 for full in tests:
  name=full.replace('::','_')
  assert run(name,[str(binary),'--exact',full,'--test-threads=1','--nocapture'],90)==0
  assert 'test result: ok. 1 passed;' in (evidence/(name+'.log')).read_text()
 assert {p:sha(source/p) for p in locks}==locks
 assert {p:sha(source/p) for p in manifest['productFiles']}==identities['files']
 (evidence/'tested.diff').write_bytes(subprocess.check_output(['git','diff','--binary'],cwd=source))
 (evidence/'result.json').write_text(json.dumps({'browser':9,'groupingUnit':14,'rustExact':11,'fullTsc':True,'locksUnchanged':True,'sourceUnchangedAfterTests':True}))
finally:
 # Never upload browser profiles or arbitrary fixture directories, even on failure.
 for folder in browser.glob('skills-group-mode-*'):
  for item in folder.iterdir():
   allowed=item.name in {'skills-group-cases.json','results.json','failure.html','failure.png'} or (item.name.startswith('case-') and (item.name.endswith('-failure.html') or item.name.endswith('-failure.png')))
   if not allowed:
    if item.is_dir():shutil.rmtree(item)
    else:item.unlink()
