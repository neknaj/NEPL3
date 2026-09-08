from pathlib import Path
import subprocess,json,os
r=Path(__file__).resolve().parent;w=r/'workspace';env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run';records=[]
for mode in ['native','wasi']:
 for package in ['nepl3-markup','nepl3-doc-html']:
  cmd=['cargo','test','--offline','--locked','--manifest-path',str(w/'Cargo.toml'),'--target-dir',str(r/'target'),'-p',package]
  if mode=='wasi':cmd+=['--target','wasm32-wasip2']
  cmd+=['--','--nocapture','--test-threads=1']
  p=subprocess.run(cmd,cwd=w,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=900);(r/(mode+'-'+package+'.log')).write_bytes(p.stdout);records.append({'command':cmd,'exit':p.returncode,'deadline':900});(r/'managed-runs.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8');print(mode,package,p.returncode,flush=True)
  if p.returncode:raise SystemExit(p.returncode)
