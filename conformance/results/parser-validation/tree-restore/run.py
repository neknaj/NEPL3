from pathlib import Path
import subprocess,time,json,hashlib,os,re
R=Path(__file__).resolve().parent
rows=[]
def run(name,args,env=None):
 start=time.monotonic();p=subprocess.run(args,cwd=R/'workspace',env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=240)
 (R/(name+'.log')).write_bytes(p.stdout)
 row=dict(name=name,command=args,exit_code=p.returncode,seconds=time.monotonic()-start)
 rows.append(row);print(json.dumps(row),flush=True)
 assert p.returncode==0,p.stdout.decode(errors='replace')[-5000:]
 return p.stdout.decode()
base=['cargo','test','--locked','-p','nepl3-engine','--test','parse','--target-dir',str(R/'target')]
run('native',base+['--','--nocapture','--test-threads=1'])
env=dict(os.environ);env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
run('wasi',base+['--target','wasm32-wasip2','--','--nocapture','--test-threads=1'],env)
run('tools-build',['cargo','build','--locked','-p','nepl3-tools','--bin','nepl3-tools','--target-dir',str(R/'target')])
binary=R/'target/debug/nepl3-tools.exe'
for name in ['03-reader','04-grammar']:
 source=R.parent/'review-doc-stop-stage/inputs'/(name+'.nepld')
 output=R/'exports'/name
 assert not output.exists()
 cmd=[str(binary),'doc-html','export',str(source),str(output)]
 start=time.monotonic();p=subprocess.run(cmd,cwd=R/'workspace',stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180)
 (R/(name+'.log')).write_bytes(p.stdout)
 before=(Path('C:/projects/NEPL3-completed-tree-owned-validation/.tmp/exports-7251b01')/(name+'.log')).read_bytes().decode().strip()
 actual=[s for s in p.stdout.decode().splitlines() if s.startswith('nepl3-tools:')]
 assert actual==[before],(name,actual,before)
 marker=[s for s in p.stdout.decode().splitlines() if s.startswith('REVIEW_PHASE=')]
 assert len(marker)==1
 row=dict(name=name,command=cmd,exit_code=p.returncode,seconds=time.monotonic()-start,source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),marker=marker[0],original_full_error_usage_equal=True)
 rows.append(row);print(json.dumps(row),flush=True)
(R/'results.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
(R/'rust-version.txt').write_bytes(subprocess.check_output(['rustc','-vV'],timeout=60))
(R/'wasmtime-version.txt').write_bytes(subprocess.check_output(['wasmtime','--version'],timeout=60))
