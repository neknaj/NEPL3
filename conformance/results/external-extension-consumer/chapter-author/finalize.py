from pathlib import Path
import subprocess,sys,json,hashlib,difflib
p=Path(__file__).resolve().parent;root=p.parents[1]
name='doc/migration/authored/22-external-extensions.nepld'
d=(root/name).read_bytes()
assert b'\r' not in d and not d.startswith(b'\xef\xbb\xbf')
d.decode('utf-8')
(p/'draft.nepld').write_bytes(d)
(p/'draft.patch').write_text(''.join(difflib.unified_diff([],d.decode('utf-8').splitlines(True),fromfile='/dev/null',tofile=name)),encoding='utf-8',newline='\n')
r=subprocess.run([sys.executable,str(p/'check.py')],cwd=root,capture_output=True)
(p/'check.log').write_bytes(r.stdout+r.stderr)
assert r.returncode==0
files=[]
for f in sorted(p.iterdir()):
    if f.is_file() and f.name!='manifest.json':
        b=f.read_bytes();files.append({'path':f.name,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(p/'manifest.json').write_text(json.dumps({'scope':'Manual authored chapter 22, author self-check only','source':'doc/spec/22-external-extensions.md','draft':name,'source_head_context':'782309899fdc55aa9db05588f8cc0d9316ee8738','source_uncommitted_at_capture':True,'production_edited':False,'independent_review':False,'runtime':False,'canonical_cutover':False,'files':files},ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(hashlib.sha256((p/'manifest.json').read_bytes()).hexdigest())
