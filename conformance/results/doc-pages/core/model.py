from pathlib import Path
import subprocess,json,shutil
D=Path('C:/projects/NEPL3-runtime/.tmp/review-doc-pages');T=D.parent/'review-pages-model-target';runs=[]
for k,args,sub,ext in [('native',[],'debug','.exe'),('wasi',['--target','wasm32-wasip2'],'wasm32-wasip2/debug','.wasm')]:
 cmd=['cargo','build','--offline','--manifest-path',str(D/'model-probe/Cargo.toml'),'--target-dir',str(T),*args];p=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=300);(D/('model-build-'+k+'.log')).write_bytes(p.stdout);assert p.returncode==0,p.stdout.decode(errors='replace');a=D/('model-'+k+ext);shutil.copyfile(T/sub/('independent-page-model'+ext),a);cmd=[str(a)]if k=='native'else['wasmtime','run',str(a)];p=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180);(D/('model-'+k+'.log')).write_bytes(p.stdout);runs.append({'command':cmd,'exit':p.returncode,'deadline':180});(D/'model-runs.json').write_text(json.dumps(runs,indent=2),encoding='utf-8');print(k,p.returncode,flush=True)
 if p.returncode:print(p.stdout.decode(errors='replace'),flush=True);raise SystemExit(p.returncode)
