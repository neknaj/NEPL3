"""Bind downloaded Actions ZIP bytes to an independently selected run and payload.

The caller authenticates metadata/downloads and establishes workflow eligibility,
run attempt and successful checks. This boundary neither authorizes publication
nor equates the containing diagnostic artifact ID with a Pages upload ID.
"""
import io
import stat
import tarfile
import zipfile

from journal.model import decode, hex_id
from payload import checked, digest, path_name
from recovery import verify as verify_payload
from .receipt import endpoint

MAX_ARCHIVE = 256 * 1024 * 1024
MAX_ENTRIES = 8192


def selected(raw, archive, *, owner, repository, artifact_id, run_id,
             repository_id, source_commit, expected_tar, expected_manifest):
    """Return the original checked raw tar, never extract or rebuild files."""
    for value in (artifact_id, run_id, repository_id):
        checked(type(value) is int and 0 < value < 2**53, 'invalid artifact/run/repository ID')
    hex_id(source_commit, 40)
    checked(isinstance(raw, bytes) and len(raw) <= 65536, 'artifact metadata limit')
    checked(isinstance(archive, bytes) and 0 < len(archive) <= MAX_ARCHIVE, 'artifact archive limit')
    # Reuse repository-name validation, without interpreting a deployment ID.
    endpoint(owner, repository, '1')
    prefix = f'https://api.github.com/repos/{owner}/{repository}/actions/artifacts/{artifact_id}'
    meta = decode(raw)
    checked(isinstance(meta, dict), 'invalid artifact metadata')
    checked(type(meta.get('id')) is int and meta['id'] == artifact_id, 'artifact ID mismatch')
    checked(meta.get('url') == prefix and meta.get('archive_download_url') == prefix + '/zip', 'artifact URL mismatch')
    checked(meta.get('name') == 'doc-browser-' + source_commit and meta.get('expired') is False, 'artifact name or expiry mismatch')
    checked(type(meta.get('size_in_bytes')) is int and meta['size_in_bytes'] == len(archive), 'archive size mismatch')
    checked(meta.get('digest') == 'sha256:' + digest(archive), 'archive digest mismatch')
    run = meta.get('workflow_run')
    checked(isinstance(run, dict), 'missing artifact run')
    for key, value in [('id', run_id), ('repository_id', repository_id), ('head_repository_id', repository_id)]:
        checked(type(run.get(key)) is int and run[key] == value, 'artifact run identity mismatch')
    checked(run.get('head_sha') == source_commit, 'artifact source mismatch')
    with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
        members = bundle.infolist()
        checked(len(members) <= MAX_ENTRIES, 'archive entry limit')
        names = {}
        total = 0
        for member in members:
            name = member.filename[:-1] if member.is_dir() else member.filename
            path_name(name)
            checked(name.casefold() not in names, 'duplicate archive path')
            names[name.casefold()] = member.is_dir()
            mode = stat.S_IFMT(member.external_attr >> 16)
            checked(mode in (0, stat.S_IFREG, stat.S_IFDIR), 'linked archive member')
            checked(mode == 0 or (mode == stat.S_IFDIR) == member.is_dir(), 'archive member kind mismatch')
            checked(member.orig_filename == member.filename, 'truncated archive name')
            checked(not member.flag_bits & 1, 'encrypted archive member')
            total += member.file_size
            checked(0 <= member.file_size <= MAX_ARCHIVE and total <= MAX_ARCHIVE, 'expanded archive limit')
        for name in names:
            parts = name.split('/')
            for end in range(1, len(parts)):
                parent = '/'.join(parts[:end])
                checked(parent not in names or names[parent], 'archive file-prefix conflict')
        def read(name, limit):
            member = bundle.getinfo(name)
            checked(not member.is_dir() and member.file_size <= limit, 'selected member size limit')
            with bundle.open(member) as stream:
                data = stream.read(limit + 1)
            checked(len(data) == member.file_size, 'selected member length mismatch')
            return data
        receipt = decode(read('pages-receipt.json', 65536))
        checked(isinstance(receipt, dict) and type(receipt.get('version')) is int and receipt['version'] == 1,
                'invalid payload receipt')
        checked(receipt.get('kind') == 'pages-tar' and receipt.get('publication_verified') is False,
                'invalid payload receipt kind')
        checked(receipt.get('tar_sha256') == expected_tar and receipt.get('manifest_sha256') == expected_manifest,
                'payload receipt identity mismatch')
        payload = verify_payload(read('pages.tar', 40 * 1024 * 1024), expected_tar, expected_manifest)
        for key, value in [('tar_bytes', len(payload.data)), ('files', payload.files)]:
            checked(type(receipt.get(key)) is int and receipt[key] == value, 'payload receipt count mismatch')
    with tarfile.open(fileobj=io.BytesIO(payload.data), mode='r:') as tar:
        with tar.extractfile('build.json') as stream:
            build = decode(stream.read())
    checked(isinstance(build, dict) and build.get('source_commit') == source_commit, 'payload source mismatch')
    return payload
