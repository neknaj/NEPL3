from pathlib import Path
import hashlib,json,subprocess,sys
p=Path(__file__).resolve().parent
root=p.parents[1]
for name in ['result.json','github-check.json','github-request.json','aliases.json']:
    (p/('root-'+name)).write_bytes((root/'.tmp/chapter13'/name).read_bytes())
assert (root/'.tmp/chapter13/final.md').read_bytes()==(p/'generated.md').read_bytes()
(p/'root-final-result.json').write_bytes((root/'.tmp/chapter13/final-result.json').read_bytes())
req=json.loads((p/'root-github-request.json').read_text(encoding='utf-8'))
assert req['text']==(p/'generated.md').read_text(encoding='utf-8')
assert req['mode']=='gfm' and req['context']=='neknaj/NEPL3'
r=json.loads((p/'root-result.json').read_text(encoding='utf-8'))
assert r['output_sha256']==hashlib.sha256((p/'generated.md').read_bytes()).hexdigest()
run=subprocess.run([sys.executable,str(p/'check.py')],cwd=root,capture_output=True)
(p/'check.log').write_bytes(run.stdout+run.stderr)
assert run.returncode==0
result=json.loads((p/'result.json').read_text(encoding='utf-8'))
for f in result['source_files']:
    assert (root/f['source']).read_bytes()==(p/f['payload']).read_bytes(),f['source']
files=[]
for f in sorted(p.iterdir()):
    if f.is_file() and f.name!='manifest.json':
        data=f.read_bytes()
        files.append({'path':f.name,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
m={'scope':'Independent chapter13 all-notes view content review','generation_commit':r['commit'],'production_rerun':False,'browser':False,'canonical_cutover':False,'independent_check_exit':run.returncode,'source_files':result['source_files'],'files':files}
(p/'manifest.json').write_text(json.dumps(m,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(hashlib.sha256((p/'manifest.json').read_bytes()).hexdigest())
