from pathlib import Path
import zipfile,json,hashlib,shutil,re,subprocess
p=Path(__file__).resolve().parent;sha=lambda f:hashlib.sha256(f.read_bytes()).hexdigest();fixed=p.name.endswith('-fixed')
snapshot=json.loads((p/'snapshot.json').read_text());overlay={r['path']:r for r in snapshot['files']}
with zipfile.ZipFile(p/'base.zip') as z:
 for item in z.infolist():
  if item.is_dir():continue
  if item.filename not in overlay:assert (p/'source'/item.filename).read_bytes()==z.read(item),item.filename
for row in overlay.values():
 f=p/'source'/row['path'];assert sha(f)==row['sha256'];out=p/'overlay'/row['path'];out.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(f,out)
for run in json.loads((p/'runs.json').read_text()):
 assert run['exit']==0;log=(p/run['log']).read_text();assert f'test result: ok. {4 if fixed else 3} passed; 0 failed;' in log
 if fixed:
  f=Path(re.search(r'Running unittests root.rs \((.+)\)',log)[1]);out=p/'artifacts'/f.name;out.parent.mkdir(exist_ok=True);shutil.copyfile(f,out)
for run in json.loads((p/'contracts-runs.json').read_text()):assert run['exit']==0 and '24 passed; 0 failed;' in (p/run['log']).read_text()
assert json.loads((p/'noatomic-run.json').read_text())['exit']==0
assert 'target_has_atomic="ptr"' not in (p/'noatomic-cfg.txt').read_text()
if fixed:
 assert (p/'allocation-order.rs').read_bytes()==(p.parent/'review-admission-storage-cache/allocation-order.rs').read_bytes()
else:
 assert json.loads((p/'before-order-run.json').read_text())['exit']==101
 assert '25820' in (p/'before-order.log').read_text() and '26959' in (p/'before-order.log').read_text()
versions={' '.join(c):subprocess.check_output(c).decode().strip() for c in [['rustc','--version','--verbose'],['cargo','--version'],['wasmtime','--version']]}
for f in p.iterdir():
 if f.is_file() and f.suffix in ['.py','.md']:f.write_text(f.read_text(encoding='utf-8-sig'),encoding='utf-8',newline='\n')
files=[f for f in p.iterdir() if f.is_file() and f.name!='manifest.json']
for folder in ['probe','overlay','artifacts']:files += [f for f in (p/folder).rglob('*') if f.is_file()]
result=dict(base=snapshot['base'],scope='Final deterministic owning admission storage cache' if fixed else 'Before deterministic charge correction: ownership/atomicity tests pass but native allocation-order comparison fails',versions=versions,files=[dict(path=f.relative_to(p).as_posix(),sha256=sha(f),bytes=f.stat().st_size) for f in sorted(files)])
(p/'manifest.json').write_text(json.dumps(result,indent=2),encoding='utf-8',newline='\n');print(str(p/'manifest.json'),sha(p/'manifest.json'),len(files))
