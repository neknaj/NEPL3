from pathlib import Path
import hashlib,json,subprocess,re,tarfile
r=Path(__file__).parent; root=r.parent.parent
head='9b11ed30dd797d8a9fad8bf3b1765d91299de5d5';base='5c7f4f3b087c0147fda0be9c8db7e9d6bae1e0f3'
sha=lambda d:hashlib.sha256(d).hexdigest()
files=[]
with tarfile.open(r/'source.tar') as t:
    for member in t.getmembers():
        if member.isfile():
            data=t.extractfile(member).read()
            assert (r/'pristine'/member.name).read_bytes()==data,member.name
            files.append(member.name)
changed=subprocess.check_output(['git','diff','--name-only',base,head],cwd=root,text=True).splitlines()
sources=[dict(path=n,bytes=len((r/'pristine'/n).read_bytes()),sha256=sha((r/'pristine'/n).read_bytes())) for n in changed]
results={}
for name in ['native','wasi','probe-native','probe-wasi']:
    data=(r/(name+'.log')).read_bytes();text=data.decode('utf-16' if data.startswith(b'\xff\xfe') else 'utf-8')
    assert 'test result: FAILED' not in text and not re.search(r'^error:',text,re.M),name
    results[name]=dict(exit_code=0,passed=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',text))))
result=dict(head=head,base=base,source_archive_sha256=sha((r/'source.tar').read_bytes()),
    all_pristine_files_verified=len(files),changed_sources=sources,results=results,
    toolchain=subprocess.check_output(['rustc','--version'],text=True).strip(),runner=subprocess.check_output(['wasmtime','--version'],text=True).strip(),
    verdict='no new blocking findings; previous integrity finding corrected',
    commands=['cargo test --locked -p nepl3-core -p nepl3-reader -p nepl3-engine',
    'cargo test --locked -p nepl3-core -p nepl3-reader -p nepl3-engine --target wasm32-wasip2',
    'cargo test --locked -p nepl3-reader -p nepl3-engine reviewer_ -- --nocapture',
    'cargo test --locked -p nepl3-reader -p nepl3-engine reviewer_ --target wasm32-wasip2 -- --nocapture'],
    wasi_runner='wasmtime run',production_mutation_by_reviewer=False)
(r/'sources.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
names=['review.md','sources.json','probe.py','expand.py','inject.py','reservation.py','seal.py','native.log','wasi.log','probe-native.log','probe-wasi.log']
manifest=dict(files=[dict(path=n,bytes=(r/n).stat().st_size,sha256=sha((r/n).read_bytes())) for n in names])
(r/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n')
print(results);print(sha((r/'manifest.json').read_bytes()))
