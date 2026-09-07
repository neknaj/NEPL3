from pathlib import Path
import subprocess,os,time,json
D=Path(__file__).resolve().parent;W=D/'workspace';T=D.parent/'review-pages-target';env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run';runs=[]
for kind,args in [('native',[]),('wasi',['--target','wasm32-wasip2'])]:
 for package,test,fil in [('nepl3-doc-core','pages',[]),('nepl3-tools','doc',['pages::'])]:
  label=kind+'-'+package;cmd=['cargo','test','--offline','--locked','--manifest-path',str(W/'Cargo.toml'),'--target-dir',str(T),'-p',package,'--test',test,*fil,*args,'--','--nocapture','--test-threads=1'];start=time.time();p=subprocess.run(cmd,cwd=W,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=1200);(D/(label+'.log')).write_bytes(p.stdout);runs.append({'command':cmd,'exit':p.returncode,'seconds':time.time()-start,'deadline':1200});(D/'managed-runs.json').write_text(json.dumps(runs,indent=2),encoding='utf-8');print(label,p.returncode,flush=True)
  if p.returncode:print(p.stdout.decode(errors='replace')[-4000:]);raise SystemExit(p.returncode)
