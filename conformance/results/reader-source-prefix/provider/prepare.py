from pathlib import Path
import subprocess,zipfile,json,hashlib,re
d=Path(__file__).resolve().parent;r=Path('C:/projects/NEPL3-snapshot-storage');s=d/'source';s.mkdir(exist_ok=True)
base=subprocess.check_output(['git','rev-parse','HEAD'],cwd=r,text=True).strip();subprocess.run(['git','archive','--format=zip','--output',str(d/'base.zip'),base],cwd=r,check=True)
with zipfile.ZipFile(d/'base.zip') as z:z.extractall(s)
rows=[]
for name in ['crates/foundation/reader/src/runtime/validate/provider.rs']:
    data=(r/name).read_bytes();(d/'provider-before.rs').write_bytes((s/name).read_bytes());p=s/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data);rows.append(dict(path=name,sha256=hashlib.sha256(data).hexdigest()))
for row in rows:assert hashlib.sha256((r/row['path']).read_bytes()).hexdigest()==row['sha256']
(d/'snapshot.json').write_text(json.dumps(dict(base=base,files=rows),indent=2),encoding='utf-8',newline='\n')
p=d/'probe';p.mkdir(exist_ok=True)
deps='\n'.join(f'{name}={{path="{(s/path).as_posix()}"}}' for name,path in [('nepl3-core','crates/foundation/core'),('nepl3-reader','crates/foundation/reader'),('nepl3-wire','crates/foundation/wire')])
(p/'Cargo.toml').write_text('[package]\nname="independent-empty-provider-closure"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[lib]\npath="root.rs"\n[dependencies]\n'+deps+'\n',encoding='utf-8',newline='\n')
original=s/'crates/foundation/reader/tests/runtime.rs';text=original.read_text(encoding='utf-8');text=re.sub(r'#\[path = "([^"]+)"\]',lambda m:f'#[path = "{(original.parent/m.group(1)).as_posix()}"]',text);text+='\n#[path="../independent.rs"]mod independent;\n';(p/'root.rs').write_text(text,encoding='utf-8',newline='\n')
print(base,len(rows))
