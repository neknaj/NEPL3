from pathlib import Path
import subprocess,shutil,json
D=Path('C:/projects/NEPL3-runtime/.tmp/review-doc-export');T=D.parent/'review-shell-target';runs=[]
for label,args,sub,ext in [('native',[],'debug','.exe'),('wasi',['--target','wasm32-wasip2'],'wasm32-wasip2/debug','.wasm')]:
 cmd=['cargo','build','--offline','--manifest-path',str(D/'shell-probe/Cargo.toml'),'--target-dir',str(T),*args];p=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=300);(D/('shell-fixed-build-'+label+'.log')).write_bytes(p.stdout);assert p.returncode==0,p.stdout.decode(errors='replace');out=D/('shell-queue-fixed-'+label+ext);shutil.copyfile(T/sub/('independent-shell-queue'+ext),out);cmd=[str(out)]if label=='native'else['wasmtime','run',str(out)];p=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=30);(D/('shell-queue-fixed-'+label+'.log')).write_bytes(p.stdout);runs.append({'command':cmd,'exit':p.returncode,'deadline':30});assert p.returncode==0;print(p.stdout.decode(),flush=True)
(D/'shell-fixed-runs.json').write_text(json.dumps(runs,indent=2),encoding='utf-8')
