from pathlib import Path
import hashlib,json,os,subprocess,sys
root=Path(__file__).resolve().parent
repo=Path('C:/projects/NEPL3-doc-authoring-b')
sources=json.loads((root/'sources.json').read_text(encoding='utf-8'))
for e in sources:
    data=(root/e['path']).read_bytes()
    assert len(data)==e['bytes'] and hashlib.sha256(data).hexdigest()==e['sha256']
    assert data==subprocess.check_output(['git','-C',str(repo),'show',e['source_commit']+':'+e['source_path']])
run=subprocess.run([sys.executable,str(root/'check.py')],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,env=dict(os.environ,PYTHONIOENCODING='utf-8'))
(root/'check.log').write_bytes(run.stdout)
(root/'execution.json').write_bytes((json.dumps({'command':'python check.py','exit_code':run.returncode,'source_git_blob_checks':len(sources),'production_runtime_executed':False},indent=2)+'\n').encode())
assert run.returncode==0
selected=[e['path'] for e in sources]+['freeze.py','check.py','finalize.py','sources.json','results.json','record.md','check.log','execution.json']
for c in ('11','12'):
    selected += [c+'/'+name for name in ('change.patch','normalized.json','readings.txt')]
entries=[]
for p in sorted(set(selected)):
    f=root/p;data=f.read_bytes()
    entries.append({'path':f.relative_to(root).as_posix(),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
(root/'manifest.json').write_bytes((json.dumps({'scope':'independent chapter 11/12 Ruby repair review','files':entries},indent=2)+'\n').encode())
for name in ('record.md','manifest.json'):
    print(name,hashlib.sha256((root/name).read_bytes()).hexdigest())
