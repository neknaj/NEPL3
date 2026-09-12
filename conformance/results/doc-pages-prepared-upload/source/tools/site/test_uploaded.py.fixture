import io
import json
import unittest
import zipfile

from deployment.artifact import uploaded
from payload import digest
import test_artifact


class UploadedTests(unittest.TestCase):
    def fixture(self, extra=False):
        meta, _, pins, tar = test_artifact.ArtifactTests().fixture()
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, 'w', compression=zipfile.ZIP_DEFLATED) as z:
            z.writestr('artifact.tar', tar)
            if extra: z.writestr('unexpected.txt', 'other')
        archive = buffer.getvalue()
        meta.update(name=f"github-pages-{pins['source_commit']}-8-1", size_in_bytes=len(archive), digest='sha256:'+digest(archive))
        receipt = dict(version=1, artifact_id=7, artifact_digest=digest(archive), run_id=8, attempt=1, publication_verified=False)
        return meta, archive, receipt, pins, tar

    def test_single_tar_is_original_and_extra_members_are_rejected(self):
        meta, archive, receipt, pins, tar = self.fixture()
        result = uploaded(json.dumps(meta).encode(), archive, json.dumps(receipt).encode(), **pins)
        self.assertEqual(result.data, tar)
        meta, archive, receipt, pins, _ = self.fixture(extra=True)
        with self.assertRaisesRegex(ValueError, 'one tar'):
            uploaded(json.dumps(meta).encode(), archive, json.dumps(receipt).encode(), **pins)

    def test_producer_receipt_and_pages_metadata_must_both_match(self):
        meta, archive, receipt, pins, _ = self.fixture()
        for key,value in [('artifact_id',8), ('run_id',9), ('attempt',2), ('version',True),
                          ('artifact_digest','0'*64), ('publication_verified',True)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                uploaded(json.dumps(meta).encode(), archive, json.dumps(dict(receipt, **{key:value})).encode(), **pins)
        meta['name'] = f"doc-browser-{pins['source_commit']}-8-1"
        with self.assertRaises(ValueError):
            uploaded(json.dumps(meta).encode(), archive, json.dumps(receipt).encode(), **pins)
