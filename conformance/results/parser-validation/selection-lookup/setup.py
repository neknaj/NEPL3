from pathlib import Path
import subprocess,io,tarfile,json,hashlib
R=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-tree-selection-lookup'
rows=[]
for name,rev in [('before','b41cb03'),('after','e7124f9')]:
 rev=subprocess.check_output(['git','-C',repo,'rev-parse',rev],timeout=60).decode().strip()
 W=R/name;assert not W.exists();W.mkdir()
 b=subprocess.check_output(['git','-C',repo,'archive','--format=tar',rev],timeout=60)
 with tarfile.open(fileobj=io.BytesIO(b)) as t:t.extractall(W,filter='data')
 path='crates/foundation/engine/tests/package/portable.rs';p=W/path
 old=p.read_bytes();new=old+b'\n'+(R/'probe.rs').read_bytes()+b'\n'+(R/'raw-probe.rs').read_bytes();p.write_bytes(new)
 rows.append(dict(name=name,commit=rev,archive_sha256=hashlib.sha256(b).hexdigest(),test_path=path,test_original_sha256=hashlib.sha256(old).hexdigest(),test_after_sha256=hashlib.sha256(new).hexdigest()))
(R/'sources.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
print('ready')
