from pathlib import Path
import subprocess,tarfile,io,json,hashlib
R=Path(__file__).resolve().parent;repo=Path('C:/projects/NEPL3-doc-page-preparation-reuse')
after=subprocess.check_output(['git','-C',str(repo),'rev-parse','8b6088e']).decode().strip()
before=subprocess.check_output(['git','-C',str(repo),'rev-parse',after+'^']).decode().strip()
rows=[]
for name,rev in [('before',before),('after',after)]:
 W=R/name;assert not W.exists()
 raw=subprocess.check_output(['git','-C',str(repo),'archive','--format=tar',rev],timeout=60);W.mkdir()
 with tarfile.open(fileobj=io.BytesIO(raw)) as tar:tar.extractall(W,filter='data')
 rows.append(dict(name=name,commit=rev,archive_sha256=hashlib.sha256(raw).hexdigest()))
(R/'sources.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
(R/'delta.diff').write_bytes(subprocess.check_output(['git','-C',str(repo),'diff',before,after],timeout=60))
print(json.dumps(rows))
