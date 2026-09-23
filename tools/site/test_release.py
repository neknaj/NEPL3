from collections.abc import Mapping
from dataclasses import dataclass, replace
import gzip
import json
from pathlib import Path
import tarfile
from types import MappingProxyType
import unittest

from deployment.release import Asset, AssetName, StorageReceipt, verify, recover
from payload import digest
from recovery import Payload
from tools.serialization.json import JsonValue, array, object_value


@dataclass(frozen=True, slots=True)
class Fixture:
    assets: tuple[Asset, ...]
    downloads: Mapping[str, bytes]

    def metadata(self) -> dict[str, JsonValue]:
        prefix = "https://api.github.com/repos/neknaj/NEPL3/releases/"
        return dict(id=7, url=prefix+"7", tag_name="site-recovery/tx-1", draft=False, immutable=True,
                    published_at="2026-09-12T00:00:00Z", assets=[dict(id=a.asset_id, name=a.name,
                    url=prefix+"assets/"+str(a.asset_id), size=a.size, digest="sha256:"+a.sha256,
                    state="uploaded") for a in self.assets])

    def verify(self, raw: bytes) -> StorageReceipt:
        return verify(raw, owner="neknaj", repository="NEPL3", release_id=7,
                      tag="site-recovery/tx-1", assets=self.assets, downloads=self.downloads)

    def recover(self, raw: bytes, manifest: str) -> tuple[StorageReceipt, Payload]:
        return recover(raw, expected_manifest=manifest, owner="neknaj", repository="NEPL3",
                       release_id=7, tag="site-recovery/tx-1", assets=self.assets, downloads=self.downloads)


def fixture(payload: bytes = b"original bytes") -> Fixture:
    downloads: dict[AssetName, bytes] = {
        "payload.tar": payload, "identity.json": b"{}", "smoke.json": b'{"result":"passed"}',
    }
    assets = tuple(Asset(name, i+10, len(data), digest(data))
                   for i, (name, data) in enumerate(downloads.items()))
    return Fixture(assets, MappingProxyType({str(name): data for name, data in downloads.items()}))


class ReleaseTests(unittest.TestCase):
    def test_recover_returns_original_real_doc_tar_after_both_checks(self) -> None:
        root = Path(__file__).resolve().parents[2]
        data = gzip.decompress((root / "tools/site/fixtures/pages.tar.gz.fixture").read_bytes())
        stored = fixture(data)
        metadata = stored.metadata()
        raw = json.dumps(metadata).encode()
        # Previously archived 14-chapter site; identity predates this adapter.
        manifest = "39000dd5b7aad48deae97741c5b077b44a242a0badb7b30e81b3c1c26f301446"
        receipt, payload = stored.recover(raw, manifest)
        self.assertEqual(receipt.release_id, 7)
        self.assertIs(payload.data, data)
        self.assertEqual(payload.files, 22)
        self.assertEqual(payload.tar_sha256, "cc053272f49f8d4d79fc756560df5401bfdc22b8c1e1a1177ae0250cdc265cd9")
        with self.assertRaisesRegex(ValueError, "manifest digest"):
            _ = stored.recover(raw, "0" * 64)
        metadata["immutable"] = False
        with self.assertRaisesRegex(ValueError, "immutable"):
            _ = stored.recover(json.dumps(metadata).encode(), manifest)

    def test_matching_storage_hash_does_not_make_invalid_tar_recoverable(self) -> None:
        stored = fixture(b"not a tar archive")
        raw = json.dumps(stored.metadata()).encode()
        _ = stored.verify(raw)  # Storage integrity alone is insufficient.
        with self.assertRaises(tarfile.ReadError):
            _ = stored.recover(raw, "0" * 64)

    def test_storage_identity_and_actual_downloads(self) -> None:
        stored = fixture()
        receipt = stored.verify(json.dumps(stored.metadata()).encode())
        self.assertEqual(receipt.release_id, 7)
        self.assertEqual(receipt.assets, stored.assets)
        self.assertFalse(hasattr(receipt, "last_known_good"))
        # This is storage-byte validation only, not a valid tar/smoke assertion.

    def test_mutable_draft_wrong_release_or_corrupt_download_fails(self) -> None:
        stored = fixture()
        changes: tuple[tuple[str, JsonValue], ...] = (
            ("immutable", False), ("immutable", 1), ("draft", True), ("id", True),
            ("id", 8), ("tag_name", "site-recovery/other"), ("published_at", None),
        )
        for key, value in changes:
            altered = stored.metadata()
            altered[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                _ = stored.verify(json.dumps(altered).encode())
        for name in stored.downloads:
            broken = dict(stored.downloads)
            broken[name] += b"x"
            changed = replace(stored, downloads=MappingProxyType(broken))
            with self.subTest(name=name), self.assertRaises(ValueError):
                _ = changed.verify(json.dumps(stored.metadata()).encode())

    def test_incomplete_duplicate_or_wrong_asset_metadata_fails(self) -> None:
        stored = fixture()
        changes: tuple[tuple[str, JsonValue], ...] = (
            ("id", 999), ("size", True), ("digest", "sha256:"+"0"*64),
            ("state", "starter"), ("url", "https://evil.invalid/"),
        )
        for key, value in changes:
            altered = stored.metadata()
            rows = list(array(altered["assets"]))
            row = dict(object_value(rows[0]))
            row[key] = value
            rows[0] = row
            altered["assets"] = rows
            with self.subTest(key=key), self.assertRaises(ValueError):
                _ = stored.verify(json.dumps(altered).encode())
        altered = stored.metadata()
        rows = list(array(altered["assets"]))
        rows[1] = rows[0]
        altered["assets"] = rows
        with self.assertRaises(ValueError):
            _ = stored.verify(json.dumps(altered).encode())
        original = array(stored.metadata()["assets"])
        variations: tuple[list[JsonValue], ...] = ([], list(original[:2]), [*original, original[0]])
        for rows in variations:
            altered = stored.metadata()
            altered["assets"] = rows
            with self.assertRaises(ValueError):
                _ = stored.verify(json.dumps(altered).encode())

    def test_malformed_types_and_duplicate_keys_are_rejected(self) -> None:
        stored = fixture()
        values: tuple[JsonValue, ...] = ([], {}, None, True)
        for value in values:
            altered = stored.metadata()
            rows = list(array(altered["assets"]))
            row = dict(object_value(rows[0]))
            row["name"] = value
            rows[0] = row
            altered["assets"] = rows
            with self.subTest(value=value), self.assertRaises(ValueError):
                _ = stored.verify(json.dumps(altered).encode())
        for raw in [b'[]', b'{"id":7,"id":8}', b' ' * 65537, b'{"id":NaN}']:
            with self.assertRaises(ValueError):
                _ = stored.verify(raw)
