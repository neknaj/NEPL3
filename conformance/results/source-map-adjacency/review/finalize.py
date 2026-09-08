import hashlib,io,json,pathlib,re,subprocess,zipfile
p=pathlib.Path(__file__).resolve().parent;repo='C:/projects/NEPL3-source-map-adjacency'
sources=json.loads((p/'sources.json').read_text());before=json.loads((p/'before/sources.json').read_text())
for row in sources['files']:
    b=(p/'workspace'/row['path']).read_bytes();assert hashlib.sha256(b).hexdigest()==row['sha256'] and len(b)==row['bytes']
paths=[r['path'] for r in sources['files']]
base_archive=subprocess.check_output(['git','-C',repo,'archive','--format=zip',before['base'],*paths])
with zipfile.ZipFile(io.BytesIO(base_archive)) as z:
    for i in z.infolist():
        if not i.is_dir():assert z.read(i)==(p/'before/workspace'/i.filename).read_bytes(),i.filename
assert (p/'review_adjacency.rs').read_bytes()==(p/'workspace/crates/foundation/core/tests/review_adjacency.rs').read_bytes()==(p/'before/workspace/crates/foundation/core/tests/review_adjacency.rs').read_bytes()
for name in ['crates/foundation/core/src/origin.rs','crates/foundation/core/tests/maps.rs']:
    q=p/'snapshot'/name;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes((p/'workspace'/name).read_bytes())
results={}
pat=r'star width=(\d+) usage=Usage \{ source_bytes: 0, work: (\d+), depth: 2, nodes: (\d+), allocation_units: (\d+), output_bytes: 0, diagnostics: 0, events: 0 \} head_size=(\d+) edge_delta=(\d+)'
for target in ['native','wasi']:
    newlog=(p/(target+'-all.log')).read_text(encoding='utf-8');oldlog=(p/'before'/(target+'-independent.log')).read_text(encoding='utf-8')
    assert json.loads((p/(target+'-all.json')).read_text())['exit_code']==0
    assert json.loads((p/'before'/(target+'-independent.json')).read_text())['exit_code']==0
    assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',newlog)))==81
    assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',oldlog)))==3
    old=[list(map(int,r)) for r in re.findall(pat,oldlog)];new=[list(map(int,r)) for r in re.findall(pat,newlog)];assert len(old)==len(new)==2
    for a,b in zip(old,new):
        width=a[0];assert a[0]==b[0] and a[2]==b[2]==width+1
        assert a[1]-b[1]==width*(width-1)
        assert b[3]-a[3]==(width+1)*b[4]+width*b[5]
    assert old[1][1]*10>=old[0][1]*27
    assert new[1][1]*10<new[0][1]*27
    assert 'pointwise_oracle valid=504 cycles=264 permutations=3 masks=256' in newlog and 'stop_caps=48' in newlog
    results[target]=dict(managed_passed=78,independent_passed=3,before=old,after=new,oracle_inputs=768,stopped_caps=48)
artifacts=[]
for root in [p/'target/debug/deps',p/'wasm-target/wasm32-wasip2/debug/deps',p/'before/target/debug/deps',p/'before/wasm-target/wasm32-wasip2/debug/deps']:
    for f in sorted(root.iterdir()):
        if f.suffix in ['.exe','.wasm']:
            b=f.read_bytes();artifacts.append(dict(path=f.relative_to(p).as_posix(),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
(p/'evidence.json').write_text(json.dumps(dict(commit=sources['commit'],base=before['base'],production_files_verified=len(paths),results=results,artifacts=artifacts),indent=2)+'\n',encoding='utf-8',newline='\n')
rows=[]
for f in sorted(p.rglob('*')):
    if not f.is_file():continue
    rel=f.relative_to(p)
    if any(part in ['workspace','target','wasm-target','__pycache__'] for part in rel.parts) or rel.as_posix()=='manifest.json':continue
    b=f.read_bytes();rows.append(dict(path=rel.as_posix(),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
b=(json.dumps(dict(files=rows),indent=2)+'\n').encode();(p/'manifest.json').write_bytes(b);print(len(rows),sum(r['bytes'] for r in rows),hashlib.sha256(b).hexdigest())
