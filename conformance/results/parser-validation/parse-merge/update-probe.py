from pathlib import Path
r=Path(__file__).resolve().parent
for name in ['source','before']:
 p=r/name/'probe/tests/probe.rs';s=p.read_text(encoding='utf-8').replace('ParseOutcome::Complete { tree, cursor }','ParseOutcome::Complete { tree, cursor, .. }');p.write_text(s,encoding='utf-8',newline='\n')
p=r/'probe-native.log'
if 'E0027' in p.read_text(encoding='utf-8'):(r/'probe-compile-error.log').write_bytes(p.read_bytes())
