from pathlib import Path
import subprocess
r=Path(__file__).resolve().parent;subprocess.run(['python',str(r/'bootstrap.py')],check=True)
for directory in ['source','before']:
 p=r/directory/'probe/tests/selection.rs';s=p.read_text();start=s.index('    use nepl3_core::budget::{Resource,StopReason};');p.write_text(s[:start]+(r/'extra.rs').read_text(),encoding='utf-8',newline='\n')
