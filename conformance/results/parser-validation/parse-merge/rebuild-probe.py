from pathlib import Path
r=Path(__file__).resolve().parent
p=r/'probe-native.log'
if 'left: 300' in p.read_text(encoding='utf-8'):
 (r/'probe-initial-source-accounting.log').write_bytes(p.read_bytes())
 (r/'probe-initial-host.rs').write_bytes((r/'source/probe/tests/parse/host.rs').read_bytes())
 (r/'probe-initial-test.rs').write_bytes((r/'source/probe/tests/probe.rs').read_bytes())
for name in ['source','before']:
 p=r/name/'probe/tests/probe.rs';s=p.read_text(encoding='utf-8');s=s[:s.index('#[test]\nfn independent_')]+(r/'extra.rs').read_text();p.write_text(s,encoding='utf-8',newline='\n')
 p=r/name/'probe/tests/parse/host.rs';s=p.read_text(encoding='utf-8');start=s.index('        if let Action::Closure { order, mode } = self.action {');end=s.index('        let mut events = vec![];',start);s=s[:start]+(r/'host-extra.rs').read_text()+s[end:];p.write_text(s,encoding='utf-8',newline='\n')
