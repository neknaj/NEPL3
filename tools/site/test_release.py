import copy
import json
import unittest

from deployment.release import Asset, verify
from payload import digest


class ReleaseTests(unittest.TestCase):
    def fixture(self):
        downloads = {"payload.tar": b"original bytes", "identity.json": b"{}", "smoke.json": b'{"result":"passed"}'}
        assets = tuple(Asset(name, i+10, len(data), digest(data)) for i, (name, data) in enumerate(downloads.items()))
        prefix = "https://api.github.com/repos/neknaj/NEPL3/releases/"
        metadata = dict(id=7, url=prefix+"7", tag_name="site-recovery/tx-1", draft=False, immutable=True,
                        published_at="2026-09-12T00:00:00Z", assets=[dict(id=a.asset_id, name=a.name, url=prefix+"assets/"+str(a.asset_id),
                        size=a.size, digest="sha256:"+a.sha256, state="uploaded") for a in assets])
        kwargs = dict(owner="neknaj", repository="NEPL3", release_id=7, tag="site-recovery/tx-1", assets=assets, downloads=downloads)
        return metadata, kwargs

    def test_storage_identity_and_actual_downloads(self):
        data, kwargs = self.fixture()
        receipt = verify(json.dumps(data).encode(), **kwargs)
        self.assertEqual(receipt.release_id, 7)
        self.assertEqual(receipt.assets, kwargs["assets"])
        self.assertFalse(hasattr(receipt, "last_known_good"))
        # This is storage-byte validation only, not a valid tar/smoke assertion.

    def test_mutable_draft_wrong_release_or_corrupt_download_fails(self):
        data, kwargs = self.fixture()
        for key, value in [("immutable", False), ("immutable", 1), ("draft", True), ("id", True),
                           ("id", 8), ("tag_name", "site-recovery/other"), ("published_at", None)]:
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                verify(json.dumps(dict(data, **{key:value})).encode(), **kwargs)
        for name in kwargs["downloads"]:
            broken = dict(kwargs["downloads"]); broken[name] += b"x"
            with self.subTest(name=name), self.assertRaises(ValueError):
                verify(json.dumps(data).encode(), **dict(kwargs, downloads=broken))

    def test_incomplete_duplicate_or_wrong_asset_metadata_fails(self):
        data, kwargs = self.fixture()
        for key, value in [("id", 999), ("size", True), ("digest", "sha256:"+"0"*64),
                           ("state", "starter"), ("url", "https://evil.invalid/")]:
            altered = copy.deepcopy(data); altered["assets"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): verify(json.dumps(altered).encode(), **kwargs)
        altered = copy.deepcopy(data); altered["assets"][1] = altered["assets"][0]
        with self.assertRaises(ValueError): verify(json.dumps(altered).encode(), **kwargs)
        for rows in [[], data["assets"][:2], data["assets"]+[data["assets"][0]]]:
            with self.assertRaises(ValueError): verify(json.dumps(dict(data, assets=rows)).encode(), **kwargs)

    def test_malformed_types_and_duplicate_keys_are_rejected(self):
        data, kwargs = self.fixture()
        for value in [[], {}, None, True]:
            altered = copy.deepcopy(data); altered["assets"][0]["name"] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                verify(json.dumps(altered).encode(), **kwargs)
        for raw in [b'[]', b'{"id":7,"id":8}', b' ' * 65537, b'{"id":NaN}']:
            with self.assertRaises(ValueError): verify(raw, **kwargs)
