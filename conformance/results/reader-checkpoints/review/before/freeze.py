import hashlib,io,json,pathlib,subprocess,zipfile
p=pathlib.Path(__file__).resolve().parent
repo='C:/projects/NEPL3-reader-checkpoints'
rev=subprocess.check_output(['git','-C',repo,'rev-parse','7318076']).decode().strip()
top=['Cargo.toml','Cargo.lock','crates','tools','interfaces','design','languages','examples','doc','conformance/fixtures','AGENTS.md']
raw=subprocess.check_output(['git','-C',repo,'archive','--format=zip',rev,*top]); rows=[]
with zipfile.ZipFile(io.BytesIO(raw)) as z:
    for item in z.infolist():
        if item.is_dir():continue
        q=p/'workspace'/item.filename;q.parent.mkdir(parents=True,exist_ok=True);data=z.read(item);q.write_bytes(data)
        rows.append(dict(path=item.filename,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
(p/'sources.json').write_text(json.dumps(dict(commit=rev,files=rows),indent=2)+'\n',encoding='utf-8',newline='\n')
tests=p/'workspace/crates/foundation/reader/tests'
raw=(tests/'runtime.rs').read_bytes();(tests/'runtime.rs').write_bytes(raw+b'\n#[path = "runtime/checkpoints.rs"]\nmod checkpoints;\n')
new=p.parent/'workspace/crates/foundation/reader/tests/runtime/checkpoints.rs'
(tests/'runtime/checkpoints.rs').write_bytes(new.read_bytes())
(p/'run.py').write_bytes((p.parent/'run.py').read_bytes())
