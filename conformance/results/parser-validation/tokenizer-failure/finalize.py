from pathlib import Path
import subprocess,json,hashlib,re,tomllib
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-tokenizer-owned-failure'
def sha(data):return hashlib.sha256(data).hexdigest()
counts={}
for mode in ['managed-native','managed-wasi','before-native','probe-native','probe-wasi']:
 assert json.loads((r/(mode+'.json')).read_text())['exit_code']==0
 counts[mode]=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(r/(mode+'.log')).read_text())))
for name in ['source','before']:
 m=json.loads((r/(name+'-manifest.json')).read_text())
 for e in m['files']:
  if e['path'] in ['Cargo.toml','Cargo.lock']:continue
  assert sha((r/name/e['path']).read_bytes())==e['sha256']
 orig=tomllib.loads(subprocess.check_output(['git','-C',repo,'show',m['commit']+':Cargo.lock']).decode());selected=tomllib.loads((r/name/'Cargo.lock').read_text());allowed={(p['name'],p['version'],p.get('checksum')) for p in orig['package']};assert all((p['name'],p['version'],p.get('checksum')) in allowed for p in selected['package'])
 for fn in ['Cargo.toml','Cargo.lock']:(r/(name+'-review-'+fn)).write_bytes((r/name/fn).read_bytes())
for e in json.loads((r/'source-manifest.json').read_text())['files']:
 if e['path'] in ['Cargo.toml','Cargo.lock','crates/foundation/reader/src/lib.rs','crates/foundation/reader/src/tokenizer/session.rs']:continue
 assert sha((r/'probe'/e['path']).read_bytes())==e['sha256']
assert (r/'probe/crates/foundation/reader/src/tokenizer/session.rs').read_bytes()==(r/'source/crates/foundation/reader/src/tokenizer/session.rs').read_bytes()+b'\n#[cfg(test)] mod independent_review;\n'
assert (r/'probe/crates/foundation/reader/src/lib.rs').read_bytes()==(r/'source/crates/foundation/reader/src/lib.rs').read_bytes()+b'\n#[cfg(test)] extern crate std;\n#[cfg(test)] extern crate self as nepl3_reader;\n'
for leaf,path in [('independent-session.rs','src/tokenizer/session/independent_review.rs'),('instrumented-lib.rs','src/lib.rs'),('instrumented-session.rs','src/tokenizer/session.rs')]:
 (r/leaf).write_bytes((r/'probe/crates/foundation/reader'/path).read_bytes())
(r/'results.json').write_text(json.dumps(dict(counts=counts,source_hashes_verified=True,fix_verified=True),indent=2)+'\n',encoding='utf-8')
record='''# Tokenizer owned failure stage independent review

Final candidate edda9439c6be842ae3dc5de4cf45beedec35d4c0; base be6aa8bbe3e402ccd5ab1bb736a74c3f87dab8c0. Initial candidate ccafb1eb1ddd20e13d70a47de0f2ce02a334ff05 had one confirmed internal recovery-contract defect. No additional blocking issue remains in the reviewed final stage.

Actual read_seed reproduction: public AcceptedTokenizationReport::empty was minted after Work123; recorded Usage was Work124/Allocation120. Passing a fresh Budget with the same Limits correctly returned ReaderError::Continuation, but initial seed_failure rewrote the returned collector Usage to0/0. This would erase the prior operation's cost when the owned collector is recovered. Existing public dispatch rejects the invalid budget earlier and was not demonstrated exploitable. Root edda943 now retains original report Usage unless Limits match and all observed counters are at least prior Usage. The original read_seed case now returns124/120, and another actual call with different Limits plus larger unrelated Usage preserves the whole original Report. Original failure log and initial source hashes remain archived; the failing test function is unchanged in the final probe.

Path review: read_seed returns owned collector for budget/preflight/closed/busy/mode/kind/request/accepted-check errors. Typed validation/copy stops keep empty_stop; depth overflow is checked before moving accepted fields. Once in Machine, finish_recover moves current collector on drive hard error, waiting preparation hard error and outcome.public failure; stopped waiting preparation returns Stopped and clears nested pending. Scoped public dispatch still enforces scope identity and maps internal failure to error only. The private failure deliberately does not retain scope or Limits yet. Outer recovery and engine checkpoint removal are future work, so this stage provides no measured Doc copy reduction.

Executed counts: COUNTS. Candidate managed suite includes actual ReaderSession/TokenizationSession retry, schema/state/source/view rejection, accepted skip rollback, native host and reservation, stopped report/source closure, and NDF Transform outcomes. Base suite passes natively. Four independent crate-local tests call real read_seed or finish_recover entries, not just seed_failure/current_failure helpers. read_seed covers Closed/Busy (after actual Await)/unknown mode/wrong state/bounds/unknown kind and source/map pointer preservation; public read_with_accepted rejects a different operation scope. finish_recover covers hard error, typed stop, incompatible waiting outcome, missing waiting scope, allocation exhaustion during waiting preparation and outcome.public(None) failure. Report diagnostics/events/sources/maps survive, and no pending is installed on these failures. The six finish cases inject private Machine/outcome states intentionally; they are not all claimed reachable through legitimate external input. All four pass native/WASI.

Frozen minimal core/reader/wire source104 candidate/103 base files, not a full repository. All production bytes were checked against Git; test-only copies append cfg(test) modules/extern imports to lib.rs and tokenizer/session.rs and add independent-session.rs. Actual implementation bodies are unchanged. Original external dependency versions/checksums retained. Production/Git were not edited. Scratch module has unused fixture-import/helper warnings; these are not a production Clippy result. During probe construction API spelling, local kind integer width and a temporary Unit borrow were corrected before successful final execution; these are not production failures.

No universal runtime completion, first-CBOR ownership proof, ARM execution, or Doc performance improvement is inferred. Potential future reuse must carry original scope/Limits outside the report-only failure and must not trust an invalid caller Budget; the final guard fixes the reproduced regression but does not implement that next API.
'''.replace('COUNTS',str(counts))
(r/'record.md').write_text(record,encoding='utf-8',newline='\n');files=[]
for p in sorted(r.iterdir()):
 if p.is_file() and p.name!='manifest.json':data=p.read_bytes();files.append(dict(path=p.name,sha256=sha(data),bytes=len(data)))
(r/'manifest.json').write_text(json.dumps(dict(candidate='edda9439c6be842ae3dc5de4cf45beedec35d4c0',base='be6aa8bbe3e402ccd5ab1bb736a74c3f87dab8c0',files=files),indent=2)+'\n',encoding='utf-8');print(counts);print(sha((r/'manifest.json').read_bytes()))
