from pathlib import Path
import subprocess
r=Path(__file__).resolve().parent
old=r.parent/'review-tokenizer-failure'
s=(old/'setup.py').read_text().replace('NEPL3-tokenizer-owned-failure','NEPL3-tokenizer-recovering-api').replace('ccafb1e','b3f894b').replace('be6aa8b','edda943')
(r/'setup.py').write_text(s,encoding='utf-8',newline='\n')
(r/'run.py').write_bytes((old/'run.py').read_bytes())
subprocess.run(['python',str(r/'setup.py')],check=True)
p=r/'source/doc/spec/03-reader.md';p.parent.mkdir(parents=True,exist_ok=True)
p.write_bytes(subprocess.check_output(['git','-C','C:/projects/NEPL3-tokenizer-recovering-api','show','b3f894b:doc/spec/03-reader.md']))
