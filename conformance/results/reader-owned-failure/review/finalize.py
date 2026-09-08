from pathlib import Path
import subprocess,json,hashlib,re,tomllib
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-reader-owned-failure'
def sha(b):return hashlib.sha256(b).hexdigest()
counts={}
for mode in ['managed-native','managed-wasi','before-native','probe-native','probe-wasi']:
 assert json.loads((r/(mode+'.json')).read_text())['exit_code']==0
 counts[mode]=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(r/(mode+'.log')).read_text())))
for name in ['source','before']:
 manifest=json.loads((r/(name+'-manifest.json')).read_text())
 for entry in manifest['files']:
  if entry['path'] in ['Cargo.toml','Cargo.lock']:continue
  assert sha((r/name/entry['path']).read_bytes())==entry['sha256']
 original=tomllib.loads(subprocess.check_output(['git','-C',repo,'show',manifest['commit']+':Cargo.lock']).decode());selected=tomllib.loads((r/name/'Cargo.lock').read_text())
 allowed={(p['name'],p['version'],p.get('checksum')) for p in original['package']}
 assert all((p['name'],p['version'],p.get('checksum')) in allowed for p in selected['package'])
 for fn in ['Cargo.toml','Cargo.lock']:(r/(name+'-review-'+fn)).write_bytes((r/name/fn).read_bytes())
# All actual implementation bytes in instrumented test crate remain exact source.
for entry in json.loads((r/'source-manifest.json').read_text())['files']:
 if entry['path'] in ['Cargo.toml','Cargo.lock','crates/foundation/reader/src/lib.rs']:continue
 assert sha((r/'probe'/entry['path']).read_bytes())==entry['sha256']
(r/'independent-session.rs').write_bytes((r/'probe/crates/foundation/reader/tests/review_owned.rs').read_bytes())
(r/'instrumented-lib.rs').write_bytes((r/'probe/crates/foundation/reader/src/lib.rs').read_bytes())
(r/'results.json').write_text(json.dumps(dict(counts=counts,source_hashes_unchanged=True,instrumentation_only_test_module=True,source_files=103,base_files=102),indent=2)+'\n',encoding='utf-8')
record='''# Reader owned failure stage 1 independent review

Candidate be6aa8bbe3e402ccd5ab1bb736a74c3f87dab8c0; base 0e0b3651bbaad0e29b1712201c8531d6934cc461. No additional correctness blocking issue found. Public method signatures and wire/schema are unchanged. This stage makes internal ownership recoverable; it does not remove the engine checkpoint or finish the outer tokenizer error ownership path.

Path review: Closed/Busy return the supplied seed untouched. Request/plan/state preparation occurs before consuming seed; typed preparation stops become Stopped with retained collector and current Usage. The root frame push still uses checkpoint_stopped for typed stops. drive/dispatch hard failures move current diagnostics/events/overflow/sources/maps into ReadFailure; successful or stopped controls use finish_recover. Suspension preparation hard errors return the owned current collector, while allocation stops keep the existing machine.reply path. Successful preparation/pending installation remains transactional. The public/crate legacy wrappers map ReadFailure back to ReaderError, preserving their old error surface. Tokenizer nested Read catches the owned failure and restores all five collector fields before propagating error; the outer hard-error path still drops it. There is no new successful portable reply or new validation authority.

The three result_large_err allowances are narrowly on ownership-returning operations. Boxing the error would introduce allocation at failure time. Collector vectors are moved; preflight tests verify pointer equality for diagnostics/events/sources/maps. No ordinary failure allocates a box or adds an unmetered recovery clone.

Executed counts: COUNTS. Managed runtime tests exercise public ReaderSession and TokenizationSession, rejected payload/state/source/view, pending retry, caller depth, accepted skip rollback, native/owned host, generated source/report stops, and native/NDF Transform outcomes. The identical base suite passes natively. Three additional independent tests execute real ReaderSession prepared plans and callbacks, with a test-only crate-local module to reach the ownership-returning entrance (not isolated helper calls): Closed, Busy, missing rule, invalid state type, source bounds; seven host modes including success/decline/context error/wrong schema/cancel/Work exhaustion/nested typed stop; two-call reader where first callback adds accepted diagnostics/events, second host error yields Await, and public resume preserves original plus newly accepted prefix. Work/Allocation sweep retains observed prefix Usage and returns 47/59 typed sticky stops; collector content remains intact. All three pass native/WASI.

The internal hard drive/finish-preparation error branches were read directly; independent tests do not claim to synthesize every otherwise impossible checked-plan invariant violation. Inner tokenizer restoration was read and its public surrounding behavior covered by managed tests; no test claims restored collector escapes an outer hard error. No Doc copy reduction or full Doc completion claim is supported by this stage. No new independent first-CBOR full transport or ARM run was performed.

Frozen minimal foundation source:103 candidate/102 base files with hashes; all actual source byte checks passed. A separate scratch test crate changes only cfg(test) extern/module declarations in lib.rs and adds independent-session.rs. It reuses exact public runtime fixture helpers; every implementation file remains byte-identical. External dependency versions/checksums retained. Production/Git were not modified. Archived evidence excludes repository copies and binaries. Initial probe compile failed because external-test std/alloc imports and Await field shape needed adaptation to a no_std crate-local test; raw log retained. A missing fixture helper was then included. The sweep was strengthened to preserve prefix Usage rather than reset it; initial successful log retained. Scratch-only unused-import warning is not a production Clippy result.
'''.replace('COUNTS',str(counts))
(r/'record.md').write_text(record,encoding='utf-8',newline='\n')
files=[]
for p in sorted(r.iterdir()):
 if p.is_file() and p.name!='manifest.json':b=p.read_bytes();files.append(dict(path=p.name,sha256=sha(b),bytes=len(b)))
(r/'manifest.json').write_text(json.dumps(dict(candidate='be6aa8bbe3e402ccd5ab1bb736a74c3f87dab8c0',base='0e0b3651bbaad0e29b1712201c8531d6934cc461',files=files),indent=2)+'\n',encoding='utf-8');print(counts);print(sha((r/'manifest.json').read_bytes()))
