import hashlib,json,pathlib,subprocess
base=pathlib.Path(__file__).resolve().parent
sources=json.loads((base/'sources.json').read_text(encoding='utf-8'))
for row in sources['files']:
    assert hashlib.sha256((base/'workspace'/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
versions={name:subprocess.check_output([name,'--version']).decode().strip() for name in ['rustc','cargo','wasmtime']}
files=[f for f in sorted(base.iterdir()) if f.is_file() and f.name!='manifest.json']
files += [base/'workspace/tools/tests/review_proof.rs',base/'workspace/tools/tests/review_lifetime.rs']
artifacts=[]
for target,ext in [('debug','exe'),('wasm32-wasip2/debug','wasm')]:
    for prefix in ['review_proof-','doc-']:
        for file in sorted((base/'workspace/target'/target/'deps').glob(prefix+'*.'+ext)):
            artifacts.append({'path':str(file),'sha256':hashlib.sha256(file.read_bytes()).hexdigest(),'bytes':file.stat().st_size})
result={'commit':sources['commit'],'base':sources['base'],'production_hashes_verified':len(sources['files']),'versions':versions,'scope':'retained syntax proof and three Doc tools consumers; independent1 plus managed4 each native/WASI, lifetime negative E0502','files':[{'path':f.relative_to(base).as_posix(),'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'bytes':f.stat().st_size} for f in files],'artifacts':artifacts}
output=base/'manifest.json'
output.write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
print(hashlib.sha256(output.read_bytes()).hexdigest())
