from pathlib import Path
import subprocess,tarfile,io,json,hashlib
r=Path(__file__).parent
repo=r.parents[1]
b=subprocess.check_output(['git','-C',str(repo),'archive','abf9b4b'])
(r/'source.tar').write_bytes(b)
with tarfile.open(fileobj=io.BytesIO(b)) as t:t.extractall(r/'workspace',filter='data')
prior=Path('C:/projects/NEPL3-checked-form-index/.tmp/review-form-index/workspace/crates/foundation/engine/tests/package.rs').read_text(encoding='utf-8')
test=prior[prior.index('#[test]\nfn reviewer_index_stop_subject_observation'):]
test=test.replace('Some(PackageSubject::Provenance(0))','None')
p=r/'workspace/crates/foundation/engine/tests/package.rs'
with p.open('a',encoding='utf-8',newline='\n') as f:f.write('\n'+test)
(r/'probe.rs').write_text(test,encoding='utf-8',newline='\n')
