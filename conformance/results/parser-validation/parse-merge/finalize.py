from pathlib import Path
import subprocess,json,hashlib,re,tomllib
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-parse-source-merge-locality'
def sha(data):return hashlib.sha256(data).hexdigest()
counts={};costs={}
for mode in ['managed-native','managed-wasi','probe-native','probe-wasi','before-native']:
 assert json.loads((r/(mode+'.json')).read_text())['exit_code']==0
 text=(r/(mode+'.log')).read_text(encoding='utf-8');counts[mode]=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',text)))
 costs[mode]={f'{o}/{m}':dict(work=int(w),allocation=int(a),source_bytes=int(s)) for o,m,w,a,s in re.findall(r'public order=(\d+) mode=(\d+) work=(\d+) allocation=(\d+) sourcebytes=(\d+)',text)}
assert {k:v['work'] for k,v in costs['probe-native'].items()}=={k:v['work'] for k,v in costs['probe-wasi'].items()}
for name in ['source','before']:
 manifest=json.loads((r/(name+'-manifest.json')).read_text())
 for entry in manifest['files']:
  if entry['path'] in ['Cargo.toml','Cargo.lock']:continue
  assert sha((r/name/entry['path']).read_bytes())==entry['sha256']
 original=tomllib.loads(subprocess.check_output(['git','-C',repo,'show',manifest['commit']+':Cargo.lock']).decode());selected=tomllib.loads((r/name/'Cargo.lock').read_text())
 allowed={(p['name'],p['version'],p.get('checksum')) for p in original['package']}
 assert all((p['name'],p['version'],p.get('checksum')) in allowed for p in selected['package'] if p['name']!='parse-merge-probe')
 for fn in ['Cargo.toml','Cargo.lock']:(r/(name+'-review-'+fn)).write_bytes((r/name/fn).read_bytes())
for p in (r/'source/probe').rglob('*'):
 if p.is_file():q=r/'probe'/p.relative_to(r/'source/probe');q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(p.read_bytes())
(r/'results.json').write_text(json.dumps(dict(counts=counts,costs=costs,source_hashes_reverified=True,original_dependency_versions=True),indent=2)+'\n',encoding='utf-8')
record='''# Parse source merge locality independent review

Final candidate 85dc3a675cd77d540757f6799debf99de62f1183; formal base 4be6b341721c3c16ec88402cc8c9982c248ae478. Initial 8efa29e170107d40511c0285d1c775fd3180eef7 managed native/WASI and corrected public native probe logs are preserved separately in initial-8efa. Final candidate adds the adjacent-hit/gap comparisons and another managed test. No correctness blocking issue found.

Hint lifetime is exactly one extend_sources call. Existing sources are fully indexed first; duplicate preexisting keys are rejected. Each insertion resets the sorted position after shifts; each successful duplicate updates it. The hint and its immediate neighbor require full, charged source/revision comparisons. Equal key still executes full SnapshotId and URI validation; lower/upper bounds or a proved adjacent gap cannot skip a possible key. No arena/model fields, serialized proof, admitted source set or caps changed. No code in this slice promotes an unvalidated provider result.

Mutation may append a prefix before a later failure, as in the base algorithm. ParseSession finish on non-stop error discards pending and returns typed error; stop returns the formal stopped progress, not a complete tree. Imported prefix counters are updated only after source/map merge succeeds. The operation does not promise rollback of already charged accepted closure construction. This review does not incorrectly treat all malformed provider replies as retryable Await.

Frozen foundation source only: 212 candidate / 211 base files (including root manifest/lock/toolchain), no full repository copy. Scratch workspace selects four foundation crates plus probe. Original core/reader/wire/engine bytes reverified against Git manifests, original external dependency versions/checksums preserved. Native/WASI independent targets used. No production or Git edits.

Executed counts: COUNTS. Final managed suite includes actual parser/Foreign/resume/owned-fallback tests as well as the new private merge tests. Independent probe reuses the repository fixture declarations/setup but supplies its own real Read provider and public ParseSession cases. Actual `let x y tail`: 32 Unicode-named sources with paired revision 0/u64::MAX in ascending/descending/permuted order; repeated declarations preserve original ordering and bytes, SourceAdmission counts 108 source bytes once, URI conflict is rejected as Reader(Source(IdentityConflict)) with no Complete reply. Host decline after first declaration resumes via normal Await and retains the closure. Cancellation, five fixed Work caps and missing-field recovery pass. Base native receives identical independent inputs and expectations. Final native/WASI Work agrees.

Complete public pipeline Work base -> final: ascending 328584 -> 307079, descending 348168 -> 316762, permuted 390810 -> 404443 (+3.49%). Decline/resume: ascending 255302 -> 248402, descending 271565 -> 259772, permuted 303070 -> 307325 (+1.40%). Native Allocation is unchanged (61553 normal,190983 resumed) and SourceBytes 108; WASI allocation follows its existing pointer width. Results are distribution-dependent, not universal speedup. Root separately reported Doc03/04 now hitting AllocationLimit under unchanged caps; this review did not independently rerun that corpus or claim full Doc completion.

Probe-development corrections retained: initial missing enum `..` caused compile error; initial generated snapshots used SourceSnapshot::new without shared admission and repeated construction, so measured SourceBytes correctly exceeded the once-admitted expectation (300 then204). Final host creates with SourceAdmission and reuses request snapshots. Initial URI-conflict test assumed successful retry, but the actual typed identity-conflict error was retained as the required rejection; base confirms the same behavior. No production fix resulted from these scratch issues. A setup update attempted to archive a still-running WASI log before its JSON existed; it stopped before modifying candidate source, then reran after completion.

Fresh CBOR provider transport, ARM, full tools/Doc corpus and performance allocation hotspots were not added by this bounded review. Existing production wire use in the fixture and managed suite is not presented as a new independent transport proof.
'''.replace('COUNTS',str(counts))
(r/'record.md').write_text(record,encoding='utf-8',newline='\n')
files=[]
for p in sorted(r.rglob('*')):
 if not p.is_file() or p.name=='manifest.json' and p.parent==r:continue
 rel=p.relative_to(r)
 if len(rel.parts)>1 and rel.parts[0] not in ['initial-8efa','probe']:continue
 data=p.read_bytes();files.append(dict(path=rel.as_posix(),bytes=len(data),sha256=sha(data)))
(r/'manifest.json').write_text(json.dumps(dict(candidate='85dc3a675cd77d540757f6799debf99de62f1183',base='4be6b341721c3c16ec88402cc8c9982c248ae478',files=files),indent=2)+'\n',encoding='utf-8');print(counts);print(sha((r/'manifest.json').read_bytes()))
