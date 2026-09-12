import io
import json
import unittest
import zipfile

from deployment.candidate import prepare_upload
from payload import digest
import test_candidate_payload
import test_uploaded


class PreparedUploadTests(unittest.TestCase):
    def fixture(self):
        run, jobs, meta, archive, kwargs, tar = test_candidate_payload.CandidatePayloadTests().fixture()
        upload, upload_zip, receipt, _, _ = test_uploaded.UploadedTests().fixture()
        upload.update(id=17, url='https://api.github.com/repos/neknaj/NEPL3/actions/artifacts/17',
                      archive_download_url='https://api.github.com/repos/neknaj/NEPL3/actions/artifacts/17/zip')
        receipt['artifact_id'] = 17
        with zipfile.ZipFile(io.BytesIO(archive)) as z:
            files = {n:z.read(n) for n in z.namelist()}
        files['pages-upload.json'] = json.dumps(receipt).encode()
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, 'w') as z:
            for name,data in files.items(): z.writestr(name,data)
        archive = buffer.getvalue()
        meta.update(size_in_bytes=len(archive), digest='sha256:'+digest(archive))
        args = [json.dumps(run).encode(), json.dumps(jobs).encode(), json.dumps(meta).encode(),
                archive, json.dumps(upload).encode(), upload_zip]
        return args, dict(kwargs,upload_id=17), tar

    def test_ci_to_diagnostic_receipt_to_original_uploaded_tar(self):
        args, kwargs, tar = self.fixture()
        candidate, payload = prepare_upload(*args, **kwargs)
        self.assertEqual(payload.data, tar)
        self.assertEqual(candidate.run_id, kwargs['run_id'])

    def test_different_upload_or_corrupt_diagnostic_fails(self):
        args, kwargs, _ = self.fixture()
        for value in (7,18):
            with self.subTest(upload_id=value), self.assertRaises(ValueError):
                prepare_upload(*args, **dict(kwargs,upload_id=value))
        args[3] += b'x'
        with self.assertRaises(ValueError): prepare_upload(*args, **kwargs)
