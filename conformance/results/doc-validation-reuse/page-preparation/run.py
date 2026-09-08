from pathlib import Path
import subprocess,sys,time,os,json
R=Path(__file__).resolve().parent
mode=sys.argv[1];target=sys.argv[2] if len(sys.argv)>2 else 'native';W=R/mode
path=W/'crates/languages/doc/html/tests/local.rs';probe=(R/'probe.rs').read_bytes()
raw=path.read_bytes();needle=b'\nmod review_reuse {'
if needle in raw:raw=raw[:raw.index(needle)]
path.write_bytes(raw+b'\n'+probe)
env=dict(os.environ)
args=['cargo','test','--locked','-p','nepl3-doc-core','-p','nepl3-doc-html','--target-dir',str(R/('target-'+mode))]
if target=='wasi':
 args+=['--target','wasm32-wasip2'];env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
args+=['--','--nocapture','--test-threads=1']
t=time.monotonic();p=subprocess.run(args,cwd=W,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=300)
name=mode+'-'+target;(R/(name+'.log')).write_bytes(p.stdout)
row=dict(command=args,workspace=str(W),target=target,exit_code=p.returncode,seconds=time.monotonic()-t)
(R/(name+'.json')).write_text(json.dumps(row,indent=2)+'\n',encoding='utf-8')
print(json.dumps(row),flush=True)
assert p.returncode==0,p.stdout.decode(errors='replace')[-6000:]
