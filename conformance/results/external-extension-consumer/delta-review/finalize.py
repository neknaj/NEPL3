from pathlib import Path
import subprocess,sys,json,hashlib
p=Path(__file__).resolve().parent
r=subprocess.run([sys.executable,str(p/'check.py')],capture_output=True)
(p/'check.log').write_bytes(r.stdout+r.stderr)
assert r.returncode==0
files=[]
for f in sorted(p.iterdir()):
    if f.is_file() and f.name!='manifest.json':
        b=f.read_bytes();files.append({'path':f.name,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(p/'manifest.json').write_text(json.dumps({'scope':'Independent review of Kepler guide/01/11 external extension additions only','source_commit':'75e73a79022e238ac7355c37588c37b910759398','blocking_findings':[],'independent_structure':True,'independent_runtime':False,'canonical_cutover':False,'files':files},ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(hashlib.sha256((p/'manifest.json').read_bytes()).hexdigest())
