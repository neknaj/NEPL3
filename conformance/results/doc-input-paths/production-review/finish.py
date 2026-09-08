from pathlib import Path
import subprocess,json,hashlib,re,tarfile
r=Path(__file__).parent;repo=r.parents[1];sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
results={}
for n,count in [('native.log',2),('wasi.log',1)]:
 b=(r/n).read_bytes();s=b.decode('utf-16'if b[:2]==b'\xff\xfe'else'utf-8-sig');assert f'{count} passed; 0 failed'in s;results[n]=re.findall('test result:.*',s)
paths=git('diff','--name-only','7831110','a604864').decode().splitlines();inputs=[]
for p in paths:
 b=git('show','a604864:'+p);assert b==(r/'workspace'/p).read_bytes();inputs.append({'path':p,'bytes':len(b),'sha256':sha(b)})
with tarfile.open(r/'source.tar')as t:
 for m in t.getmembers():
  if m.isfile()and(m.name.startswith('tools/src/')or m.name.startswith('crates/')):assert (r/'workspace'/m.name).read_bytes()==t.extractfile(m).read()
(r/'source.json').write_text(json.dumps({'head':git('rev-parse','a6e304c').decode().strip(),'production':git('rev-parse','a604864').decode().strip(),'base':git('rev-parse','7831110').decode().strip(),'inputs':inputs,'results':results},indent=2)+'\n',encoding='utf-8')
(r/'production.diff').write_bytes(git('diff','7831110','a604864'))
names=['setup.py','cli.py','finish.py','native.log','wasi.log','cli-results.json','source.json','production.diff','review.md']
names += [p.relative_to(r).as_posix()for p in (r/'inputs').glob('*.json')]
names += [p.relative_to(r).as_posix()for p in r.glob('*.stderr')]
for label in ['omitted','null','explicit']:
 names += [p.relative_to(r).as_posix()for p in (r/('output-'+label)).rglob('*')if p.is_file()]
entries=[]
for n in names:
 b=(r/n).read_bytes();entries.append({'path':n,'bytes':len(b),'sha256':sha(b)})
(r/'manifest.json').write_text(json.dumps(entries,indent=2)+'\n',encoding='utf-8');print(sha((r/'manifest.json').read_bytes()),len(entries))
