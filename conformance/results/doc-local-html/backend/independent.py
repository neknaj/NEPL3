from pathlib import Path
import os,subprocess,time,json,hashlib,shutil
D=Path(__file__).resolve().parent;T=D.parent/'review-fixed-target';env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run';runs=[]
def run(label,cmd,timeout=600):
 start=time.time();p=subprocess.run([str(x)for x in cmd],cwd=D/'workspace',env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout);log=D/(label+'.log');log.write_bytes(p.stdout);runs.append(dict(label=label,command=[str(x)for x in cmd],exit=p.returncode,seconds=time.time()-start,deadline=timeout,log=str(log),sha256=hashlib.sha256(p.stdout).hexdigest()));(D/'independent-runs.json').write_text(json.dumps(runs,indent=2)+'\n',encoding='utf-8');print(label,p.returncode,flush=True)
 if p.returncode:print(p.stdout.decode('utf-8',errors='replace')[-8000:],flush=True);raise SystemExit(p.returncode)
for kind,args,sub,ext in [('native',[],'debug','.exe'),('wasi',['--target','wasm32-wasip2'],'wasm32-wasip2/debug','.wasm')]:
 run('independent-build-'+kind,['cargo','build','--offline','--manifest-path',D/'probe/Cargo.toml','--bins','--target-dir',T,*args])
 for name in ['independent-doc-local-html','source']:
  artifact=D/(kind+'-'+name+ext);shutil.copyfile(T/sub/(name+ext),artifact);run(kind+'-'+name,[artifact]if kind=='native'else['wasmtime','run',artifact],180)
