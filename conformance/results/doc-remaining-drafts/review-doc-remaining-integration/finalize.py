from pathlib import Path
import subprocess,sys,json,hashlib
R=Path(__file__).resolve().parent
p=subprocess.run([sys.executable,str(R/'check.py')],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60)
(R/'check.log').write_bytes(p.stdout);assert p.returncode==0,p.stdout.decode(errors='replace')
files=[]
for p in sorted(R.rglob('*')):
 if not p.is_file() or p==R/'manifest.json':continue
 b=p.read_bytes();files.append(dict(path=p.relative_to(R).as_posix(),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
b=(json.dumps({'target':'5a0b72f93dad54848f5616347bae59cf1be2a345','files':files},indent=2)+'\n').encode()
(R/'manifest.json').write_bytes(b)
print(json.dumps({'entries':len(files),'bytes':sum(f['bytes'] for f in files),'sha256':hashlib.sha256(b).hexdigest()}))
