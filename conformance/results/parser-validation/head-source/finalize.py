from pathlib import Path
import subprocess,json,re,hashlib,tomllib
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-tree-head-source-locality'
def sha(x):return hashlib.sha256(x).hexdigest()
def git(*a):return subprocess.check_output(['git','-C',repo,*a])
head=git('rev-parse','6f84a51').decode().strip();base=git('rev-parse','e7124f9').decode().strip()
verified={}
for directory in ['source','before']:
 manifest=json.loads((r/(directory+'-manifest.json')).read_text());n=0
 for f in manifest['files']:
  if f['path'] in ['Cargo.toml','Cargo.lock']:continue
  assert sha((r/directory/f['path']).read_bytes())==f['sha256'],(directory,f['path']);n+=1
 verified[directory]=n
 original=tomllib.loads(git('show',manifest['commit']+':Cargo.lock').decode());lookup={(p['name'],p['version']):p for p in original['package'] if 'source' in p}
 for p in tomllib.loads((r/directory/'Cargo.lock').read_text())['package']:
  if 'source' in p:assert p==lookup[(p['name'],p['version'])]
for p in ['tests/selection.rs','tests/foreign.rs','tests/parse/support.rs']:
 assert (r/'source/probe'/p).read_bytes()==(r/'before/probe'/p).read_bytes()
 dest=r/'probe-files'/p;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes((r/'source/probe'/p).read_bytes())
for name in ['Cargo.toml','Cargo.lock']:(r/('review-'+name)).write_bytes((r/'source'/name).read_bytes())
def extract(mode):
 s=(r/(mode+'.log')).read_text(encoding='utf-8')
 errors=re.findall(r'(?:FOREIGN_ERROR|ERROR) [^\r\n]+',s)
 costs={ (int(n),int(pos)):{key:int(value) for key,value in re.findall(r'(\w+): (\d+)',usage)} for n,pos,usage in re.findall(r'COST count=(\d+) position=(\d+) Usage \{([^}]+)',s)}
 stops={int(cap):(result,int(work)) for cap,result,work in re.findall(r'STOP cap=(\d+) result=(.*?) work=(\d+)',s)}
 return errors,costs,stops
a,old,oldstop=extract('before-native');b,new,newstop=extract('probe-native');c,wasi,wasistop=extract('probe-wasi')
assert a==b==c and len(a)==7
perf=[]
for key,v in old.items():
 n,pos=key;after=new[key];assert v['work']-after['work']==(17*(n-1)*52 if pos==0 else 0)
 assert {k:x for k,x in v.items() if k!='work'}=={k:x for k,x in after.items() if k!='work'}
 assert after['work']==wasi[key]['work']
 perf.append(dict(sources=n,position=pos,before_work=v['work'],after_work=after['work'],allocation_unchanged=True))
assert oldstop.keys()==newstop.keys()==wasistop.keys()
stopchanges=[]
for cap,(outcome,work) in oldstop.items():
 assert outcome==newstop[cap][0]==wasistop[cap][0]
 assert newstop[cap]==wasistop[cap]
 if work!=newstop[cap][1]:stopchanges.append(dict(cap=cap,before_charged_work=work,after_charged_work=newstop[cap][1],outcome=outcome))
tests={}
for mode in ['managed-native','managed-wasi','probe-native','probe-wasi','before-native']:
 assert json.loads((r/(mode+'.json')).read_text())['exit_code']==0
 tests[mode]=sum(map(int,re.findall(r'test result: ok\. (\d+) passed;', (r/(mode+'.log')).read_text())))
(r/'comparison.json').write_text(json.dumps(dict(head=head,base=base,tests=tests,typed_error_cases_equal=7,common_stop_caps=len(oldstop),stopped_work_granularity_changes=stopchanges,source_bodies_verified=verified,external_lock_versions_checksums_equal=True,probe_both_revisions_exact=True,performance=perf),indent=2)+'\n',encoding='utf-8')
delta=[]
for p in git('diff','--name-only',base,head).decode().splitlines():
 data=git('show',head+':'+p);delta.append(dict(path=p,sha256=sha(data),bytes=len(data)))
(r/'delta-hashes.json').write_text(json.dumps(delta,indent=2)+'\n',encoding='utf-8')
(r/'versions.json').write_text(json.dumps({cmd:subprocess.check_output([cmd,'--version']).decode().strip() for cmd in ['rustc','cargo','wasmtime']},indent=2)+'\n',encoding='utf-8')
files=[]
for p in sorted(r.iterdir()):
 if p.is_file() and p.name!='manifest.json':files.append(dict(path=p.name,sha256=sha(p.read_bytes()),bytes=p.stat().st_size))
for p in sorted((r/'probe-files').rglob('*')):
 if p.is_file():files.append(dict(path=str(p.relative_to(r)).replace('\\','/'),sha256=sha(p.read_bytes()),bytes=p.stat().st_size))
(r/'manifest.json').write_text(json.dumps(dict(head=head,base=base,files=files),indent=2)+'\n',encoding='utf-8');print(tests);print('stopped work granularity',stopchanges);print(sha((r/'manifest.json').read_bytes()))
