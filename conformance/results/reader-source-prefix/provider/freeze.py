from pathlib import Path
import json,hashlib,re,shutil,subprocess,zipfile
p=Path(__file__).resolve().parent
sha=lambda f:hashlib.sha256(f.read_bytes()).hexdigest()
snapshot=json.loads((p/'snapshot.json').read_text());overlay={r['path']:r for r in snapshot['files']}
with zipfile.ZipFile(p/'base.zip') as z:
 for item in z.infolist():
  if item.is_dir():continue
  f=p/'source'/item.filename
  if item.filename not in overlay:assert f.read_bytes()==z.read(item),item.filename
for row in overlay.values():
 f=p/'source'/row['path'];assert sha(f)==row['sha256'];out=p/'overlay'/row['path'];out.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(f,out)
runs=json.loads((p/'runs.json').read_text());artifacts=[]
for run in runs:
 assert run['exit']==0;log=(p/run['log']).read_text();assert 'test result: ok. 39 passed; 0 failed;' in log
 source=Path(re.search(r'Running unittests root.rs \((.+)\)',log)[1]);source=source if source.is_absolute() else p.parent.parent/source
 out=p/'artifacts'/source.name;out.parent.mkdir(exist_ok=True);shutil.copyfile(source,out);artifacts.append(dict(path=out.relative_to(p).as_posix(),target=run['target'],sha256=sha(out)))
versions={' '.join(c):subprocess.check_output(c).decode().strip() for c in [['rustc','--version','--verbose'],['cargo','--version'],['wasmtime','--version']]}
for f in p.iterdir():
 if f.is_file() and f.suffix in ['.py','.md']:f.write_text(f.read_text(encoding='utf-8-sig'),encoding='utf-8',newline='\n')
files=[f for f in p.iterdir() if f.is_file() and f.name!='manifest.json']
for folder in ['probe','overlay','artifacts']:files += [f for f in (p/folder).rglob('*') if f.is_file()]
rows=[dict(path=f.relative_to(p).as_posix(),sha256=sha(f),bytes=f.stat().st_size) for f in sorted(files)]
result=dict(base=snapshot['base'],scope='Independent lazy empty provider source closure boundary review; only provider.rs overlay; 39 native and 39 WASI tests',files=rows,versions=versions,artifacts=artifacts)
(p/'manifest.json').write_text(json.dumps(result,indent=2),encoding='utf-8',newline='\n');print(str(p/'manifest.json'),sha(p/'manifest.json'),len(rows))
