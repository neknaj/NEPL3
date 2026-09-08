from pathlib import Path
import re,json,hashlib,subprocess
r=Path(__file__).parent;repo=r.parents[1]
def log(n):
 b=(r/n).read_bytes();return b.decode('utf-16'if b[:2]==b'\xff\xfe'else'utf-8-sig')
def observations(n,prefix):return re.findall(prefix+r'[^\r\n]*',log(n))
assert observations('native-fixed.log','REVIEW_OUTCOME ')==observations('before-fixed.log','REVIEW_OUTCOME ')==observations('wasi-fixed.log','REVIEW_OUTCOME ')
for prefix in ['REVIEW_TEXT_OUTCOME ','REVIEW_TEXT_SOURCES ','REVIEW_TEXT_MAPS ']:assert observations('text-native.log',prefix)==observations('text-before.log',prefix)==observations('text-wasi.log',prefix),prefix
names=['native.log','before.log','wasi.log','native-fixed.log','before-fixed.log','wasi-fixed.log','text-native.log','text-before.log','text-wasi.log']
results={n:{'tests':re.findall('test result:.*',log(n)),'usage':observations(n,'REVIEW_(?:TEXT_)?USAGE ')}for n in names}
(r/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
inputs=[]
for p in git('diff','--name-only','e568d95','889c5a4').decode().splitlines()+['crates/foundation/engine/src/parse/model.rs','crates/foundation/engine/src/parse/session/head.rs','doc/spec/03-reader.md']:
 b=git('show','889c5a4:'+p);inputs.append({'path':p,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
bins=[]
for directory in ['target/debug/deps','target-wasi/wasm32-wasip2/debug/deps']:
 for p in (r/directory).glob('parse-*'):
  if p.suffix in ['.exe','.wasm']:bins.append({'path':p.relative_to(r).as_posix(),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
(r/'sources.json').write_text(json.dumps({'head':git('rev-parse','889c5a4').decode().strip(),'base':git('rev-parse','e568d95').decode().strip(),'files':inputs,'probe_binaries':bins,'outcomes_sources_maps_equal_before_after_and_wasi':True},indent=2)+'\n',encoding='utf-8')
(r/'production.diff').write_bytes(git('diff','e568d95','889c5a4'))
files=names+['setup.py','probe.rs','fix-probe.py','add-text.py','generated-probe.rs','finish.py','results.json','sources.json','production.diff','review.md']
entries=[]
for n in files:
 b=(r/n).read_bytes();entries.append({'path':n,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(r/'manifest.json').write_text(json.dumps(entries,indent=2)+'\n',encoding='utf-8');print(hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest(),len(entries))
