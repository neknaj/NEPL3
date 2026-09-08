from pathlib import Path
import json,hashlib,subprocess
r=Path(__file__).resolve().parent;repo=r.parents[1]
for p in ['CONTRIBUTING.md','doc/migration/authored/project/CONTRIBUTING.nepld']:assert (repo/p).read_bytes()==(r/'before'/p).read_bytes()
for path,source,index in [('SECURITY.md','base',2),('doc/migration/authored/project/SECURITY.nepld','before',2)]:
 old=(r/source/path).read_bytes().splitlines(keepends=True);new=(repo/path).read_bytes().splitlines(keepends=True);assert len(old)==len(new)
 assert [i for i,(a,b) in enumerate(zip(old,new)) if a!=b]==[index]
files=['CONTRIBUTING.md','SECURITY.md','doc/migration/authored/project/CONTRIBUTING.nepld','doc/migration/authored/project/SECURITY.nepld']
rows=[]
for p in files:
 b=(repo/p).read_bytes();assert b'\r' not in b and not b.startswith(b'\xef\xbb\xbf');rows.append(dict(path=p,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
(r/'final-files.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
print('Exact imports, one SECURITY sentence per representation, UTF-8 LF: PASS')
