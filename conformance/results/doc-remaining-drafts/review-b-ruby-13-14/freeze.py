from pathlib import Path
import subprocess,json,hashlib
out=Path(__file__).resolve().parent
repo=Path('C:/projects/NEPL3-doc-authoring-b')
rows=[]
for label,commit,chapter in [('13','7e9e52c','13-reproducibility'),('14','d50f168','14-web-ui')]:
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
    data=subprocess.check_output(['git','show','d50f168:'+path],cwd=repo)
    dest=out/'context'/path;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
    rows.append({'path':str(dest.relative_to(out)),'source_commit':'d50f168','source_path':path,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
(out/'sources.json').write_bytes((json.dumps(rows,indent=2)+'\n').encode())
print('Frozen',len(rows),'source files')

