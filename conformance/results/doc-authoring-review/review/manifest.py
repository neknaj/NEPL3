import hashlib,json,pathlib
p=pathlib.Path(__file__).resolve().parent
rows=[]
for f in sorted(p.rglob('*')):
 if not f.is_file() or '__pycache__' in f.parts or f.name=='manifest.json':continue
 b=f.read_bytes();rows.append(dict(path=f.relative_to(p).as_posix(),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
out=dict(scope='Independent full authored review guide content review; original 337057d, draft 7c6f29b, root one-space correction4b549f2. No production runtime or migration acceptance.',files=rows)
b=(json.dumps(out,ensure_ascii=False,indent=2)+'\n').encode('utf-8');(p/'manifest.json').write_bytes(b)
print(len(rows),sum(x['bytes'] for x in rows),hashlib.sha256(b).hexdigest())
