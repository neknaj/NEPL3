from pathlib import Path
import subprocess,hashlib,json,shutil
r=Path(__file__).resolve().parent;repo=r.parents[1]
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
imports=[('5eeba8f','doc/migration/authored/project/SECURITY.nepld'),('efa4847','CONTRIBUTING.md'),('efa4847','doc/migration/authored/project/CONTRIBUTING.nepld')]
records=[]
for rev,path in imports:
 b=git('show',rev+':'+path);p=repo/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b)
 frozen=r/'before'/path;frozen.parent.mkdir(parents=True,exist_ok=True);frozen.write_bytes(b)
 records.append(dict(commit=git('rev-parse',rev).decode().strip(),path=path,sha256=hashlib.sha256(b).hexdigest(),bytes=len(b)))
for path in ['SECURITY.md','CONTRIBUTING.md','README.md','implementation-status.json','AGENTS.md','doc/authoring.md']:
 b=git('show','2b48d5:'+path);p=r/'base'/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b)
 records.append(dict(commit=git('rev-parse','2b48d5').decode().strip(),path=path,sha256=hashlib.sha256(b).hexdigest(),bytes=len(b)))
(r/'inputs.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
shutil.copyfile('C:/projects/NEPL3-doc-guide-refresh/.tmp/authoring-refresh/check.py',r/'check.py')
