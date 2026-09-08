from pathlib import Path
import subprocess,json,hashlib,re
r=Path(__file__).resolve().parent
repo='C:/projects/NEPL3-source-map-ordered-locality'
def git(*args):return subprocess.check_output(['git','-C',repo,*args])
for name,rev in [('source','4be6b34'),('before','58d622c')]:
 head=git('rev-parse',rev).decode().strip();dest=r/name;dest.mkdir(exist_ok=True);files=[]
 paths=git('ls-tree','-r','--name-only',head,'crates/foundation/core','Cargo.toml','Cargo.lock','rust-toolchain.toml').decode().splitlines()
 for path in paths:
  data=git('show',head+':'+path);files.append(dict(path=path,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
  p=dest/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
 # Review workspace contains only the unchanged core crate and a probe. No complete repository copy.
 p=dest/'Cargo.toml';p.write_text(re.sub(r'members = \[[^\n]+', 'members = ["crates/foundation/core", "probe"]',p.read_text(encoding='utf-8')),encoding='utf-8',newline='\n')
 p=dest/'probe';(p/'tests').mkdir(parents=True,exist_ok=True)
 (p/'Cargo.toml').write_text('[package]\nname="revision-probe"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nnepl3-core={path="../crates/foundation/core"}\n',encoding='utf-8')
 (p/'tests/revision.rs').write_bytes((r/'revision.rs').read_bytes())
 (r/(name+'-manifest.json')).write_text(json.dumps(dict(commit=head,files=files),indent=2)+'\n',encoding='utf-8')
 print(name,head,len(files))
(r/'delta.diff').write_bytes(git('diff','58d622c','4be6b34'))
