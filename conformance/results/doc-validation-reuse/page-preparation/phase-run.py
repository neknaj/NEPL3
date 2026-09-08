from pathlib import Path
import subprocess,json,time,sys,hashlib
R=Path(__file__).resolve().parent;name=sys.argv[1]
args=['cargo','build','--locked','-p','nepl3-tools','--target-dir',str(R/'target-phase')]
t=time.monotonic();p=subprocess.run(args,cwd=R/'phase',stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=300)
(R/('phase-'+name+'-build.log')).write_bytes(p.stdout);assert p.returncode==0
binary=R/'target-phase/debug/nepl3-tools.exe'
args=[str(binary),'doc-html','pages',str(R/'phase-input/pages.json'),str(R/('phase-output-'+name))]
p=subprocess.run(args,cwd=R/'phase',stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180)
(R/('phase-'+name+'.log')).write_bytes(p.stdout)
row=dict(command=args,exit_code=p.returncode,seconds=time.monotonic()-t,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
(R/('phase-'+name+'.json')).write_text(json.dumps(row,indent=2)+'\n',encoding='utf-8')
print(json.dumps(row));print(p.stdout.decode());assert p.returncode==1
