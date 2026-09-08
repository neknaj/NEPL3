from pathlib import Path
r=Path(__file__).resolve().parent
for name in ['source','before']:
 p=r/name/'probe/tests/selection.rs';s=p.read_text();start=s.index('    use nepl3_core::budget::{Resource,StopReason};');p.write_text(s[:start]+(r/'extra.rs').read_text(),encoding='utf-8',newline='\n')
 p=r/name/'probe/tests/foreign.rs';s=p.read_text();start=s.index('    use nepl3_engine::tree::TreeError;');end=s.index('    Ok(())\n}',start);p.write_text(s[:start]+(r/'foreign-extra.rs').read_text()+s[end:],encoding='utf-8',newline='\n')
