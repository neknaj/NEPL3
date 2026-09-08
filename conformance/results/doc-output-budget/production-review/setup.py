from pathlib import Path
import subprocess,tarfile,io
r=Path(__file__).parent;repo=r.parents[1]
for name,commit in [('workspace','b84e9a3'),('before','244c15f')]:
 b=subprocess.check_output(['git','-C',str(repo),'archive',commit]);(r/(name+'.tar')).write_bytes(b)
 with tarfile.open(fileobj=io.BytesIO(b))as t:t.extractall(r/name,filter='data')
 p=r/name/'tools/tests/doc/export.rs'
 with p.open('a',encoding='utf-8',newline='\n')as f:f.write((r/'probe.rs').read_text(encoding='utf-8'))
