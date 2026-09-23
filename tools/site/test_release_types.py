import json
from types import MappingProxyType
import unittest

from deployment.release import Asset, AssetName, verify
from payload import digest
from tools.serialization.json import JsonValue


class ReleaseTypeTests(unittest.TestCase):
    def test_readonly_downloads_and_finite_asset_names(self) -> None:
        names: tuple[AssetName, ...] = ('payload.tar', 'identity.json', 'smoke.json')
        downloads = MappingProxyType({name: b'{}' for name in names})
        assets = tuple(Asset(name, index + 10, 2, digest(b'{}')) for index, name in enumerate(names))
        prefix = 'https://api.github.com/repos/neknaj/NEPL3/releases/'
        metadata: dict[str, JsonValue] = dict(id=7, url=prefix + '7', tag_name='site-recovery/tx-1',
                draft=False, immutable=True, published_at='2026-09-12T00:00:00Z',
                assets=[dict(id=asset.asset_id, name=asset.name, url=prefix + 'assets/' + str(asset.asset_id),
                             size=2, digest='sha256:' + asset.sha256, state='uploaded') for asset in assets])

        def check(value: dict[str, JsonValue]) -> None:
            raw = json.dumps(value).encode()
            result = verify(raw, owner='neknaj', repository='NEPL3', release_id=7,
                            tag='site-recovery/tx-1', assets=assets, downloads=downloads)
            self.assertEqual(result.assets, assets)
            self.assertEqual(result.metadata_sha256, digest(raw))

        check(metadata)
        # Valid timestamp shape still requires an actual calendar date.
        for date in ('2026-02-30T00:00:00Z', '2026-09-12T25:00:00Z', '2026-09-12T00:00:00+00:00'):
            with self.subTest(date=date), self.assertRaises(ValueError):
                check(dict(metadata, published_at=date))


if __name__ == '__main__':
    _ = unittest.main()
