import hashlib,json,pathlib,re,subprocess,sys
p=pathlib.Path(__file__).resolve().parent
repo='C:/projects/NEPL3-reader-checkpoints'
source=json.loads((p/'sources.json').read_text(encoding='utf-8'))
for item in source['files']:
    assert hashlib.sha256((p/'workspace'/item['path']).read_bytes()).hexdigest()==item['sha256'],item['path']
changed=subprocess.check_output(['git','-C',repo,'diff','--name-only','7318076','92d7cd2']).decode().splitlines()
for path in changed:
    target=p/'reviewed-source'/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((p/'workspace'/path).read_bytes())
for path in ['review_helpers.rs','review_checkpoints.rs','review_frame_work.rs']:
    target=p/'probes'/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((p/'workspace/crates/foundation/reader/tests'/path).read_bytes())
versions={command:subprocess.check_output([command,'--version']).decode().strip() for command in ['rustc','cargo','wasmtime']}
versions['python']=sys.version
artifacts=[]
for base in [p/'target/debug/deps',p/'wasm-target/wasm32-wasip2/debug/deps',p/'before/target/debug/deps',p/'before/wasm-target/wasm32-wasip2/debug/deps']:
    for f in sorted(base.glob('*')):
        if f.suffix not in ['.exe','.wasm'] or not f.name.startswith(('runtime-','review_checkpoints-','review_frame_work-')):continue
        artifacts.append(dict(path=str(f.relative_to(p)).replace('\\','/'),sha256=hashlib.sha256(f.read_bytes()).hexdigest(),bytes=f.stat().st_size))
executions={f.stem:json.loads(f.read_text(encoding='utf-8')) for f in p.glob('*.json') if f.stem.startswith(('native-','wasi-'))}
assert executions['native-all']['exit_code']==executions['wasi-all']['exit_code']==0
assert json.loads((p/'before/native-growing.json').read_text())['exit_code']==101
assert json.loads((p/'before/wasi-growing.json').read_text())['exit_code']==3
evidence=dict(production_commit=source['commit'],base='73180760a888a193f88d9801ce86313d98f724d0',source_files_verified=len(source['files']),versions=versions,artifacts=artifacts,executions=executions,status='semantics checked; owned frame conversion Work precharge correction pending')
(p/'evidence-before.json').write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8',newline='\n')
selected=[]
for f in sorted(p.rglob('*')):
    if not f.is_file():continue
    rel=f.relative_to(p)
    if any(part in ['workspace','target','wasm-target','__pycache__'] for part in rel.parts):continue
    if f.name.startswith('manifest'):continue
    selected.append(dict(path=str(rel).replace('\\','/'),sha256=hashlib.sha256(f.read_bytes()).hexdigest(),bytes=f.stat().st_size))
(p/'manifest-before.json').write_text(json.dumps(dict(scope='fixed92d7cd2 independent review before Work correction',files=selected),indent=2)+'\n',encoding='utf-8',newline='\n')
print(len(selected),hashlib.sha256((p/'manifest-before.json').read_bytes()).hexdigest())
