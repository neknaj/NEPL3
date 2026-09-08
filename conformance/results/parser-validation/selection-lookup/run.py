from pathlib import Path
import subprocess,json,time,os,re
R=Path(__file__).resolve().parent;rows=[]
for name,target,full in [('before',None,False),('after',None,True),('after','wasm32-wasip2',True)]:
 label=name+('-wasi' if target else '-native')
 cmd=['cargo','test','--locked','-p','nepl3-engine','--test','package']
 if not full:cmd+=['review_indexed_context']
 cmd+=['--target-dir',str(R/('target-'+name))]
 env=dict(os.environ)
 if target:cmd+=['--target',target];env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
 cmd+=['--','--nocapture','--test-threads=1']
 start=time.monotonic();p=subprocess.run(cmd,cwd=R/name,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=300)
 (R/(label+'.log')).write_bytes(p.stdout)
 row=dict(label=label,command=cmd,exit_code=p.returncode,seconds=time.monotonic()-start);rows.append(row);print(json.dumps(row),flush=True)
 assert p.returncode==0,p.stdout.decode(errors='replace')[-4000:]
(R/'execution.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
