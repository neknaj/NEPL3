from pathlib import Path
import hashlib,json,subprocess,re
r=Path(__file__).parent;repo=r.parents[1]
def git(*args):return subprocess.check_output(['git','-C',str(repo),*args])
summary={}
for name in ['native.log','wasi.log']:
 b=(r/name).read_bytes();s=b.decode('utf-16' if b[:2]==b'\xff\xfe' else 'utf-8-sig')
 assert '22 passed; 0 failed' in s,name
 summary[name]={'tests':re.findall('test result:.*',s),'observation':re.findall('REVIEW_SUBJECT[^\r\n]*',s)}
summary['head']=git('rev-parse','abf9b4b').decode().strip()
summary['base']=git('rev-parse','a20fae7').decode().strip()
summary['commands']=['cargo test --locked -p nepl3-engine --test package -- --nocapture','cargo test --locked -p nepl3-engine --test package --target wasm32-wasip2 -- --nocapture; runner wasmtime run']
summary['tools']={'rustc':'1.97.0 (2d8144b78 2026-07-07)','wasmtime':'44.0.1 (f302ebd6b 2026-04-30)'}
(r/'results.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
(r/'fix.diff').write_bytes(git('diff','a20fae7','abf9b4b'))
(r/'review.md').write_text('''# Independent subject correction review

PASS scoped correction abf9b4b608adeccb7a9bfb087afd276e7577c07d. Exactly two changed files from a20fae7: package/check.rs clears subject immediately before derived FormIndex construction; package test adds Work and Allocation final-one-short checks with typed reason, None subject and sticky budget. Earlier provenance validation errors are unchanged.

Frozen Git archive; no live production modification. Independent old public probe reused with ONLY expected subject changed from Some(Provenance(0)) to None. Native original and fixed observation have identical all Usage fields; only corrected subject changes. Original evidence retained at C:/projects/NEPL3-checked-form-index/.tmp/review-form-index, manifest SHA7ba526b0dd5423fb3d0514751eca1aad1d09eec030b90ce981ed3564fd5c1274.

Native and WASI each run all 21 production package tests plus independent reproduction (22 passed each). Both include real package.check_detailed index Work/Allocation stops and previous invalid provenance rejection. No broader workspace/Doc/browser tests repeated for this two-file correction. Physical allocator failure remains source-inspected rather than forced. This confirms the prior review's one diagnostic-attribution finding is resolved; no further blocking issue found.
''',encoding='utf-8')
files=['setup.py','probe.rs','finish.py','native.log','wasi.log','results.json','fix.diff','review.md']
entries=[]
for p in files:
 b=(r/p).read_bytes();entries.append({'path':p,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(r/'manifest.json').write_text(json.dumps(entries,indent=2)+'\n',encoding='utf-8')
print(hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest())
