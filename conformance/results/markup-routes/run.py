from pathlib import Path
import subprocess,json,os,shutil,time
D=Path(__file__).resolve().parent;W=D/'workspace';T=D.parent/'review-routes-target';env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run';runs=[]
def run(label,cmd,timeout=600):
 p=subprocess.run([str(x)for x in cmd],cwd=W,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout);(D/(label+'.log')).write_bytes(p.stdout);runs.append({'label':label,'command':[str(x)for x in cmd],'exit':p.returncode,'deadline':timeout});(D/'runs.json').write_text(json.dumps(runs,indent=2),encoding='utf-8');print(label,p.returncode,flush=True)
 if p.returncode:print(p.stdout.decode(errors='replace')[-5000:],flush=True);raise SystemExit(p.returncode)
for kind,args,sub,ext in [('native',[],'debug','.exe'),('wasi',['--target','wasm32-wasip2'],'wasm32-wasip2/debug','.wasm')]:
 run('probe-build-'+kind,['cargo','build','--offline','--manifest-path',D/'probe/Cargo.toml','--target-dir',T,*args]);a=D/('probe-'+kind+ext);shutil.copyfile(T/sub/('independent-markup-routes'+ext),a);run('probe-'+kind,[a]if kind=='native'else['wasmtime','run',a],60)
 run('managed-'+kind,['cargo','test','--offline','--locked','--manifest-path',W/'Cargo.toml','--target-dir',T,'-p','nepl3-markup','--test','routes',*args,'--','--nocapture','--test-threads=1'])
