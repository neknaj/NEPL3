from pathlib import Path
import json,hashlib,re,tomllib,subprocess
r=Path(__file__).resolve().parent;verified={}
for name in ['source','before']:
 data=json.loads((r/(name+'-manifest.json')).read_text(encoding='utf-8'))
 for f in data['files']:
  b=(r/name/f['path']).read_bytes();assert hashlib.sha256(b).hexdigest()==f['sha256'] and len(b)==f['bytes']
 verified[name]={'commit':data['commit'],'files':len(data['files'])}
def packages(p):return set((x['name'],x['version'],x.get('checksum')) for x in tomllib.loads(p.read_text(encoding='utf-8'))['package'] if 'source' in x)
assert packages(r/'source-probe/Cargo.lock')==packages(r/'before-probe/Cargo.lock')
assert packages(r/'source-probe/Cargo.lock').issubset(packages(r/'source/Cargo.lock'))
assert (r/'tail.rs').read_bytes()==(r/'source-probe/tests/tail.rs').read_bytes()==(r/'before-probe/tests/tail.rs').read_bytes()
counts={}
for mode in ['managed-native','managed-wasi','probe-native','probe-wasi','before-native']:
 result=json.loads((r/(mode+'.json')).read_text(encoding='utf-8'));assert result['exit_code']==0
 counts[mode]=sum(map(int,re.findall(r'test result: ok\. (\d+) passed;', (r/(mode+'.log')).read_text(encoding='utf-8'))))
versions={cmd:subprocess.check_output(cmd.split()).decode().strip() for cmd in ['rustc -Vv','cargo -V','wasmtime --version']}
(r/'results.json').write_text(json.dumps(dict(snapshots=verified,test_counts=counts,identical_probe_before_after=True,probe_dependencies_match_snapshot_lock=True,versions=versions,disposition='no correctness blocking finding; locality improvements and nonlocal overhead separately measured'),indent=2)+'\n',encoding='utf-8')
files=[]
for p in sorted(r.iterdir()):
 if p.is_file() and p.name!='manifest.json':
  b=p.read_bytes();files.append(dict(path=p.name,sha256=hashlib.sha256(b).hexdigest(),bytes=len(b)))
for name in ['source-probe/Cargo.toml','source-probe/Cargo.lock','before-probe/Cargo.toml','before-probe/Cargo.lock']+[p.relative_to(r).as_posix() for p in sorted((r/'probe-draft-error').iterdir()) if p.is_file()]:
 b=(r/name).read_bytes();files.append(dict(path=name,sha256=hashlib.sha256(b).hexdigest(),bytes=len(b)))
(r/'manifest.json').write_text(json.dumps(dict(files=files),indent=2)+'\n',encoding='utf-8');print(counts);print(hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest())
