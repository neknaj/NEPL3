import hashlib,io,json,pathlib,subprocess,zipfile
p=pathlib.Path(__file__).resolve().parent
repo='C:/projects/NEPL3-reader-checkpoints'
rev=subprocess.check_output(['git','rev-parse','92d7cd2'],cwd=repo).decode().strip()
top=['Cargo.toml','Cargo.lock','crates','tools','interfaces','design','languages','examples','doc','conformance/fixtures','AGENTS.md']
raw=subprocess.check_output(['git','archive','--format=zip',rev,*top],cwd=repo)
workspace=p/'workspace';workspace.mkdir(parents=True,exist_ok=True)
rows=[]
with zipfile.ZipFile(io.BytesIO(raw)) as z:
    for item in z.infolist():
        if item.is_dir():continue
        q=workspace/item.filename
        q.parent.mkdir(parents=True,exist_ok=True)
        data=z.read(item);q.write_bytes(data)
        rows.append(dict(path=item.filename,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
(p/'sources.json').write_text(json.dumps(dict(commit=rev,archive_sha256=hashlib.sha256(raw).hexdigest(),files=rows),indent=2)+'\n',encoding='utf-8',newline='\n')
(p/'root-probe-notes.md').write_bytes(pathlib.Path(repo+'/.tmp/checkpoint-notes.md').read_bytes())
print(rev,len(rows))
