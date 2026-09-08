import pathlib,subprocess,hashlib,json
p=pathlib.Path(__file__).resolve().parent
repo='C:/projects/NEPL3-doc-authoring-delivery'
commit='46a54fb8b4be41668696895f63918213ccc6d1fa'
base=subprocess.check_output(['git','-C',repo,'rev-parse','c90d306']).decode().strip()
files=[(commit,'doc/migration/authored/20-doc-html.nepld'),(base,'doc/spec/20-doc-html.md')]+[(commit,x) for x in ['doc/authoring.md','AGENTS.md','design/forms.json']]
manifest=[]
for rev,path in files:
    data=subprocess.check_output(['git','-C',repo,'show',rev+':'+path])
    target=p/'snapshot'/path; target.parent.mkdir(parents=True,exist_ok=True); target.write_bytes(data)
    manifest.append(dict(commit=rev,path=path,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
(p/'sources.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n')
