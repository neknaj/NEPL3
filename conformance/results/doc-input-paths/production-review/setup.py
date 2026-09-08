from pathlib import Path
import subprocess,tarfile,io
r=Path(__file__).parent;repo=r.parents[1]
b=subprocess.check_output(['git','-C',str(repo),'archive','a6e304c']);(r/'source.tar').write_bytes(b)
with tarfile.open(fileobj=io.BytesIO(b))as t:t.extractall(r/'workspace',filter='data')
