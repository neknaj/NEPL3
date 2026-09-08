from pathlib import Path
import subprocess,tarfile,io,json,hashlib
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-source-index-locality'
for name,rev in [('source','58d622c'),('before','79f35ca')]:
    head=subprocess.check_output(['git','-C',repo,'rev-parse',rev]).decode().strip();dest=r/name;dest.mkdir(exist_ok=True)
    t=tarfile.open(fileobj=io.BytesIO(subprocess.check_output(['git','-C',repo,'archive',head])))
    for m in t.getmembers():assert (dest/m.name).resolve().is_relative_to(dest.resolve()) and not m.issym() and not m.islnk()
    t.extractall(dest,filter='data');files=[]
    for p in sorted(dest.rglob('*')):
        if p.is_file():
            b=p.read_bytes();files.append(dict(path=p.relative_to(dest).as_posix(),sha256=hashlib.sha256(b).hexdigest(),bytes=len(b)))
    (r/(name+'-manifest.json')).write_text(json.dumps(dict(commit=head,files=files),indent=2)+'\n',encoding='utf-8')
    probe=r/(name+'-probe');(probe/'tests').mkdir(parents=True,exist_ok=True)
    (probe/'Cargo.toml').write_text('[package]\nname="source-tail-probe"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\nnepl3-core={path="../'+name+'/crates/foundation/core"}\n',encoding='utf-8')
    (probe/'tests/tail.rs').write_bytes((r/'tail.rs').read_bytes())
    print(name,head,len(files))
(r/'delta.diff').write_bytes(subprocess.check_output(['git','-C',repo,'diff','79f35ca','58d622c']))
