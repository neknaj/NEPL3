from pathlib import Path
import subprocess,sys,os,json,time
R=Path(__file__).resolve().parent
target=sys.argv[1];env=dict(os.environ)
args=['cargo','test','--locked','-p','nepl3-tools','--test','doc','pages::','--target-dir',str(R/'target-after')]
if target=='wasi':
 args+=['--target','wasm32-wasip2'];env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
args+=['--','--nocapture','--test-threads=1']
t=time.monotonic();p=subprocess.run(args,cwd=R/'after',env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=300)
(R/('tools-'+target+'.log')).write_bytes(p.stdout)
row=dict(command=args,exit_code=p.returncode,seconds=time.monotonic()-t)
(R/('tools-'+target+'.json')).write_text(json.dumps(row,indent=2)+'\n',encoding='utf-8')
print(json.dumps(row));assert p.returncode==0,p.stdout.decode(errors='replace')[-5000:]
