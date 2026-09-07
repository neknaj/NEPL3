from pathlib import Path
import os,subprocess,json,time,shutil
D=Path(__file__).resolve().parent;W=D/'workspace';T=D.parent/'review-fixed-target';env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run';runs=[]
for label,args in [('build-cli',['build','--bin','nepl3-tools']),('managed-native',['test','--test','doc','export::']),('managed-wasi',['test','--test','doc','export::','--target','wasm32-wasip2'])]:
 cmd=['cargo',args[0],'--offline','--locked','--manifest-path',str(W/'Cargo.toml'),'--target-dir',str(T),'-p','nepl3-tools',*args[1:]]
 if label!='build-cli':cmd+=['--','--nocapture','--test-threads=1']
 start=time.time();p=subprocess.run(cmd,cwd=W,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=1200);(D/(label+'.log')).write_bytes(p.stdout);runs.append({'label':label,'command':cmd,'exit':p.returncode,'seconds':time.time()-start,'deadline':1200});(D/'managed-runs.json').write_text(json.dumps(runs,indent=2),encoding='utf-8');print(label,p.returncode,flush=True)
 if p.returncode:print(p.stdout.decode(errors='replace')[-4000:]);raise SystemExit(p.returncode)
 if label=='build-cli':shutil.copyfile(T/'debug/nepl3-tools.exe',D/'nepl3-tools.exe')
