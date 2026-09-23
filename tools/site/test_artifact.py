import gzip
from collections.abc import Mapping
from dataclasses import dataclass, replace
import io
import json
from pathlib import Path
import tarfile
import unittest
import zipfile

from deployment.artifact import selected
from payload import digest
from recovery import Payload
from tools.serialization.json import JsonValue, decode, object_value, string


@dataclass(frozen=True, slots=True)
class Pins:
    source_commit: str
    expected_tar: str = 'cc053272f49f8d4d79fc756560df5401bfdc22b8c1e1a1177ae0250cdc265cd9'
    expected_manifest: str = '39000dd5b7aad48deae97741c5b077b44a242a0badb7b30e81b3c1c26f301446'

    def select(self, metadata: JsonValue, archive: bytes) -> Payload:
        return selected(json.dumps(metadata).encode(), archive, owner='neknaj', repository='NEPL3',
                        artifact_id=7, run_id=8, repository_id=9, source_commit=self.source_commit,
                        expected_tar=self.expected_tar, expected_manifest=self.expected_manifest)

    def command_options(self) -> tuple[str, ...]:
        return ('--owner', 'neknaj', '--repository', 'NEPL3', '--artifact-id', '7', '--run-id', '8',
                '--repository-id', '9', '--source-commit', self.source_commit,
                '--expected-tar', self.expected_tar, '--expected-manifest', self.expected_manifest)


type Entry = tuple[str | zipfile.ZipInfo, bytes]


class ArtifactTests(unittest.TestCase):
    def fixture(self, extra: Entry | None = None,
                receipt_change: Mapping[str, JsonValue] | None = None) -> tuple[dict[str, JsonValue], bytes, Pins, bytes]:
        root = Path(__file__).resolve().parents[2]
        tar = gzip.decompress((root / 'tools/site/fixtures/pages.tar.gz.fixture').read_bytes())
        with tarfile.open(fileobj=io.BytesIO(tar)) as source:
            stream = source.extractfile('build.json')
            if stream is None:
                raise ValueError('fixture build.json must be a file')
            with stream:
                commit = string(object_value(decode(stream.read()))['source_commit'])
        # Both expected hashes predate this implementation (14 real Doc chapters).
        pins = Pins(commit)
        receipt: dict[str, JsonValue] = dict(version=1, kind='pages-tar', publication_verified=False, tar_sha256=pins.expected_tar,
                       manifest_sha256=pins.expected_manifest, tar_bytes=len(tar), files=22)
        receipt.update(receipt_change or {})
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, 'w', compression=zipfile.ZIP_DEFLATED) as bundle:
            bundle.writestr('pages.tar', tar)
            bundle.writestr('pages-receipt.json', json.dumps(receipt))
            if extra: bundle.writestr(*extra)
        archive = buffer.getvalue()
        url = 'https://api.github.com/repos/neknaj/NEPL3/actions/artifacts/7'
        meta: dict[str, JsonValue] = dict(id=7, name='doc-browser-'+commit, url=url, archive_download_url=url+'/zip', expired=False,
                    size_in_bytes=len(archive), digest='sha256:'+digest(archive),
                    workflow_run=dict(id=8, repository_id=9, head_repository_id=9, head_sha=commit))
        return meta, archive, pins, tar

    def test_real_doc_tar_is_preserved(self) -> None:
        meta, archive, pins, tar = self.fixture()
        result = pins.select(meta, archive)
        self.assertEqual(result.data, tar)
        self.assertEqual(result.files, 22)

    def test_metadata_substitution_and_archive_corruption(self) -> None:
        meta, archive, pins, _ = self.fixture()
        changes: tuple[tuple[str, JsonValue], ...] = (('id', True), ('expired', True), ('name', 'other'), ('digest', 'sha256:'+'0'*64),
                           ('size_in_bytes', 1), ('archive_download_url', 'https://other.invalid/'))
        for key, value in changes:
            with self.subTest(key=key), self.assertRaises(ValueError):
                _ = pins.select(dict(meta, **{key:value}), archive)
        for key in ('id', 'repository_id', 'head_repository_id', 'head_sha'):
            changed = dict(meta, workflow_run=dict(object_value(meta['workflow_run']), **{key:0}))
            with self.subTest(key=key), self.assertRaises(ValueError):
                _ = pins.select(changed, archive)
        with self.assertRaises(ValueError):
            _ = pins.select(meta, archive+b'x')

    def test_untrusted_receipt_cannot_override_expected_identity(self) -> None:
        changes: tuple[dict[str, JsonValue], ...] = (dict(files=True), dict(tar_bytes=0), dict(version=True),
                       dict(tar_sha256='0'*64), dict(publication_verified=True))
        for change in changes:
            meta, archive, pins, _ = self.fixture(receipt_change=change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                _ = pins.select(meta, archive)

    def test_unsafe_and_duplicate_zip_entries(self) -> None:
        link = zipfile.ZipInfo('alias'); link.create_system = 3; link.external_attr = 0o120777 << 16
        entries: tuple[Entry, ...] = (('../escape', b'x'), ('PAGES.TAR', b'x'), (link, b'pages.tar'))
        for entry in entries:
            meta, archive, pins, _ = self.fixture(extra=entry)
            with self.subTest(entry=str(entry[0])), self.assertRaises(ValueError):
                _ = pins.select(meta, archive)

    def test_matching_metadata_does_not_change_payload_source(self) -> None:
        meta, archive, pins, _ = self.fixture()
        pins = replace(pins, source_commit='0'*40)
        meta['name'] = 'doc-browser-'+'0'*40
        meta['workflow_run'] = dict(object_value(meta['workflow_run']), head_sha='0'*40)
        with self.assertRaisesRegex(ValueError, 'payload source'):
            _ = pins.select(meta, archive)

    def test_file_prefix_and_mismatched_directory_mode_are_rejected(self) -> None:
        wrong_kind = zipfile.ZipInfo('directory'); wrong_kind.create_system = 3
        wrong_kind.external_attr = 0o040755 << 16
        entries: tuple[Entry, ...] = (('pages.tar/child', b'x'), (wrong_kind, b''))
        for entry in entries:
            meta, archive, pins, _ = self.fixture(extra=entry)
            with self.subTest(entry=str(entry[0])), self.assertRaises(ValueError):
                _ = pins.select(meta, archive)
