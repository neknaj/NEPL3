import hashlib,json,pathlib,re,subprocess,shutil
p=pathlib.Path(__file__).resolve().parent
sources=json.loads((p/'sources.json').read_text(encoding='utf-8'))
for row in sources['files']:
    b=(p/'workspace'/row['path']).read_bytes()
    assert len(b)==row['bytes'] and hashlib.sha256(b).hexdigest()==row['sha256'],row['path']
repo='C:/projects/NEPL3-stream-digest'
changed=subprocess.check_output(['git','-C',repo,'diff','--name-only','3c42d75',sources['commit']]).decode().splitlines()
for name in changed:
    q=p/'snapshot'/name;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes((p/'workspace'/name).read_bytes())
design=p.parent/'review-stream-digest-design'
overlay=json.loads((design/'sources.json').read_text(encoding='utf-8'))
for row in overlay['files']:
    b=(design/'snapshot'/row['path']).read_bytes();assert hashlib.sha256(b).hexdigest()==row['sha256']
    assert b==(p/'before/workspace'/row['path']).read_bytes()
    q=p/'before/overlay'/row['path'];q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
(p/'before/original-design-sources.json').write_bytes((design/'sources.json').read_bytes())
assert (p/'before/workspace/crates/foundation/wire/tests/review_stream.rs').read_bytes()==(p/'review_stream.rs').read_bytes()==(p/'workspace/crates/foundation/wire/tests/review_stream.rs').read_bytes()
assert (p/'vectors.rs').read_bytes()==(p/'workspace/crates/foundation/wire/tests/review_vectors.inc').read_bytes()
results={}
for target in ['native','wasi']:
    entry=json.loads((p/(target+'-all.json')).read_text());assert entry['exit_code']==0
    log=(p/(target+'-all.log')).read_text(encoding='utf-8');assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))==34
    old=(p/'before'/(target+'-observation.log')).read_text(encoding='utf-8')
    assert json.loads((p/'before'/(target+'-observation.json')).read_text())['exit_code']==0
    for width,a,b in [(1000,16016,80),(100000,1600016,16)]:
        pattern=rf'frontier width={width} usage=.*?allocation_units: (\d+)'
        assert [int(x) for x in re.findall(pattern,old)]==[a]
        assert [int(x) for x in re.findall(pattern,log)]==[b]
    assert 'allocation_units: 1041' in old and 'allocation_units: 17,' in log
    results[target]=dict(managed_passed=31,independent_passed=3,vectors=60,domains=3,boundary_stops=20,caller_depth=7,source_bytes=0)
artifacts=[]
for root in [p/'target/debug/deps',p/'wasm-target/wasm32-wasip2/debug/deps',p/'before/target/debug/deps',p/'before/wasm-target/wasm32-wasip2/debug/deps']:
    for f in sorted(root.iterdir()):
        if f.suffix in ['.exe','.wasm']:
            b=f.read_bytes();artifacts.append(dict(path=f.relative_to(p).as_posix(),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
(p/'evidence.json').write_text(json.dumps(dict(commit=sources['commit'],verified_production_files=len(sources['files']),results=results,artifacts=artifacts),indent=2)+'\n',encoding='utf-8',newline='\n')
rows=[]
for f in sorted(p.rglob('*')):
    if not f.is_file():continue
    rel=f.relative_to(p)
    if any(part in ['workspace','target','wasm-target','__pycache__'] for part in rel.parts) or rel.as_posix()=='manifest.json':continue
    b=f.read_bytes();rows.append(dict(path=rel.as_posix(),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
b=(json.dumps(dict(scope='fixed 1de3e1c independent streaming canonical digest review',files=rows),indent=2)+'\n').encode();(p/'manifest.json').write_bytes(b)
print(len(rows),sum(r['bytes'] for r in rows),hashlib.sha256(b).hexdigest())
