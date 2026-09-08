from pathlib import Path
import hashlib,json
repo=Path(__file__).resolve().parents[2];out=Path(__file__).resolve().parent
for source,name in [
 ('.tmp/root-guides-generation/AGENTS.log','root-invalid.log'),
 ('.tmp/root-guides-generation/results.json','root-invalid-results.json'),
 ('.tmp/generate-invalid-command.py','root-invalid-command.py'),
 ('.tmp/root-guides-generation-corrected/AGENTS.log','root-corrected.log'),
 ('.tmp/root-guides-generation-corrected/results.json','root-corrected-results.json'),
 ('.tmp/generate.py','root-corrected-command.py')]:
 (out/name).write_bytes((repo/source).read_bytes())
data=json.loads((out/'root-corrected-results.json').read_bytes());run=next(x for x in data['runs'] if x['name']=='AGENTS')
assert run['source_sha256']=='35d047c89d04bfc51527aee7f818e17acabd327618f8dfea2ccf978359a22154'
assert run['exit_code']==1 and run['command'][1:3]==['doc-html','export']
assert hashlib.sha256((out/'root-corrected.log').read_bytes()).hexdigest()==run['log_sha256']
assert not Path(run['command'][-1]).exists()
log=(out/'root-corrected.log').read_text(encoding='utf-8')
assert 'NeedsResolution' in log and log.count('Link {')==2 and 'doc/authoring.md' in log and 'https://github.com/neknaj/gloss#ruby' in log
assert hashlib.sha256(Path(run['command'][0]).read_bytes()).hexdigest()==data['binary_sha256']
items=[{'path':p.relative_to(out).as_posix(),'bytes':len(p.read_bytes()),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(out.rglob('*')) if p.is_file() and p.name!='manifest.json' and '__pycache__' not in p.parts]
raw=(json.dumps({'scope':'Independent AGENTS content review with separately attributed root failure execution evidence','files':items},indent=2)+'\n').encode();(out/'manifest.json').write_bytes(raw);print(hashlib.sha256(raw).hexdigest())
