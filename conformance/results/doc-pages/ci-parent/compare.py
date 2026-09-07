import hashlib, io, json, subprocess, zipfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
artifact = 10032422156
raw = subprocess.check_output(['gh', 'api', f'repos/neknaj/NEPL3/actions/artifacts/{artifact}/zip'])
digest = hashlib.sha256(raw).hexdigest()
assert digest == '8a633a91b634c675e93886a3257e55c5b6dea1249ef4b93750dec0e25c69b457'
records = []
with zipfile.ZipFile(io.BytesIO(raw)) as archive:
    names = [n for n in archive.namelist() if not n.endswith('/')]
    local = root / '.tmp/migration-site'
    assert sorted(names) == sorted(p.relative_to(local).as_posix() for p in local.rglob('*') if p.is_file())
    for name in names:
        data = archive.read(name)
        same = data == (local / name).read_bytes()
        records.append(dict(path=name, same=same, remote_sha256=hashlib.sha256(data).hexdigest(), local_sha256=hashlib.sha256((local/name).read_bytes()).hexdigest()))
    manifest = json.loads(archive.read('manifest.json'))
    for item in manifest['files']:
        assert hashlib.sha256(archive.read(item['path'])).hexdigest() == item['sha256']
result = dict(run=34160389457, source='2ddddd05e9334b83c9f654e3a1042555de5aefd4', artifact=artifact, archive_sha256=digest, files=records)
(root / '.tmp/ci-pages-comparison.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8', newline='\n')
assert all(r['same'] for r in records), records
print('CI artifact and local Windows export are byte-identical:', len(records), 'files')
