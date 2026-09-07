from pathlib import Path
import subprocess,json,os
p=Path(__file__).resolve().parent;rows=[]
for target in ['native','wasm32-wasip2']:
 cmd=['cargo','test','--offline','--locked','--manifest-path',str(p/'source/Cargo.toml'),'-p','nepl3-core','--test','contracts','--target-dir',str(p.parent/'review-fixed-target')]
 if target!='native':cmd+=['--target',target]
 cmd+=['--','--nocapture','--test-threads=1'];env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
 with (p/('contracts-'+target+'.log')).open('w',encoding='utf-8',newline='\n') as log:r=subprocess.run(cmd,cwd=p/'source',env=env,stdout=log,stderr=subprocess.STDOUT,timeout=600)
 rows.append(dict(command=cmd,cwd=str(p/'source'),target=target,exit=r.returncode,deadline=600,log='contracts-'+target+'.log'));(p/'contracts-runs.json').write_text(json.dumps(rows,indent=2),encoding='utf-8',newline='\n');print(target,r.returncode,flush=True)
