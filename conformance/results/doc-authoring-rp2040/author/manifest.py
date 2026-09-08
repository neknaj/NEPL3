from pathlib import Path
import hashlib,json,subprocess
out=Path(__file__).resolve().parent;root=out.parents[1]
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
path='doc/migration/authored/conformance/targets/rp2040/README.nepld'
blob=subprocess.check_output(['git','show',head+':'+path],cwd=root)
assert blob==(out/'draft.nepld').read_bytes()==(root/path).read_bytes()
assert subprocess.check_output(['git','diff-tree','--no-commit-id','--name-only','-r',head],cwd=root,text=True).splitlines()==[path]
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root)
(out/'commit.json').write_text(json.dumps({'commit':head,'branch':'docs/rp2040-guide-current','only_changed_path':path,'git_blob_sha256':hashlib.sha256(blob).hexdigest(),'clean':True,'pushed':False},indent=2)+'\n',encoding='utf-8')
files=[]
for p in sorted(out.rglob('*')):
    if p.is_file() and p.name!='manifest.json':
        b=p.read_bytes();files.append({'path':p.relative_to(out).as_posix(),'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)})
p=out/'manifest.json';p.write_text(json.dumps({'commit':head,'scope':'manual author checkpoint, not independently reviewed or runtime-validated','files':files},indent=2)+'\n',encoding='utf-8')
print(json.dumps({'commit':head,'manifest_sha256':hashlib.sha256(p.read_bytes()).hexdigest()}))
