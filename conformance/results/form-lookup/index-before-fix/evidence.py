from pathlib import Path
import hashlib,json,subprocess,re
r=Path(__file__).parent
def textlog(name):
 b=(r/name).read_bytes();return b.decode('utf-16' if b[:2] in (b'\xff\xfe',b'\xfe\xff') else 'utf-8-sig')
def git(*args):return subprocess.check_output(['git','-C',str(r.parents[1]),*args])
records={}
for name in ['native.log','wasi.log','probe-native.log','probe-before.log','probe-wasi.log','subject.log']:
 s=textlog(name);records[name]={'test_results':re.findall(r'test result:.*',s),'observations':re.findall(r'REVIEW_[^\r\n]*',s)}
native=records['probe-native.log']['observations'];before=records['probe-before.log']['observations'];wasi=records['probe-wasi.log']['observations']
def identity(lines):return next(x.split('identity=',1)[1] for x in lines if x.startswith('REVIEW_CHECK'))
assert identity(native)==identity(before)==identity(wasi)
records['identity_equal_before_native_wasi']=True
records['head']=git('rev-parse','9dbda46').decode().strip();records['base']=git('rev-parse','3f23aec').decode().strip()
(r/'results.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
paths=git('diff','--name-only','3f23aec','9dbda46').decode().splitlines()+['doc/spec/04-grammar.md']
inputs=[]
for p in paths:
 b=git('show','9dbda46:'+p);inputs.append({'path':p,'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)})
(r/'sources.json').write_text(json.dumps(inputs,indent=2)+'\n',encoding='utf-8')
files=['probe.py','subject.py','evidence.py','results.json','sources.json','review.md','native.log','wasi.log','probe-native.log','probe-before.log','probe-wasi.log','subject.log']
entries=[]
for p in files:
 b=(r/p).read_bytes();entries.append({'path':p,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(r/'manifest.json').write_text(json.dumps(entries,indent=2)+'\n',encoding='utf-8')
print(hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest())
