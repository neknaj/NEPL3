from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys

root = Path(__file__).resolve().parent
repo = Path('C:/projects/NEPL3-doc-authoring-b')
sources = json.loads((root / 'sources.json').read_text(encoding='utf-8'))
for entry in sources:
    data = (root / entry['path']).read_bytes()
    assert len(data) == entry['bytes']
    assert hashlib.sha256(data).hexdigest() == entry['sha256']
    original = subprocess.check_output(['git', '-C', str(repo), 'show', entry['source_commit'] + ':' + entry['source_path']])
    assert data == original
prior = Path('C:/projects/NEPL3-runtime/.tmp/review-doc-authoring-b/review.md')
(root / 'prior-review.md').write_bytes(prior.read_bytes())
env = dict(os.environ, PYTHONIOENCODING='utf-8')
run = subprocess.run([sys.executable, str(root / 'check.py')], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env)
(root / 'check.log').write_bytes(run.stdout)
(root / 'execution.json').write_bytes((json.dumps({'command': 'python check.py', 'exit_code': run.returncode, 'source_git_blob_checks': len(sources), 'production_runtime_executed': False}, indent=2) + '\n').encode())
assert run.returncode == 0
selected = [entry['path'] for entry in sources]
selected += ['freeze.py', 'check.py', 'finalize.py', 'sources.json', 'results.json', 'record.md', 'prior-review.md', 'check.log', 'execution.json']
for chapter in ('05', '10'):
    selected += [chapter + '/' + name for name in ('change.patch', 'normalized.json', 'readings.txt')]
entries = []
for path in sorted(set(selected)):
    file = root / path
    data = file.read_bytes()
    entries.append({'path': file.relative_to(root).as_posix(), 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
(root / 'manifest.json').write_bytes((json.dumps({'scope': 'independent chapter 05/10 Ruby repair content review', 'files': entries}, indent=2) + '\n').encode())
for name in ('record.md', 'manifest.json'):
    print(name, hashlib.sha256((root / name).read_bytes()).hexdigest())
