from pathlib import Path
import hashlib,json
out=Path(__file__).resolve().parent;files=[]
for p in sorted(out.rglob('*')):
    if p.is_file() and p.name!='manifest.json':
        b=p.read_bytes();files.append({'path':p.relative_to(out).as_posix(),'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)})
p=out/'manifest.json';p.write_text(json.dumps({'scope':'author self-check for non-Doc resource prose; no runtime or independent review','files':files},indent=2)+'\n',encoding='utf-8')
print(hashlib.sha256(p.read_bytes()).hexdigest())
