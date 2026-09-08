from pathlib import Path
import subprocess,hashlib,json,re
R=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-tree-selection-lookup'
def sha(b):return hashlib.sha256(b).hexdigest()
sources=json.loads((R/'sources.json').read_bytes());allrows=[]
for e in sources:
 rows=[]
 for raw in subprocess.check_output(['git','-C',repo,'ls-tree','-rz','--full-tree',e['commit']],timeout=60).split(b'\0'):
  if not raw:continue
  meta,path=raw.split(b'\t',1);mode,kind,oid=meta.decode().split();path=path.decode()
  if kind!='blob':continue
  b=(R/e['name']/path).read_bytes()
  if path==e['test_path']:assert sha(b)==e['test_after_sha256']
  else:assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==oid,(e['name'],path)
  rows.append(dict(path=path,git_blob=oid,sha256=sha(b),bytes=len(b)))
 allrows.append(dict(name=e['name'],commit=e['commit'],files=rows))
(R/'workspace-files.json').write_text(json.dumps(allrows,indent=2)+'\n',encoding='utf-8')
logs={n:(R/(n+'.log')).read_text(encoding='utf-8') for n in ['before-native','after-native','after-wasi']}
errors={n:re.findall(r'REVIEW_ERROR ([^\r\n]+)',s) for n,s in logs.items()}
raw={n:re.findall(r'REVIEW_RAW ([^\r\n]+)',s) for n,s in logs.items()}
assert len(errors['before-native'])==216 and len(raw['before-native'])==5
assert errors['before-native']==errors['after-native']==errors['after-wasi']
assert raw['before-native']==raw['after-native']==raw['after-wasi']
def costs(s):return {(int(n),r):(int(w),int(a)) for n,r,w,a in re.findall(r'REVIEW_COST n=(\d+) reverse=(\w+) work=(\d+) allocation=(\d+)',s)}
c={n:costs(s) for n,s in logs.items()}
for key,after in c['after-native'].items():
 n=key[0];before=c['before-native'][key];wasi=c['after-wasi'][key]
 assert after[0]-before[0]==-4*n*n+3
 assert after[1]-before[1]==(2*n+3)*8+48
 assert after[0]==wasi[0]
counts={n:sum(map(int,re.findall(r'test result: ok\. (\d+) passed',s))) for n,s in logs.items()}
assert counts=={'before-native':3,'after-native':24,'after-wasi':24}
result=dict(result='passed',counts=counts,error_matrix=216,raw_first_receiver_negatives=5,portable_permutations=24,costs={n:[dict(parents=k[0],reverse=k[1],work=v[0],allocation=v[1]) for k,v in d.items()] for n,d in c.items()},work_formula='after-before=-4*n*n+3',native_allocation_formula='after-before=(2*n+3)*8+48')
(R/'result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
for e in sources:
 (R/(e['name']+'-tree.rs')).write_bytes(subprocess.check_output(['git','-C',repo,'show',e['commit']+':crates/foundation/engine/src/tree.rs'],timeout=60))
(R/'delta.diff').write_bytes(subprocess.check_output(['git','-C',repo,'diff',sources[0]['commit'],sources[1]['commit']],timeout=60))
(R/'rust-version.txt').write_bytes(subprocess.check_output(['rustc','-vV'],timeout=60))
(R/'wasmtime-version.txt').write_bytes(subprocess.check_output(['wasmtime','--version'],timeout=60))
print(json.dumps(dict(result='passed',counts=counts)))
