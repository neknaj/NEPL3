import hashlib,json,pathlib,subprocess
p=pathlib.Path(__file__).resolve().parent
s=json.loads((p/'sources.json').read_text(encoding='utf-8'))
for r in s['files']:
    assert hashlib.sha256((p/'workspace'/r['path']).read_bytes()).hexdigest()==r['sha256'],r['path']
files=[f for f in sorted(p.iterdir()) if f.is_file() and f.name!='manifest.json']+[p/'workspace/tools/tests/review_sentences.rs']
artifacts=[]
for target,ext in [('debug','exe'),('wasm32-wasip2/debug','wasm')]:
    for prefix in ['review_sentences-','doc-']:
        for f in sorted((p/'workspace/target'/target/'deps').glob(prefix+'*.'+ext)):
            artifacts.append({'path':str(f),'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'bytes':f.stat().st_size})
m=p/'manifest.json'
m.write_text(json.dumps({'commit':s['commit'],'base':s['base'],'source_hashes_rechecked':len(s['files']),'versions':{x:subprocess.check_output([x,'--version']).decode().strip() for x in ['rustc','cargo','wasmtime']},'result':'independent2 plus managed10 native and WASI passed; no blocking finding','files':[{'path':f.relative_to(p).as_posix(),'sha256':hashlib.sha256(f.read_bytes()).hexdigest()} for f in files],'artifacts':artifacts},indent=2)+'\n',encoding='utf-8',newline='\n')
print(hashlib.sha256(m.read_bytes()).hexdigest())
