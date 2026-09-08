from pathlib import Path
import hashlib,json,re,subprocess,tomllib
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-tokenizer-recovering-api'
def sha(data):return hashlib.sha256(data).hexdigest()
def git(*args):return subprocess.check_output(['git','-C',repo,*args])
source=json.loads((r/'source-manifest.json').read_text());head=source['commit'];base=json.loads((r/'before-manifest.json').read_text())['commit']
verified={}
for directory in ['source','probe']:
 n=0
 for f in source['files']:
  if f['path'] in ['Cargo.toml','Cargo.lock']:continue
  assert sha((r/directory/f['path']).read_bytes())==f['sha256'],(directory,f['path']);n+=1
 verified[directory]=n
old=b'Err(failure) => match prefix.restore(failure.accepted) {'
fault=(r/'fault/crates/foundation/reader/src/tokenizer/session.rs').read_bytes()
assert fault.replace((r/'fault-hook.txt').read_bytes(),old)==(r/'source/crates/foundation/reader/src/tokenizer/session.rs').read_bytes()
original_lock=tomllib.loads(git('show',head+':Cargo.lock').decode())
for directory in ['source','probe','fault']:
 lock=tomllib.loads((r/directory/'Cargo.lock').read_text())
 original_external={p['name']:p for p in original_lock['package'] if 'source' in p}
 for p in lock['package']:
  if 'source' in p:assert p==original_external[p['name']]
(r/'public-probe.rs').write_bytes((r/'probe/crates/foundation/reader/tests/independent_recover.rs').read_bytes())
for name in ['Cargo.toml','Cargo.lock']:(r/('review-'+name)).write_bytes((r/'source'/name).read_bytes())
delta=[]
for path in git('diff','--name-only',base,head).decode().splitlines():
 data=git('show',head+':'+path);delta.append(dict(path=path,sha256=sha(data),bytes=len(data)))
(r/'verification.json').write_text(json.dumps(dict(head=head,base=base,delta=delta,verified_pristine_non_manifest_files=verified,fault_only_expected_hook=True,external_lock_entries_match=True,working_tree_status=git('status','--short').decode()),indent=2)+'\n',encoding='utf-8')
versions={cmd:subprocess.check_output([cmd,'--version']).decode().strip() for cmd in ['rustc','cargo','wasmtime']}
(r/'versions.json').write_text(json.dumps(versions,indent=2)+'\n',encoding='utf-8')
for name in ['managed-native','managed-wasi','probe-native','probe-wasi','fault-native','fault-wasi']:
 assert json.loads((r/(name+'.json')).read_text())['exit_code']==0
tests={name:sum(map(int,re.findall(r'test result: ok\. (\d+) passed;', (r/(name+'.log')).read_text(encoding='utf-8')))) for name in ['managed-native','managed-wasi','probe-native','probe-wasi','fault-native','fault-wasi']}
(r/'test-summary.json').write_text(json.dumps(tests,indent=2)+'\n',encoding='utf-8')
files=[]
for p in sorted(r.iterdir()):
 if p.is_file() and p.name!='manifest.json':files.append(dict(path=p.name,sha256=sha(p.read_bytes()),bytes=p.stat().st_size))
(r/'manifest.json').write_text(json.dumps(dict(review='tokenizer-native-recovering-api',head=head,base=base,files=files),indent=2)+'\n',encoding='utf-8')
print(tests);print('manifest',sha((r/'manifest.json').read_bytes()))
