import pathlib,subprocess,json,hashlib,re,shutil
p=pathlib.Path(__file__).resolve().parent
def sha(b):return hashlib.sha256(b).hexdigest()
sources=json.loads((p/'sources.json').read_text());before=json.loads((p/'before/sources.json').read_text());overrides={r['path']:r for r in before['overrides']}
for root in [p,p/'before']:
 for f in sources['files']:
  expected=overrides.get(f['path'],f) if root!=p else f;b=(root/'workspace'/f['path']).read_bytes();assert sha(b)==expected['sha256'] and len(b)==expected['bytes']
 for name in ['review_locality.rs','review_adjacency.rs']:
  b=(root/'workspace/crates/foundation/core/tests'/name).read_bytes()
  if not (p/name).exists():(p/name).write_bytes(b)
  assert b==(p/name).read_bytes()
for name in ['02-foundation.md','12-model-invariants.md']:
 q=p/'snapshot/doc/spec'/name;q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p/'workspace/doc/spec'/name,q)
versions={}
for command in [['rustc','-Vv'],['cargo','-V'],['wasmtime','--version']]:versions[' '.join(command)]=subprocess.check_output(command).decode().strip()
(p/'versions.json').write_text(json.dumps(versions,indent=2)+'\n',encoding='utf-8',newline='\n')
runs=[]
for root,labels,count in [(p,['native-all','wasi-all'],84),(p/'before',['native-independent','wasi-independent'],6)]:
 for label in labels:
  metadata=json.loads((root/(label+'.json')).read_text());assert metadata['exit_code']==0;log=(root/(label+'.log')).read_text(encoding='utf-8');totals=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',log)
  assert sum(int(x[0]) for x in totals)==count and all(x[1:]==('0','0') for x in totals)
  expected=[478106,1049590] if root==p else [808432,1803898]
  stars=re.findall(r'star width=(\d+) usage=Usage \{ source_bytes: 0, work: (\d+), depth: 2, nodes: (\d+), allocation_units: (\d+)',log)
  assert [int(x[1]) for x in stars[:2]]==expected
  if root==p:assert 'exhaustive_work_stops=673' in log and 'cap1500000=Err' not in log
  else:assert 'exhaustive_work_stops=967' in log and 'cap1500000=Err(Stopped(WorkLimit))' in log
  artifacts=[]
  for rel in re.findall(r'Running [^\r\n]*?\((target[^)]+)\)',log):
   file=root/'workspace'/rel.replace('\\','/');b=file.read_bytes();artifacts.append(dict(path=str(file),bytes=len(b),sha256=sha(b)))
  runs.append(dict(label=str(root.relative_to(p)/label),passed=count,commands=metadata,artifacts=artifacts,stars=stars))
(p/'evidence.json').write_text(json.dumps(dict(production_files_revalidated=598,identical_independent_inputs=True,runs=runs),indent=2)+'\n',encoding='utf-8',newline='\n')
files=[]
for f in sorted(p.rglob('*')):
 if f.is_file() and 'workspace' not in f.relative_to(p).parts and '__pycache__' not in f.parts and f.name!='manifest.json':
  b=f.read_bytes();files.append(dict(path=f.relative_to(p).as_posix(),bytes=len(b),sha256=sha(b)))
b=(json.dumps(dict(files=files),indent=2)+'\n').encode();(p/'manifest.json').write_bytes(b);print(len(files),sum(f['bytes'] for f in files),sha(b))
