from pathlib import Path
import json,zipfile,subprocess,hashlib
p=Path(__file__).resolve().parent;r=Path('C:/projects/NEPL3-snapshot-storage');w=p/'source';w.mkdir(exist_ok=True)
base=subprocess.check_output(['git','rev-parse','HEAD'],cwd=r,text=True).strip();subprocess.run(['git','archive','--format=zip','--output',str(p/'base.zip'),base],cwd=r,check=True)
with zipfile.ZipFile(p/'base.zip') as z:z.extractall(w)
name='crates/foundation/core/src/source.rs';(p/'source-before.rs').write_bytes((w/name).read_bytes());data=(r/name).read_bytes();(w/name).write_bytes(data);sha=hashlib.sha256(data).hexdigest();assert (r/name).read_bytes()==data
(p/'snapshot.json').write_text(json.dumps(dict(base=base,files=[dict(path=name,sha256=sha)]),indent=2),encoding='utf-8',newline='\n')
probe=p/'probe';probe.mkdir(exist_ok=True)
(probe/'Cargo.toml').write_text('[package]\nname="independent-admission-storage-cache"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[lib]\npath="root.rs"\n[dependencies]\nnepl3-core={path="'+(w/'crates/foundation/core').as_posix()+'"}\nnepl3-wire={path="'+(w/'crates/foundation/wire').as_posix()+'"}\n',encoding='utf-8',newline='\n')
print(base,sha)
