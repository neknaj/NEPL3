from pathlib import Path
import subprocess,json,hashlib,sys
root=Path(__file__).resolve().parent
p=subprocess.run([sys.executable,str(root/'check.py')],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60)
(root/'check.log').write_bytes(p.stdout)
assert p.returncode==0,p.stdout.decode(errors='replace')
(root/'python-version.txt').write_bytes(subprocess.check_output([sys.executable,'--version'],timeout=60))
entries=[]
for f in sorted(root.rglob('*')):
    if not f.is_file() or '__pycache__' in f.parts or f.name=='manifest.json':continue
    b=f.read_bytes();entries.append(dict(path=f.relative_to(root).as_posix(),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
b=(json.dumps(dict(head='1fee084e3bff960525ac87cbf6180a0af0caa962',files=entries),ensure_ascii=False,indent=2)+'\n').encode()
(root/'manifest.json').write_bytes(b)
print(json.dumps(dict(entries=len(entries),bytes=sum(x['bytes'] for x in entries),manifest_sha256=hashlib.sha256(b).hexdigest())))
