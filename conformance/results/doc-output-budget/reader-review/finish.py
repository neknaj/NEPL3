from pathlib import Path
import json,hashlib,tarfile
r=Path(__file__).parent;repo=r.parents[1];sha=lambda b:hashlib.sha256(b).hexdigest()
snapshot=repo/'.tmp/review-output-budget/workspace';archive=repo/'.tmp/review-output-budget/workspace.tar'
with tarfile.open(archive) as t:
 for m in t.getmembers():
  if m.isfile() and (m.name.startswith('tools/src/') or m.name.startswith('crates/')):
   assert (snapshot/m.name).read_bytes()==t.extractfile(m).read(),m.name
default=json.loads((r/'default/input.json').read_text());explicit=json.loads((r/'explicit-200m/input.json').read_text());limits=explicit.pop('output_limits');assert default==explicit
baseline=json.loads((repo/'.tmp/review-output-budget/native-manifest.json').read_text())['output_budget']['limits']
assert {k:v for k,v in limits.items()if k!='work'}=={k:v for k,v in baseline.items()if k!='work'};assert limits['work']==200000000 and baseline['work']==100000000
manifest=json.loads((r/'explicit-200m/output/manifest.json').read_text());assert manifest['output_budget']['usage']['work']==101271279
(r/'review.md').write_text('''# Independent actual Reader execution

PASS narrow actual-input verification using previously independently built fixed b84e9a3854dd23538154194054c28713714ba123 production executable; binary SHA6bdf4475e02fedb2add55d4fb41320e0bb527930d8c13ea447421ddbc3f71cec. All tools/src and crates source bytes in the executable checkout rechecked against original Git archive. Reviewer-only edits are test code outside production. The independent build has a different binary SHA from root's build, not a substituted root executable.

Copied exact original manifests and 69199-byte source SHA162fe9ec7faf8c671ef3769fc27342209577db38084d0201220b7871c430bab3 to new scratch. Both original input manifests match after removing explicit output_limits; id candidate, route reference/reader/index.html. Other seven output limits equal default; only Work100000000 versus200000000 differs, selected before each run.

Default run exits1 with serialize WorkLimit and creates no output directory. Explicit200M run exits0, final Work101271279. Independently hashed HTML218111bytes SHA6b0b428902327d3bd0a7d5da43f0418bb7bfda98d13e4da45bf876530e2b24c6; CSS1860bytes SHA95e1239989b6be84d51d5cbf23a0c48f655d43392fc1e1e0e3791d5235d413d2. These and manifest3099bytes are byte-identical to root's separate run. Default failure remains recorded and is not corrected into success.

No full Markdown versus rendered semantic text comparison was newly executed; existing subset guide parser does not handle this complete Reader chapter. This evidence establishes actual native production generation under selected limits and deterministic artifact bytes, not full-content acceptance, browser display, Pages deployment or all-Doc migration. No production edits or resource defaults changed by reviewer.
''',encoding='utf-8')
entries=[]
for p in sorted(r.rglob('*')):
 if p.is_file() and p!=r/'manifest.json':
  b=p.read_bytes();entries.append({'path':p.relative_to(r).as_posix(),'bytes':len(b),'sha256':sha(b)})
(r/'manifest.json').write_text(json.dumps(entries,indent=2)+'\n',encoding='utf-8');print(sha((r/'manifest.json').read_bytes()),len(entries))
