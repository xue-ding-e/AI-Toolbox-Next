import subprocess,sys,os,json,time,re,tempfile,pathlib
repo,tip,out=sys.argv[1:]
def g(*args,input=None,env=None):
 p=subprocess.run(['git','-C',repo,*args],input=input,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env,timeout=90)
 if p.returncode: raise RuntimeError(f'{args}: {p.stdout.decode(errors="replace")} {p.stderr.decode(errors="replace")}')
 return p.stdout.decode().strip()
def merge(current,c,base):
 p=subprocess.run(['git','-C',repo,'merge-tree','--write-tree','--merge-base='+base,current,c],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=90)
 txt=p.stdout.decode(); lines=txt.splitlines()
 if p.returncode==0:return lines[0]
 stages=re.findall(r'^100644 ([0-9a-f]{40}) ([123])\t(.+)$',txt,re.M)
 rules=json.loads(pathlib.Path(__file__).with_name('resolution_allowlist.json').read_text())
 rule=rules.get(c)
 if rule and stages==[tuple(x) for x in rule['stages']] and txt.count('CONFLICT ')==len(rule['resolved']):
  idx=out+'.resolve-index';env=os.environ.copy();env['GIT_INDEX_FILE']=idx
  g('read-tree',lines[0],env=env)
  for path,blob in rule['resolved'].items():g('update-index','--cacheinfo','100644,'+blob+','+path,env=env)
  result=g('write-tree',env=env)
  with open(out+'.resolutions.jsonl','a') as f:f.write(json.dumps({'source':c,'current':current,'stages':stages,'resolution':rule['reason'],'resolved':rule['resolved'],'result_tree':result})+'\n')
  return result
 raise RuntimeError(txt+p.stderr.decode())
raw=g('rev-list','--parents',tip)
parents={s[0]:s[1:] for l in raw.splitlines() if (s:=l.split())}
order=[]; seen=set()
def visit(c):
 if c in seen:return
 seen.add(c)
 for p in parents[c]:visit(p)
 order.append(c)
sys.setrecursionlimit(20000);visit(tip)
new=None; tree=None; mapping=[]; boundaries=[]; last_mainline=None
firstparents=set(g('rev-list','--first-parent',tip).splitlines())
for i,c in enumerate(order):
 ps=parents[c]; target=g('rev-parse',c+'^{tree}'); msg=g('show','-s','--format=%B',c)
 if not ps: result=target
 elif len(ps)==1:
  pt=g('rev-parse',ps[0]+'^{tree}')
  if tree==pt: result=target
  else:
   try: result=merge(new,c,ps[0])
   except Exception as e:
    open(out+'.blocked.json','w').write(json.dumps({'source':c,'index':i,'current':new,'error':str(e)},indent=2));raise
 else:
  chosen=ps[0]
  if c not in firstparents and last_mainline:
   candidates=[p for p in ps if subprocess.run(['git','-C',repo,'merge-base','--is-ancestor',p,last_mainline],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30).returncode==0]
   if len(candidates)==1:chosen=candidates[0]
  result=target if c in firstparents else merge(new,c,chosen)
  with open(out+'.merge-parents.jsonl','a') as f:f.write(json.dumps({'source':c,'current':new,'result_tree':result,'chosen_parent':chosen,'latest_mainline':last_mainline,'reason':'source mainline tree boundary' if c in firstparents else 'unique original parent already ancestral to latest mainline, otherwise first parent','numstat':g('diff','--numstat',tree,result)})+'\n')
  if c in firstparents and result!=target: raise RuntimeError('Merge boundary mismatch '+c)
  if result==tree:
   mapping.append({'old':c,'new':new,'kind':'redundant-merge','tree':tree})
   if c in firstparents:
    boundaries.append({'old':c,'new':new,'tree':tree});last_mainline=c
   open(out+'.mapping.json','w').write(json.dumps(mapping,indent=2))
   open(out+'.boundaries.json','w').write(json.dumps(boundaries,indent=2))
   continue
  with open(out+'.integrations.jsonl','a') as f:f.write(json.dumps({'source':c,'current':new,'result_tree':result,'kind':'mainline-integration' if c in firstparents else 'merge-integration','numstat':g('diff','--numstat',tree,result)})+'\n')
  msg='chore(history): preserve upstream merge integration\n\n'+msg+'\n\nOriginal merge parents: '+' '.join(ps)
 if c in firstparents and result!=target: raise RuntimeError('Boundary mismatch '+c)
 vals=g('show','-s','--format=%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI',c).split('\0')
 env=os.environ.copy()
 for k,v in zip(['GIT_AUTHOR_NAME','GIT_AUTHOR_EMAIL','GIT_AUTHOR_DATE','GIT_COMMITTER_NAME','GIT_COMMITTER_EMAIL','GIT_COMMITTER_DATE'],vals):env[k]=v
 args=['commit-tree',result]+(['-p',new] if new else [])
 new=g(*args,input=(msg+'\n\nOriginal-Commit: '+c+'\n').encode(),env=env);tree=result
 mapping.append({'old':c,'new':new,'kind':('mainline-integration' if c in firstparents else 'merge-integration') if len(ps)>1 else 'original','tree':tree})
 if c in firstparents:
  boundaries.append({'old':c,'new':new,'tree':tree});last_mainline=c
 g('update-ref','refs/heads/linear-progress',new)
 if i%100==0:print(json.dumps({'processed':i,'total':len(order),'tip':new}),flush=True)
 open(out+'.mapping.json','w').write(json.dumps(mapping,indent=2))
 open(out+'.boundaries.json','w').write(json.dumps(boundaries,indent=2))
g('update-ref','refs/heads/main',new)
print(json.dumps({'complete':True,'tip':new,'tree':tree,'source':tip,'count':g('rev-list','--count',new),'merges':g('rev-list','--count','--merges',new)}),flush=True)
