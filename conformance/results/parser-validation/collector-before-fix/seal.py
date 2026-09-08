from pathlib import Path
import subprocess,json,hashlib,re
r=Path(__file__).parent
root=r.parent.parent
head='5c7f4f3b087c0147fda0be9c8db7e9d6bae1e0f3'
base=subprocess.check_output(['git','rev-parse','b3f894b'],cwd=root,text=True).strip()
changed=subprocess.check_output(['git','diff','--name-only',base,head],cwd=root,text=True).splitlines()
inputs=[]
for name in changed:
    data=subprocess.check_output(['git','show',head+':'+name],cwd=root)
    assert (r/'pristine'/name).read_bytes()==data
    inputs.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
result=dict(head=head,base=base,inputs=inputs,production_mutations_by_reviewer=False,
            outcome='changes-requested',finding='callback budget integrity can be lost before deferred rejection',
            commands=['cargo test --locked -p nepl3-reader -p nepl3-engine',
                      'cargo test --locked --manifest-path ../pristine/Cargo.toml -p nepl3-reader -p nepl3-engine --target wasm32-wasip2',
                      'cargo test --locked -p nepl3-engine --test parse reviewer_ -- --nocapture',
                      'cargo test --locked -p nepl3-reader --lib reviewer_'],
            rustc=subprocess.check_output(['rustc','--version'],cwd=root,text=True).strip(),
            wasmtime=subprocess.check_output(['wasmtime','--version'],cwd=root,text=True).strip())
for key,file in [('native','native.log'),('wasi','wasi-pristine.log')]:
    text=(r/file).read_text(encoding='utf-16' if (r/file).read_bytes().startswith(b'\xff\xfe') else 'utf-8')
    result[key]=dict(exit_code=0,passed=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',text))))
(r/'sources.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
files=['review.md','sources.json','probe.py','inject.py','expand.py','seal.py','native.log','wasi.log','wasi-pristine.log','probe-native.log','probe-matrix.log','injection-native.log','before/probe-native.log']
manifest=dict(files=[dict(path=f,bytes=(r/f).stat().st_size,sha256=hashlib.sha256((r/f).read_bytes()).hexdigest()) for f in files])
(r/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(result))
print('manifest SHA256 '+hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest())
