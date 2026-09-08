from pathlib import Path
import subprocess,json,hashlib
out=Path(__file__).resolve().parent
repo=Path('C:/projects/NEPL3-doc-authoring-b')
rows=[]
for label,commit,chapter in [('11','ebac378','11-conformance'),('12','9dd5a5c','12-model-invariants')]:
    full=subprocess.check_output(['git','rev-parse',commit],cwd=repo,text=True).strip()
    parent=subprocess.check_output(['git','rev-parse',commit+'^'],cwd=repo,text=True).strip()
    requests=[('before.nepld',parent,'doc/migration/authored/'+chapter+'.nepld'),('after.nepld',full,'doc/migration/authored/'+chapter+'.nepld'),('original.md',full,'doc/spec/'+chapter+'.md')]
    for name,rev,path in requests:
        data=subprocess.check_output(['git','show',rev+':'+path],cwd=repo)
        dest=out/label/name;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
        rows.append({'path':str(dest.relative_to(out)),'source_commit':rev,'source_path':path,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
    patch=subprocess.check_output(['git','diff',parent,full,'--','doc/migration/authored/'+chapter+'.nepld'],cwd=repo)
    (out/label/'change.patch').write_bytes(patch)
for path in ['AGENTS.md','doc/authoring.md','doc/spec/05-document.md','doc/spec/doc-signatures.md','design/forms.json','tools/audit/structure.py']:
    data=subprocess.check_output(['git','show','9dd5a5c:'+path],cwd=repo)
    dest=out/'context'/path;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
    rows.append({'path':str(dest.relative_to(out)),'source_commit':'9dd5a5c','source_path':path,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
rev='5657677e9b4a1cc44830b9a365206822920983fe'
path='doc/spec/12-model-invariants.md'
data=subprocess.check_output(['git','show',rev+':'+path],cwd=repo)
dest=out/'12'/'corrected-original.md';dest.write_bytes(data)
rows.append({'path':str(dest.relative_to(out)),'source_commit':rev,'source_path':path,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
(out/'sources.json').write_bytes((json.dumps(rows,indent=2)+'\n').encode())
print('Frozen',len(rows),'source files')
