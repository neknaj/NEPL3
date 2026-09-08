from pathlib import Path
import subprocess,json,hashlib,re
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-reader-owned-failure'
def git(*a):return subprocess.check_output(['git','-C',repo,*a])
for name,rev in [('source','be6aa8b'),('before','0e0b365')]:
 head=git('rev-parse',rev).decode().strip();dest=r/name;dest.mkdir(exist_ok=True);files=[]
 paths=git('ls-tree','-r','--name-only',head,'crates/foundation/core','crates/foundation/reader','crates/foundation/wire','Cargo.toml','Cargo.lock','rust-toolchain.toml').decode().splitlines()
 for path in paths:
  data=git('show',head+':'+path);files.append(dict(path=path,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)));p=dest/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
 p=dest/'Cargo.toml';p.write_text(re.sub(r'members = \[[^\n]+','members = ["crates/foundation/core", "crates/foundation/wire", "crates/foundation/reader"]',p.read_text(encoding='utf-8')),encoding='utf-8',newline='\n')
 (r/(name+'-manifest.json')).write_text(json.dumps(dict(commit=head,files=files),indent=2)+'\n',encoding='utf-8');print(name,head,len(files))
(r/'delta.diff').write_bytes(git('diff','0e0b365','be6aa8b'))
