import ast,hashlib,io,json,pathlib,re,subprocess,zipfile
p=pathlib.Path(__file__).resolve().parent
repo='C:/projects/NEPL3-reader-checkpoints'
commit=json.loads((p/'commit.json').read_text())['commit']
top=['Cargo.toml','Cargo.lock','crates','tools','interfaces','design','languages','examples','doc','conformance/fixtures','AGENTS.md']
archive=subprocess.check_output(['git','-C',repo,'archive','--format=zip',commit,*top]);sources=[]
with zipfile.ZipFile(io.BytesIO(archive)) as z:
    for item in z.infolist():
        if item.is_dir():continue
        data=z.read(item);assert data==(p/'workspace'/item.filename).read_bytes(),item.filename
        sources.append(dict(path=item.filename,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
(p/'sources.json').write_text(json.dumps(dict(commit=commit,files=sources),indent=2)+'\n',encoding='utf-8',newline='\n')
for name in ['review_frame_work.rs','review_checkpoints.rs','review_helpers.rs']:
    assert (p/'workspace/crates/foundation/reader/tests'/name).read_bytes()==(p.parent/'probes'/name).read_bytes(),name
results={}
for target in ['native','wasi']:
    label=target+'-all-after'
    run=json.loads((p/(label+'.json')).read_text());assert run['exit_code']==0
    log=(p/(label+'.log')).read_text(encoding='utf-8')
    count=sum(int(n) for n in re.findall(r'test result: ok\. (\d+) passed',log));assert count==62
    before=(p.parent/(target+'-frame-work-before-final.log')).read_text(encoding='utf-8')
    pattern=r'depth=(\d+) full_resume=\((\d+), (\d+), true\) largest_adjacent_work_allocation_jump=\((\d+), (\d+), (\d+), (\d+), false, (\d+)\)'
    old=[[int(n) for n in row] for row in re.findall(pattern,before)]
    new=[[int(n) for n in row] for row in re.findall(pattern,log)]
    assert len(old)==len(new)==2
    for a,b in zip(old,new):
        assert a[0]==b[0] and b[1]-a[1]==a[0] and a[2]==b[2] and b[3]-a[3]==a[0]
        assert a[4:7]==b[4:7] and a[7]==1 and b[7]==b[0]+1
    results[target]=dict(total_passed=count,managed=59,independent_semantics=2,independent_observation=1,before=old,after=new)
artifacts=[]
for folder in [p/'target/debug/deps',p/'wasm-target/wasm32-wasip2/debug/deps']:
    for f in sorted(folder.glob('*')):
        if f.suffix in ['.exe','.wasm'] and f.name.startswith(('runtime-','review_checkpoints-','review_frame_work-')):
            artifacts.append(dict(path=str(f.relative_to(p)).replace('\\','/'),bytes=f.stat().st_size,sha256=hashlib.sha256(f.read_bytes()).hexdigest()))
(p/'evidence.json').write_text(json.dumps(dict(commit=commit,production_files_verified=len(sources),results=results,artifacts=artifacts,blocking=False),indent=2)+'\n',encoding='utf-8',newline='\n')
before_manifest=p.parent/'manifest-before.json'
before=json.loads(before_manifest.read_text())
for row in before['files']:
    assert hashlib.sha256((p.parent/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
files=[]
for f in sorted(p.rglob('*')):
    if not f.is_file():continue
    rel=f.relative_to(p)
    if any(part in ['workspace','target','wasm-target','__pycache__'] for part in rel.parts) or f.name=='manifest.json':continue
    files.append(dict(path=str(rel).replace('\\','/'),sha256=hashlib.sha256(f.read_bytes()).hexdigest(),bytes=f.stat().st_size))
(p/'manifest.json').write_text(json.dumps(dict(scope='0c583b7 correction and final independent review',before_manifest='../manifest-before.json',before_sha256=hashlib.sha256(before_manifest.read_bytes()).hexdigest(),files=files),indent=2)+'\n',encoding='utf-8',newline='\n')
print(len(files),hashlib.sha256((p/'manifest.json').read_bytes()).hexdigest())
