from pathlib import Path
import subprocess,json,hashlib,time
D=Path(__file__).resolve().parent;W=D/'workspace';E=D/'nepl3-tools.exe';F=D/'filesystem';F.mkdir(exist_ok=True);runs=[];sha=lambda b:hashlib.sha256(b).hexdigest()
def call(name,input_path,out,success):
 cmd=[str(E),'doc-html','export',str(input_path),str(out)];start=time.time();p=subprocess.run(cmd,cwd=F,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180);(D/('cli-'+name+'.log')).write_bytes(p.stdout);runs.append({'name':name,'command':cmd,'exit':p.returncode,'seconds':time.time()-start,'deadline':180,'input_sha256':sha(input_path.read_bytes())if input_path.is_file()else None});(D/'cli-runs.json').write_text(json.dumps(runs,indent=2),encoding='utf-8');assert (p.returncode==0)==success,(name,p.stdout.decode(errors='replace'));print(name,p.returncode,flush=True);return p.stdout
source='article ja "Title" body cons paragraph cons "[x/read]{y/note} & <tag>" nil nil\r\n';src=F/'input spaces & [literal].nepld';src.write_bytes(source.encode())
for name in ['first','repeat']:call(name,src,F/name,True)
for name in ['document.html','assets/doc.css','manifest.json']:assert (F/'first'/name).read_bytes()==(F/'repeat'/name).read_bytes(),name
m=json.loads((F/'first/manifest.json').read_text());assert m['source_sha256']==sha(src.read_bytes());assert m['options']=={'parallel':'Rows'};assert not m['viewer_scripts'];assert 'Separate bounded operations' in m['budget_scope']
for row in m['files']:assert row['sha256']==sha((F/'first'/row['path']).read_bytes())
assert (F/'first/assets/doc.css').read_bytes()==(W/'crates/languages/doc/html/assets/doc.css').read_bytes()
old={str(p.relative_to(F/'first')):sha(p.read_bytes())for p in (F/'first').rglob('*')if p.is_file()};call('existing-directory',src,F/'first',False);assert old=={str(p.relative_to(F/'first')):sha(p.read_bytes())for p in (F/'first').rglob('*')if p.is_file()}
old=src.read_bytes();call('input-as-output',src,src,False);assert src.read_bytes()==old
call('missing-parent',src,F/'absent/child',False);assert not (F/'absent').exists()
for name,data in [('invalid-utf8',b'\xff'),('over-cap',b' '*10_000_001),('syntax',b'article ja "unclosed'),('trailing',source.encode()+b' unexpected'),('hidden-label',b'article ja "T" body cons paragraph cons sentence cons ref missing text "x" nil nil nil'),('external',b'article ja "T" body cons paragraph cons sentence cons link external "https://example.invalid/" text "x" nil nil nil'),('asset',b'article ja "T" body cons image asset "missing" none "alt" none nil'),('foreign',b'article ja "T" body cons code Math root 0 9 nil')]:
 p=F/(name+'.nepld');p.write_bytes(data);log=call(name,p,F/name,False);assert not (F/name).exists();
 if name in ['external','asset','foreign']:assert b'NeedsResolution' in log,log
 if name=='over-cap':assert b'SourceLimit' in log
for count in [250,251,252]:
 p=F/('depth'+str(count)+'.nepld');p.write_text('article ja sentence cons '+'strong '*count+'text "x" nil body nil',encoding='utf-8');out=F/('depth'+str(count));cmd=[str(E),'doc-html','export',str(p),str(out)];r=subprocess.run(cmd,cwd=F,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180);(D/('depth'+str(count)+'.log')).write_bytes(r.stdout);runs.append({'name':'depth'+str(count),'command':cmd,'exit':r.returncode,'deadline':180,'input_sha256':sha(p.read_bytes())});print('depth',count,r.returncode,flush=True)
large=W/'examples/document/linear-combination.nepld';call('large',large,F/'large',True)
(D/'cli-runs.json').write_text(json.dumps(runs,indent=2),encoding='utf-8')
print('CLI independent filesystem/identity/local-only matrix completed',flush=True)
