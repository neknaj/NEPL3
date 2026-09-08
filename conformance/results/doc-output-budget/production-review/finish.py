from pathlib import Path
import json,hashlib,subprocess,re,struct
r=Path(__file__).parent;repo=r.parents[1]
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
results={}
for name in ['native.log','wasi.log','native-corrected.log','wasi-corrected.log','before.log']:
 b=(r/name).read_bytes();s=b.decode('utf-16' if b[:2]==b'\xff\xfe' else 'utf-8-sig');results[name]=re.findall('test result:.*',s)
 if 'corrected' in name:assert '7 passed; 0 failed' in s
wasm=json.loads((r/'workspace/observed-manifest.json').read_text());native=json.loads((r/'native-manifest.json').read_text());assert wasm['identity']==native['identity']
keys=['source_bytes','work','depth','nodes','allocation_units','output_bytes','diagnostics','events']
expected=hashlib.sha256(b'nepl3.local-doc-pages.execution/1\0'+bytes.fromhex(wasm['identity'])+b''.join(struct.pack('>Q',wasm['output_budget'][part][k])for part in ['limits','initial_usage']for k in keys)).hexdigest();assert expected==wasm['execution_identity']
(r/'wasi-manifest.json').write_text(json.dumps(wasm,indent=2)+'\n',encoding='utf-8')
(r/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
(r/'production.diff').write_bytes(git('diff','244c15f','b84e9a3'))
inputs=[]
for p in git('diff','--name-only','244c15f','b84e9a3').decode().splitlines()+['tools/src/doc/source.rs']:
 b=git('show','b84e9a3:'+p);inputs.append({'path':p,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(r/'inputs.json').write_text(json.dumps({'head':git('rev-parse','b84e9a3').decode().strip(),'base':git('rev-parse','244c15f').decode().strip(),'files':inputs},indent=2)+'\n',encoding='utf-8')
names=['setup.py','probe.rs','observe.py','cli.py','finish.py','native.log','wasi.log','native-corrected.log','wasi-corrected.log','before.log','native-manifest.json','before-manifest.json','wasi-manifest.json','files.json','comparison.json','cli.json','results.json','inputs.json','production.diff','review.md']
entries=[]
for p in names:
 b=(r/p).read_bytes();entries.append({'path':p,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(r/'manifest.json').write_text(json.dumps(entries,indent=2)+'\n',encoding='utf-8');print(hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest())
