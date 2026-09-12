"""Validate saved original Pages tar bytes without filesystem extraction.

Expected identities must come from the separately validated LKG record. This
verifier does not establish release immutability, current deployment identity,
or permission to restore. The original input bytes are returned unchanged.
"""
from dataclasses import dataclass
import io
import re
import tarfile

from payload import MAX_BYTES, MAX_FILES, checked, digest, path_name, tar_bytes, validate_files


@dataclass(frozen=True)
class Payload:
    data: bytes
    tar_sha256: str
    manifest_sha256: str
    files: int


def verify(data, expected_tar, expected_manifest):
    checked(isinstance(data, bytes) and 0 < len(data) <= MAX_BYTES + MAX_FILES * 2048, 'raw tar size limit')
    checked(isinstance(expected_tar, str) and re.fullmatch('[0-9a-f]{64}', expected_tar), 'invalid tar identity')
    checked(digest(data) == expected_tar, 'saved tar digest mismatch')
    files = {}
    total = 0
    # mode r: deliberately excludes compression wrappers. The recovery record
    # identifies the original raw Pages tar, not a transport-specific wrapper.
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:') as archive:
        for member in archive:
            checked(member.isfile() and not member.issparse(), 'non-regular recovery member')
            name = path_name(member.name)
            checked(name not in files and len(files) < MAX_FILES, 'duplicate or excessive tar members')
            checked(member.size >= 0 and total + member.size <= MAX_BYTES, 'tar content size limit')
            with archive.extractfile(member) as stream:
                value = stream.read(member.size + 1)
            checked(len(value) == member.size, 'truncated recovery member')
            total += len(value)
            files[name] = value
    validate_files(files, expected_manifest)
    # Reject hidden extra archives, trailing noncanonical bytes, unknown PAX
    # attributes and altered metadata. This comparison never replaces the
    # recovered payload with newly serialized bytes.
    checked(tar_bytes(files) == data, 'noncanonical saved tar')
    return Payload(data, expected_tar, expected_manifest, len(files))
