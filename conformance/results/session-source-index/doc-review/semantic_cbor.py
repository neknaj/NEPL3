from pathlib import Path
import subprocess,concurrent.futures,json,hashlib
R=Path(__file__).resolve().parent
def run(label):
 w=R/label;d=R/('runs-'+label);t=R/('target-'+label)
 (w/'tools/examples/review_doc.rs').write_bytes((R/'probe.rs').read_bytes())
 commands=[['cargo','build','--locked','-p','nepl3-tools','--example','review_doc','--target-dir',str(t)],[str(t/'debug/examples/review_doc.exe'),str(R/'inputs/linear.nepld'),str(d/'semantic-cbor')]]
 rows=[]
 for i,cmd in enumerate(commands):
  path=d/f'cbor-{i}.log'
  with path.open('wb') as log:p=subprocess.run(cmd,cwd=w,stdout=log,stderr=subprocess.STDOUT,timeout=300)
  rows.append({'command':cmd,'cwd':str(w),'exit':p.returncode,'log':path.name,'log_sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
  assert p.returncode==0
 (d/'cbor-results.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf8')
 (d/'cbor-binary.json').write_text(json.dumps({'path':str(t/'debug/examples/review_doc.exe'),'sha256':hashlib.sha256((t/'debug/examples/review_doc.exe').read_bytes()).hexdigest()},indent=2)+'\n',encoding='utf8')
 return label
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as p:
 print(list(p.map(run,['before','after'])))
