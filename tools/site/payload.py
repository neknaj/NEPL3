"""Freeze an already checked site as a deterministic, link-free Pages tar.

The expected manifest digest comes from the checked build artifact. Packaging
does not grant publication/LKG status and does not rebuild or modify HTML.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import tarfile

MAX_BYTES = 32 * 1024 * 1024
MAX_FILES = 4096


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checked(condition, reason):
    if not condition:
        raise ValueError(reason)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        checked(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def linked(metadata):
    return stat.S_ISLNK(metadata.st_mode) or bool(
        getattr(metadata, 'st_file_attributes', 0) & getattr(stat, 'FILE_ATTRIBUTE_REPARSE_POINT', 0x400))


def real_directory(path):
    absolute = path.absolute()
    for parent in [*reversed(absolute.parents), absolute]:
        metadata = parent.lstat()
        checked(stat.S_ISDIR(metadata.st_mode) and not linked(metadata), 'linked or non-directory ancestor')


def walk_error(error):
    raise error


def path_name(name):
    checked(isinstance(name, str) and 0 < len(name) <= 4096, 'invalid path length')
    checked(re.fullmatch(r'[A-Za-z0-9._/-]+', name), 'invalid path characters')
    for part in name.split('/'):
        checked(part not in ('', '.', '..') and not part.endswith('.'), 'invalid path segment')
        stem = part.split('.')[0].upper()
        checked(stem not in {'CON', 'PRN', 'AUX', 'NUL'} and
                not re.fullmatch(r'(COM|LPT)[1-9]', stem), 'reserved path')
    return name


def snapshot(root, expected_manifest):
    checked(re.fullmatch(r'[0-9a-f]{64}', expected_manifest), 'invalid expected digest')
    real_directory(root)
    files = {}
    total = 0
    entries = 0
    for directory, dirs, names in os.walk(root, followlinks=False, onerror=walk_error):
        real_directory(Path(directory))
        entries += len(dirs) + len(names)
        checked(entries <= MAX_FILES * 2, 'entry limit')
        for child in dirs:
            checked(not linked((Path(directory) / child).lstat()), 'linked directory')
        for child in names:
            path = Path(directory) / child
            name = path_name(path.relative_to(root).as_posix())
            metadata = path.lstat()
            checked(stat.S_ISREG(metadata.st_mode) and not linked(metadata) and metadata.st_nlink == 1, 'linked or special file')
            checked(len(files) < MAX_FILES, 'file limit')
            with path.open('rb') as stream:
                actual = os.fstat(stream.fileno())
                checked(stat.S_ISREG(actual.st_mode) and not linked(actual) and actual.st_nlink == 1 and
                        (actual.st_dev, actual.st_ino) == (metadata.st_dev, metadata.st_ino), 'file changed before read')
                data = stream.read(MAX_BYTES - total + 1)
            total += len(data)
            checked(total <= MAX_BYTES, 'site byte limit')
            files[name] = data
    return validate_files(files, expected_manifest)


def validate_files(files, expected_manifest):
    checked(isinstance(files, dict) and 0 < len(files) <= MAX_FILES, 'file limit')
    checked(all(isinstance(data, bytes) for data in files.values()), 'file bytes required')
    checked(sum(map(len, files.values())) <= MAX_BYTES, 'site byte limit')
    checked(isinstance(expected_manifest, str) and re.fullmatch(r'[0-9a-f]{64}', expected_manifest), 'invalid expected digest')
    for name in files:
        path_name(name)
    all_names = {name.lower() for name in files}
    checked(len(all_names) == len(files), 'case-colliding files')
    for name in files:
        parts = name.split('/')
        for length in range(1, len(parts)):
            checked('/'.join(parts[:length]).lower() not in all_names, 'file/directory prefix collision')
    checked('manifest.json' in files, 'missing manifest')
    checked(digest(files['manifest.json']) == expected_manifest, 'manifest digest mismatch')
    manifest = json.loads(files['manifest.json'].decode('utf-8'), object_pairs_hook=unique_object)
    checked(isinstance(manifest, dict) and type(manifest.get('version')) is int and
            manifest['version'] == 1, 'unsupported manifest')
    records = manifest.get('files')
    checked(isinstance(records, list) and 0 < len(records) < MAX_FILES, 'invalid file records')
    declared = set()
    folded = set()
    prefixes = {}
    for record in records:
        checked(isinstance(record, dict), 'invalid file record')
        name = path_name(record.get('path'))
        checked(name not in declared and name.lower() not in folded, 'duplicate file')
        declared.add(name)
        folded.add(name.lower())
        parts = name.split('/')
        for length in range(1, len(parts) + 1):
            prefix = '/'.join(parts[:length])
            prior = prefixes.setdefault(prefix.lower(), prefix)
            checked(prior == prefix, 'case-colliding path prefix')
        checked(name != 'manifest.json' and name in files, 'missing or self-referential file')
        checked(type(record.get('bytes')) is int and record['bytes'] == len(files[name]), 'file size mismatch')
        checked(record.get('sha256') == digest(files[name]), 'file digest mismatch')
    checked(declared | {'manifest.json'} == files.keys(), 'unlisted site file')
    checked({'index.html', 'build.json', '.nojekyll'} <= declared, 'missing Pages entry')
    return files


def pack(root, expected_manifest, output):
    checked(not output.exists(), 'output already exists')
    checked(not output.resolve().is_relative_to(root.resolve()), 'output must be outside input site')
    files = snapshot(root, expected_manifest)
    payload = tar_bytes(files)
    with output.open('xb') as stream:
        stream.write(payload)
    return {'version': 1, 'kind': 'pages-tar', 'manifest_sha256': expected_manifest,
            'tar_sha256': digest(payload), 'tar_bytes': len(payload), 'files': len(files),
            'publication_verified': False}


def tar_bytes(files):
    # Only frozen byte buffers enter tarfile. No filesystem link/mtime/uid is
    # copied into the payload, including files modified after snapshot capture.
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode='w', format=tarfile.PAX_FORMAT) as archive:
        for name, data in sorted(files.items()):
            entry = tarfile.TarInfo(name)
            entry.size = len(data)
            entry.mode = 0o644
            entry.mtime = 0
            entry.uid = entry.gid = 0
            entry.uname = entry.gname = ''
            archive.addfile(entry, io.BytesIO(data))
    payload = buffer.getvalue()
    # Per-file headers/padding have a separate finite overhead allowance.
    checked(len(payload) <= MAX_BYTES + MAX_FILES * 2048, 'tar size limit')
    return payload


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('site', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--manifest-sha256', required=True)
    args = parser.parse_args()
    print(json.dumps(pack(args.site, args.manifest_sha256, args.output), sort_keys=True))
