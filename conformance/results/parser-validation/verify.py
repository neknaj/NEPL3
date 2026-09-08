"""Verify original review bytes, including individually gzip-packed large files."""
import gzip
import hashlib
import json
from pathlib import Path

base = Path(__file__).resolve().parent
count = 0
for manifest in sorted(base.glob('*/manifest.json')):
    for entry in json.loads(manifest.read_bytes())['files']:
        relative = Path(entry['path'])
        if relative.is_absolute() or '..' in relative.parts:
            raise ValueError(relative)
        path = manifest.parent / relative
        packed = path.with_name(path.name + '.gz')
        if path.exists() and packed.exists():
            raise ValueError(f'ambiguous archive entry: {path}')
        data = path.read_bytes() if path.exists() else gzip.decompress(packed.read_bytes())
        if len(data) != entry['bytes'] or hashlib.sha256(data).hexdigest() != entry['sha256']:
            raise ValueError(f'changed review bytes: {path}')
        count += 1
print(f'verified {count} original review payloads')
