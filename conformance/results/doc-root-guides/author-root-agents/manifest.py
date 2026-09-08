from pathlib import Path
import hashlib,json
r=Path(__file__).parent
entries=[]
for p in sorted(r.iterdir()):
 if p.is_file() and p.name!='manifest.json':
  b=p.read_bytes();entries.append({'path':p.name,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(r/'manifest.json').write_text(json.dumps({'files':entries},indent=2)+'\n',encoding='utf-8');print(hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest())
