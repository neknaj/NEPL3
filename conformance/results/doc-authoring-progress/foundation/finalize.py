import hashlib,json,pathlib,subprocess,sys
p=pathlib.Path(__file__).resolve().parent
r=subprocess.run([sys.executable,'check.py'],cwd=p,capture_output=True,timeout=60)
(p/'execution.json').write_text(json.dumps(dict(command=[sys.executable,'check.py'],python=sys.version,timeout_seconds=60,exit_code=r.returncode,stdout=r.stdout.decode('utf-8'),stderr=r.stderr.decode('utf-8')),ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
assert r.returncode==0
files=[]
for f in sorted(p.rglob('*')):
 if f.is_file() and '__pycache__' not in f.parts and f.name!='manifest.json':
  b=f.read_bytes();files.append(dict(path=f.relative_to(p).as_posix(),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
b=(json.dumps(dict(scope='Independent manuscript content review only',files=files),indent=2)+'\n').encode()
(p/'manifest.json').write_bytes(b)
print(len(files),hashlib.sha256(b).hexdigest())
