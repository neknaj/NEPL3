from pathlib import Path
import json,subprocess,os
p=Path(__file__).resolve().parent;w=p/'workspace';rows=[]
for target in ['native','wasm32-wasip2']:
 for kind,args in [('tools',['-p','nepl3-tools','--test','doc','html::']),('core',['-p','nepl3-doc-html','--test','local'])]:
  cmd=['cargo','test','--offline','--locked','--manifest-path',str(w/'Cargo.toml'),'--target-dir',str(p.parent/'review-fixed-target')]+args
  if target!='native':cmd+=['--target',target]
  cmd+=['--','--nocapture','--test-threads=1'];env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run';name=f'{kind}-{target}'
  with (p/(name+'.log')).open('w',encoding='utf-8',newline='\n') as log:r=subprocess.run(cmd,cwd=w,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=1200)
  rows.append(dict(name=name,command=cmd,cwd=str(w),target=target,exit=r.returncode,deadline=1200,log=name+'.log'));(p/'managed-runs.json').write_text(json.dumps(rows,indent=2),encoding='utf-8',newline='\n');print(name,r.returncode,flush=True)
