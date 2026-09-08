from pathlib import Path
import subprocess,json,gzip,hashlib,sys,html.parser
R=Path(__file__).resolve().parent;repo=R.parents[1]
def sha(b):return hashlib.sha256(b).hexdigest()
def save(p,b):
 q=R/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
def git(*args):return subprocess.check_output(['git','-C',str(repo),*args])
p=subprocess.run([sys.executable,str(R/'compare.py')],stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
save('compare.log',p.stdout);assert p.returncode==0
save('production.patch',git('diff','e568d95','889c5a4'))
assert git('rev-parse','c15928a^{tree}')==git('rev-parse','889c5a4^{tree}')
assert git('diff','e568d95','889c5a4','--','interfaces','languages','tools/src/doc','crates/languages/doc')==b''
save('versions.txt',subprocess.check_output(['rustc','--version','--verbose'])+subprocess.check_output(['cargo','--version'])+sys.version.encode())
initial=(R/'probe.rs').read_bytes().replace(b'  let bytes=nepl3_wire::encode(&v,&mut budget()).map_err(err)?;\n  std::fs::write(out.join("document.cbor"),bytes).map_err(err)?;\n',b'')
save('probe-initial.rs',initial)
maps=[]
def compressed(p):
 b=(R/p).read_bytes();dst='compressed/'+p+'.gz';z=gzip.compress(b,mtime=0);save(dst,z);assert gzip.decompress(z)==b
 maps.append({'stored':dst,'original':p,'bytes':len(b),'sha256':sha(b)})
for label in ['before','after']:
 compressed(label+'-files.json')
 for name in ['tree.txt','document.txt','document.cbor']:
  compressed('runs-'+label+'/semantic-cbor/'+name)
 for path in ['crates/foundation/engine/src/parse/build.rs','crates/foundation/engine/src/parse/build/tests.rs','crates/foundation/engine/src/parse/session.rs','tools/src/doc/source.rs','tools/src/doc/export.rs']:
  save('source/'+label+'/'+path,(R/label/path).read_bytes())
root=repo/'.tmp/actual-site-pages';data=json.loads((root/'results.json').read_bytes())
assert data['exit_code']==0
assert sha(Path(data['command'][0]).read_bytes())==data['binary_sha256']
assert sha((root/'command.log').read_bytes())==data['log_sha256']
for e in data['sources']:
 b=git('show','889c5a4:'+e['path'].replace('\\','/'))
 assert sha(b)==e['sha256'] and len(b)==e['bytes']
 assert b==(root/'drafts'/Path(e['path']).name).read_bytes()
for e in data['files']:
 b=(root/e['path']).read_bytes();assert sha(b)==e['sha256'] and len(b)==e['bytes']
manifest=json.loads((root/'output/manifest.json').read_bytes())
for e in manifest['files']:assert sha((root/'output'/e['path']).read_bytes())==e['sha256']
request=json.loads((root/'input.json').read_bytes())
assert manifest['output_budget']['limits']==request['output_limits']
assert manifest['output_budget']['usage']['work']==125393174
for page in manifest['pages']:
 assert page['operation_limits']['work']==100000000 and page['operation_limits']['allocation_units']==500000000
class Links(html.parser.HTMLParser):
 def __init__(self):super().__init__();self.links=[]
 def handle_starttag(self,t,a):
  if t=='a':self.links.extend(v for k,v in a if k=='href')
h=Links();h.feed((root/'output/docs/15-site/index.html').read_text(encoding='utf8'))
assert '../17-math-html/index.html' in h.links
for f in root.rglob('*'):
 if f.is_file():save('root-pages-receipt/'+f.relative_to(root).as_posix(),f.read_bytes())
save('root-pages-receipt/script.py',(repo/'.tmp/actual-site-pages.py').read_bytes())
save('storage-map.json',(json.dumps(maps,indent=2)+'\n').encode())
selected=[]
for f in sorted(R.rglob('*')):
 if not f.is_file():continue
 p=f.relative_to(R);s=p.as_posix()
 if p.parts[0] in ['before','after','target-before','target-after'] or f.suffix=='.tar' or f.name in ['before-files.json','after-files.json','manifest.json']:continue
 if p.parts[0].startswith('runs-') and len(p.parts)>1:
  if p.parts[1]=='semantic':continue
  if p.parts[1]=='semantic-cbor' and f.name in ['tree.txt','document.txt','document.cbor']:continue
 b=f.read_bytes();selected.append({'path':s,'bytes':len(b),'sha256':sha(b)})
save('manifest.json',(json.dumps({'files':selected},indent=2)+'\n').encode())
print(json.dumps({'files':len(selected),'bytes':sum(e['bytes'] for e in selected),'manifest_sha256':sha((R/'manifest.json').read_bytes())}))
