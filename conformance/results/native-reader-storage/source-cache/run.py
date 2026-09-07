from pathlib import Path
import subprocess,json,os
d=Path(__file__).resolve().parent;rows=[]
for target in ['native','wasm32-wasip2']:
    cmd=['cargo','test','--offline','--manifest-path',str(d/'probe/Cargo.toml'),'--target-dir',str(d.parent/'review-fixed-target')]
    if target!='native':cmd+=['--target',target]
    cmd+=['--','--nocapture','--test-threads=1'];env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
    with (d/(target+'.log')).open('w',encoding='utf-8',newline='\n') as log:p=subprocess.run(cmd,cwd=d.parent.parent,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=1200)
    rows.append(dict(target=target,command=cmd,exit=p.returncode,log=target+'.log'));(d/'runs.json').write_text(json.dumps(rows,indent=2),encoding='utf-8',newline='\n');print(target,p.returncode,flush=True)
