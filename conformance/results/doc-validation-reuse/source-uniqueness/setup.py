from pathlib import Path
import subprocess,tarfile,io
r=Path(__file__).parent;repo=r.parents[1]
for name,commit in [('workspace','13b2155'),('before','8b6088e')]:
 data=subprocess.check_output(['git','-C',str(repo),'archive',commit]);(r/(name+'.tar')).write_bytes(data)
 with tarfile.open(fileobj=io.BytesIO(data)) as t:t.extractall(r/name,filter='data')
 test=r/name/'crates/languages/doc/core/tests/portable.rs'
 with test.open('a',encoding='utf-8',newline='\n') as f:f.write((r/'probe.rs').read_text(encoding='utf-8'))
