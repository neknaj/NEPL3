from pathlib import Path
import subprocess,os,json,shutil,time
D=Path(__file__).resolve().parent;W=D/'workspace';T=D.parent/'review-pages-probe-target';runs=[]
for kind,args,sub,ext in [('native',[],'debug','.exe'),('wasi',['--target','wasm32-wasip2'],'wasm32-wasip2/debug','.wasm')]:
 cmd=['cargo','build','--offline','--manifest-path',str(D/'probe/Cargo.toml'),'--target-dir',str(T),*args];p=subprocess.run(cmd,cwd=W,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=900);(D/('build-'+kind+'.log')).write_bytes(p.stdout);runs.append({'command':cmd,'exit':p.returncode,'deadline':900});(D/'probe-runs.json').write_text(json.dumps(runs,indent=2),encoding='utf-8');print('build',kind,p.returncode,flush=True)
 if p.returncode:print(p.stdout.decode(errors='replace')[-5000:],flush=True);raise SystemExit(p.returncode)
 a=D/('actual-'+kind+ext);shutil.copyfile(T/sub/('independent-doc-pages'+ext),a);cmd=[str(a)]if kind=='native'else['wasmtime','run',str(a)];p=subprocess.run(cmd,cwd=W,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180);(D/('actual-'+kind+'.log')).write_bytes(p.stdout);runs.append({'command':cmd,'exit':p.returncode,'deadline':180});(D/'probe-runs.json').write_text(json.dumps(runs,indent=2),encoding='utf-8');print('actual',kind,p.returncode,flush=True)
 if p.returncode:print(p.stdout.decode(errors='replace')[-5000:],flush=True);raise SystemExit(p.returncode)
