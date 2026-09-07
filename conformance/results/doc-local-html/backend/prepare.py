from pathlib import Path
import subprocess,hashlib,json
d=Path(__file__).resolve().parent;r=Path('C:/projects/NEPL3-runtime');w=d/'workspace';w.mkdir(exist_ok=True)
names=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard'],cwd=r,text=True).splitlines()
rows=[]
for name in sorted(set(names)):
    p=r/name
    if not p.is_file():continue
    data=p.read_bytes();out=w/name;out.parent.mkdir(parents=True,exist_ok=True);out.write_bytes(data)
    rows.append(dict(path=name,sha256=hashlib.sha256(data).hexdigest()))
for row in rows:assert hashlib.sha256((r/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
(d/'files.json').write_text(json.dumps(rows,indent=2),encoding='utf-8',newline='\n')
(d/'snapshot.json').write_text(json.dumps(dict(base=subprocess.check_output(['git','rev-parse','HEAD'],cwd=r,text=True).strip(),files=len(rows),input='examples/document/linear-combination.nepld',input_bytes=(w/'examples/document/linear-combination.nepld').stat().st_size),indent=2),encoding='utf-8',newline='\n')
print(len(rows))
