from pathlib import Path
import subprocess,os,json,hashlib
root=Path(__file__).resolve().parent
result=subprocess.run(['python',str(root/'check.py')],cwd=root.parents[1],env={**os.environ,'PYTHONIOENCODING':'utf-8'},stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
(root/'check.log').write_bytes(result.stdout)
assert result.returncode==0,result.stdout
repo=Path('C:/projects/NEPL3-doc-authoring-b')
for s in json.loads((root/'sources.json').read_text(encoding='utf-8')):
    data=(root/s['path']).read_bytes()
    assert hashlib.sha256(data).hexdigest()==s['sha256']
    assert data==subprocess.check_output(['git','-C',str(repo),'show',s['source_commit']+':'+s['source_path']])
files=[]
for p in sorted(root.rglob('*')):
    if p.is_file() and '__pycache__' not in p.parts and p.name!='manifest.json':
        data=p.read_bytes();files.append({'path':str(p.relative_to(root)).replace('\\','/'),'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
manifest={'result':'no additional blocking manuscript finding','review_type':'independent content and structure only','production_runtime_executed':False,'url_reachability_checked':False,'canonical_migration_completed':False,'fixed_git_blobs_reverified':True,'files':files}
out=root/'manifest.json';out.write_bytes((json.dumps(manifest,indent=2)+'\n').encode())
print(str(out));print(hashlib.sha256(out.read_bytes()).hexdigest());print('files',len(files))
