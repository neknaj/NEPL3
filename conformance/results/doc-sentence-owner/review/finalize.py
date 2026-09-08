import hashlib,json,pathlib,subprocess
p=pathlib.Path(__file__).resolve().parent
source=json.loads((p/'sources.json').read_text(encoding='utf-8'))
repo='C:/projects/NEPL3-doc-literal-source';rev=source['commit']
tree={}
for line in subprocess.check_output(['git','ls-tree','-r',rev],cwd=repo).decode().splitlines():
    meta,path=line.split('\t',1);tree[path]=meta.split()[2]
for row in source['files']:
    raw=(p/'workspace'/row['path']).read_bytes()
    assert hashlib.sha256(raw).hexdigest()==row['sha256'],row['path']
    assert hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()==tree[row['path']],row['path']
artifacts=[]
for target,ext in [('debug/deps','exe'),('wasm32-wasip2/debug/deps','wasm')]:
    for name in ['review_sentence_payload','doc','portable']:
        for q in sorted((p/'target'/target).glob(name+'-*.'+ext)):
            artifacts.append(dict(path=str(q),sha256=hashlib.sha256(q.read_bytes()).hexdigest(),bytes=q.stat().st_size))
versions={tool:subprocess.check_output([tool,'--version']).decode().strip() for tool in ['rustc','cargo','wasmtime','python']}
logs={name:json.loads((p/(name+'.json')).read_text(encoding='utf-8')) for name in ['native-independent','wasi-independent','native-managed','wasi-managed','native-managed-portable','wasi-managed-portable']}
assert all(v['exit_code']==0 for v in logs.values())
(p/'evidence.json').write_text(json.dumps(dict(commit=rev,source_files_verified=len(source['files']),git_blob_bytes_verified=True,versions=versions,executions=logs,artifacts=artifacts),indent=2)+'\n',encoding='utf-8',newline='\n')
print('verified',len(source['files']),'sources;',len(artifacts),'artifacts')
