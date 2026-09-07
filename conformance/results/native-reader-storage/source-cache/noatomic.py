from pathlib import Path
import subprocess,json
p=Path(__file__).resolve().parent;cmd=['cargo','check','--offline','--locked','--manifest-path',str(p/'source/Cargo.toml'),'-p','nepl3-core','-p','nepl3-wire','--target','thumbv6m-none-eabi','--target-dir',str(p.parent/'review-fixed-noatomic-target')]
with (p/'noatomic.log').open('w',encoding='utf-8',newline='\n') as log:r=subprocess.run(cmd,cwd=p/'source',stdout=log,stderr=subprocess.STDOUT,timeout=600)
result=dict(command=cmd,cwd=str(p/'source'),exit=r.returncode,deadline=600,target='thumbv6m-none-eabi');(p/'noatomic-run.json').write_text(json.dumps(result,indent=2),encoding='utf-8',newline='\n');print(result)
(p/'noatomic-cfg.txt').write_bytes(subprocess.check_output(['rustc','--print','cfg','--target','thumbv6m-none-eabi']))
