from pathlib import Path
import json,re,hashlib,subprocess
r=Path(__file__).parent;root=r.parent.parent
def read(p):
    b=p.read_bytes();return b.decode('utf-16' if b.startswith(b'\xff\xfe') else 'utf-8')
old=read(r.parent/'review-bootstrap-stop/sweep.log');new=read(r/'tests.log')
before={int(m[1]):dict(re.findall(r'(\w+):\s+(\d+)',m[2])) for m in re.finditer(r'REVIEW_SWEEP_ERROR cap=(\d+).*?usage=(Usage \{[^}]+\})',old,re.S)}
after={int(m[1]):dict(re.findall(r'(\w+):\s+(\d+)',m[2])) for m in re.finditer(r'REVIEW_PREPARATION cap=(\d+) usage=(Usage \{[^}]+\})',new,re.S)}
assert len(before)==34 and after==before,(len(before),len(after))
formal=list(map(int,re.findall(r'REVIEW_COMPLETE_STOP cap=(\d+)',new)));assert formal
nonstop=re.search(r'REVIEW_NONSTOP (.+)',new);assert nonstop and 'Boundary' in nonstop[1]
assert 'test result: ok. 9 passed; 0 failed; 1 ignored;' in new
head=json.loads((r/'source.json').read_text())['head']
for path in ['tools/src/bootstrap/runtime.rs','tools/src/bootstrap/tests.rs']:
    (r/(path.replace('/','--')+'.source')).write_bytes(subprocess.check_output(['git','show',head+':'+path],cwd=root))
(r/'probe-tests.rs').write_bytes((r/'workspace/tools/src/bootstrap/tests.rs').read_bytes())
result=dict(head=head,preparation_caps_and_identical_usage=after,formal_complete_reply_stops=formal,nonstop_error=nonstop[1],native_tests=dict(passed=9,ignored=1),scope='native bootstrap tests plus independent nonstopping malformed root; ignored test remains unrun')
(r/'results.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
files=sorted(p for p in r.iterdir() if p.is_file() and p.name!='manifest.json')
manifest=dict(files=[dict(path=p.name,bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in files])
(r/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8');print(len(after),len(formal),hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest())
