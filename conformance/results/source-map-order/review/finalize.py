from pathlib import Path
import hashlib,json,re,subprocess,tomllib
r=Path(__file__).resolve().parent
def sha(data):return hashlib.sha256(data).hexdigest()
counts={};costs={}
for mode in ['native','wasi','before-native']:
 assert json.loads((r/(mode+'.json')).read_text())['exit_code']==0
 log=(r/(mode+'.log')).read_text(encoding='utf-8');counts[mode]=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))-3
 costs[mode]={f'{s}/{o}':dict(work=int(w),allocation=int(a),nodes=int(n),depth=int(d)) for s,o,w,a,n,d in re.findall(r'shape=(\d+) order=(\d+) work=(\d+) allocation=(\d+) nodes=(\d+) depth=(\d+)',log)}
assert {k:v['work'] for k,v in costs['native'].items()}=={k:v['work'] for k,v in costs['wasi'].items()}
for name in ['source','before']:
 manifest=json.loads((r/(name+'-manifest.json')).read_text())
 for entry in manifest['files']:
  if entry['path'] in ['Cargo.toml','Cargo.lock']:continue
  assert sha((r/name/entry['path']).read_bytes())==entry['sha256']
 original=tomllib.loads(subprocess.check_output(['git','-C','C:/projects/NEPL3-source-map-ordered-locality','show',manifest['commit']+':Cargo.lock']).decode())
 selected=tomllib.loads((r/name/'Cargo.lock').read_text())
 allowed={(p['name'],p['version'],p.get('checksum')) for p in original['package']}
 assert all((p['name'],p['version'],p.get('checksum')) in allowed for p in selected['package'] if p['name']!='revision-probe')
 for fn in ['Cargo.toml','Cargo.lock']:(r/(name+'-review-'+fn)).write_bytes((r/name/fn).read_bytes())
(r/'results.json').write_text(json.dumps(dict(managed_passed=counts,independent_passed_each=3,costs=costs,source_files_unchanged=True,external_lock_versions_preserved=True),indent=2)+'\n',encoding='utf-8')
record='''# Ordered snapshot graph locality independent review

Candidate 4be6b341721c3c16ec88402cc8c9982c248ae478; base 58d622c5d0909475dc8be24bbd3e89c80627ae06. No correctness blocking issue found. Scope is origin.rs snapshot_dag and one managed maps test. Complete SnapshotId comparison includes source, revision and digest; every hint comparison is charged before inspecting identity. Recent endpoint cache hits still bypass ordered search exactly as before. Ordered hint is a valid sorted-vector position: ordinary lookup sets the successful midpoint, new vertex insertion resets it to the new position, and no deletion occurs. Recent indices refer to append-only nodes, so shifting ordered indices does not invalidate them. The state exists for only one graph build and is not a portable authority/cache.

All geometry/source checks and the pointwise fallback after a coarse graph cycle remain in place. SourceMap proof is returned only after success; stopped local graph construction does not publish partial state. No cap, schema, or source declaration change exists.

Core source frozen from Git (36 files per revision, not the full repository). Scratch-only workspace selects core plus independent probe; all production files rechecked byte-identical and resolved external dependencies remain the original lock versions/checksums. A preliminary shell quoting error creating the scratch setup script occurred before any test; apply_patch created the script and all recorded tests below executed normally. No production or Git edits.

Managed core counts: COUNTS. All 9 candidate maps tests pass, including the existing exhaustive 512 three-vertex graphs/all splits and new 24 edge orders. Independent 3 native + 3 WASI and same base-native 3 pass: 512 distinct four-vertex Boolean reachability oracles with 3 edge orders/every split; same Unicode name/different revisions including u64::MAX; coarse cyclic but pointwise acyclic disjoint spans, actual cycle, self-shift; wrong digest rejected as missing snapshot before graph construction. Complete digest comparison in graph nodes is also directly read in the derived SnapshotId Ord path. 720 Work/Allocation/Nodes/Depth limits produce 368 sticky stops on both targets, cancellation preserved, public source store unchanged; 9000-byte Unicode IDs stop under low Work without copying their payloads.

256-edge Work base -> candidate: ascending chain 178771 -> 51567, descending chain 223818 -> 185206, permuted chain 202226 -> 198992; ascending star 102674 -> 39072, descending star 147466 -> 84207, permuted star 124175 -> 129712 (+4.46%). Repeated identical edge is unchanged 26322 in all orders, confirming recent endpoint hits remain effective. Allocation/Nodes/Depth remain equal between revisions on native; native/WASI Work matches, while pointer-sized allocation accounting differs by target. Long Unicode chain 63310 -> 54268 Work. This is not a universal improvement. Full Doc corpus/tool/reader behavior is measured separately by root; this review does not claim corpus speedup or merge readiness from synthetic evidence alone.
'''.replace('COUNTS',str(counts))
(r/'record.md').write_text(record,encoding='utf-8',newline='\n')
files=[]
for p in sorted(r.iterdir()):
 if p.is_file() and p.name!='manifest.json':
  data=p.read_bytes();files.append(dict(path=p.name,bytes=len(data),sha256=sha(data)))
(r/'manifest.json').write_text(json.dumps(dict(candidate='4be6b341721c3c16ec88402cc8c9982c248ae478',base='58d622c5d0909475dc8be24bbd3e89c80627ae06',files=files),indent=2)+'\n',encoding='utf-8');print(counts);print(sha((r/'manifest.json').read_bytes()))
