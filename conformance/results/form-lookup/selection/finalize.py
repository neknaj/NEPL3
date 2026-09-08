from pathlib import Path
import hashlib,json,re,subprocess,tomllib,shutil
r=Path(__file__).resolve().parent
subprocess.run(['python',str(r/'compare.py')],check=True,stdout=subprocess.DEVNULL)
verified=[]
for name in ['source','before']:
 m=json.loads((r/(name+'-manifest.json')).read_text());checked=0
 for row in m['files']:
  if row['path'] in ['Cargo.toml','Cargo.lock']:continue
  p=r/name/row['path'];assert hashlib.sha256(p.read_bytes()).hexdigest()==row['sha256'],p;checked+=1
 original=subprocess.check_output(['git','-C','C:/projects/NEPL3-parser-form-selection','show',m['commit']+':Cargo.lock'])
 def external(b):return sorted((x['name'],x['version'],x.get('checksum')) for x in tomllib.loads(b.decode())['package'] if 'source' in x)
 old=external(original);new=external((r/name/'Cargo.lock').read_bytes());assert set(new)<=set(old)
 verified.append(dict(name=name,commit=m['commit'],unchanged_files=checked,retained_external_lock_versions_checksums_unchanged=True,omitted_unused_external_packages=sorted(set(old)-set(new))))
for mode in ['native','before-native','wasi','managed-native','managed-wasi']:assert json.loads((r/(mode+'.json')).read_text())['exit_code']==0
for mode in ['managed-native','managed-wasi']:
 n=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(r/(mode+'.log')).read_text(encoding='utf-8'))));assert n==61,(mode,n)
(r/'verification.json').write_text(json.dumps(verified,indent=2)+'\n',encoding='utf-8')
with (r/'versions.log').open('wb') as f:
 for cmd in [['rustc','--version'],['cargo','--version'],['wasmtime','--version']]:subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,check=True)
p=r/'probe';p.mkdir(exist_ok=True)
for file in (r/'source/probe').rglob('*'):
 if file.is_file():target=p/file.relative_to(r/'source/probe');target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(file,target)
assert (r/'source/probe/tests/head.rs').read_bytes()==(r/'before/probe/tests/head.rs').read_bytes()
files=[]
for p in sorted(r.iterdir()):
 if p.is_file() and p.name!='manifest.json':files.append(p)
files+=sorted((r/'probe').rglob('*'));rows=[]
for p in files:
 if p.is_file():b=p.read_bytes();rows.append(dict(path=p.relative_to(r).as_posix(),sha256=hashlib.sha256(b).hexdigest(),bytes=len(b)))
(r/'manifest.json').write_text(json.dumps(dict(candidate='3f23aec87da9bef9e7ac02639d4bb08dafed2aab',base='6f84a51b626de595fb73635ed4b116055edfb9e4',files=rows),indent=2)+'\n',encoding='utf-8')
print(len(rows),hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest())
