from pathlib import Path
import json,hashlib,subprocess,re
r=Path(__file__).parent;repo=r.parents[1]
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
records={}
for name in ['native.log','before.log','wasi.log','long-native.log','long-before.log']:
 b=(r/name).read_bytes();s=b.decode('utf-16' if b[:2]==b'\xff\xfe' else 'utf-8-sig');assert '; 0 failed;' in s
 records[name]={'results':re.findall(r'test result:.*',s),'observations':re.findall(r'REVIEW_[^\r\n]*',s)}
def conflicts(name):return [x for x in records[name]['observations'] if x.startswith('REVIEW_CONFLICT')]
assert conflicts('native.log')==conflicts('before.log')==conflicts('wasi.log')
a=(r/'workspace/workspace.html').read_bytes();b=(r/'before/before.html').read_bytes();assert a==b
(r/'output.html').write_bytes(a)
records['output']={'bytes':len(a),'sha256':hashlib.sha256(a).hexdigest(),'before_after_equal':True}
source=(r/'workspace/examples/document/linear-combination.nepld').read_bytes()
records['input']={'path':'examples/document/linear-combination.nepld','bytes':len(source),'sha256':hashlib.sha256(source).hexdigest()}
records['head']=git('rev-parse','13b2155').decode().strip();records['base']=git('rev-parse','8b6088e').decode().strip()
(r/'results.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
(r/'production.diff').write_bytes(git('diff','8b6088e','13b2155'))
inputs=[]
for p in ['crates/languages/doc/core/src/check/document.rs','crates/languages/doc/core/tests/portable.rs','crates/foundation/core/src/source.rs','doc/spec/12-model-invariants.md']:
 b=git('show','13b2155:'+p);inputs.append({'path':p,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(r/'sources.json').write_text(json.dumps(inputs,indent=2)+'\n',encoding='utf-8')
files=['setup.py','probe.rs','longdoc.py','finish.py','native.log','before.log','wasi.log','long-native.log','long-before.log','results.json','sources.json','production.diff','output.html','review.md']
entries=[]
for p in files:
 b=(r/p).read_bytes();entries.append({'path':p,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(r/'manifest.json').write_text(json.dumps(entries,indent=2)+'\n',encoding='utf-8')
print(hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest());print(records['input']);print(records['output'])
