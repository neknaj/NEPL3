import pathlib,shutil,hashlib,json,subprocess
p=pathlib.Path(__file__).resolve().parent
repo=pathlib.Path('C:/projects/NEPL3-reader-checkpoints')
shutil.copytree(p.parent/'workspace',p/'workspace')
files=[]
for path in ['crates/foundation/reader/src/runtime/mod.rs','crates/foundation/reader/tests/runtime/checkpoints.rs']:
    raw=(repo/path).read_bytes();target=p/'snapshot'/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(raw)
    (p/'workspace'/path).write_bytes(raw)
    assert raw==(repo/path).read_bytes()
    files.append(dict(path=path,sha256=hashlib.sha256(raw).hexdigest(),bytes=len(raw)))
(p/'delta.json').write_text(json.dumps(dict(base='92d7cd2e49f31b8ae0a3702f764e1e8b3c844bdf',files=files),indent=2)+'\n',encoding='utf-8',newline='\n')
(p/'delta.patch').write_bytes(subprocess.check_output(['git','-C',str(repo),'diff','--',*[f['path'] for f in files]]))
(p/'run.py').write_bytes((p.parent/'run.py').read_bytes())
print(hashlib.sha256((p/'delta.json').read_bytes()).hexdigest())
