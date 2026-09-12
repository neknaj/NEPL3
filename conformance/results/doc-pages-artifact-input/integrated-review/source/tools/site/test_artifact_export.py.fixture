import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from artifact import export
import test_artifact


class ArtifactExportTests(unittest.TestCase):
    def test_command_writes_original_payload_and_never_overwrites(self):
        meta, archive, kwargs, tar = test_artifact.ArtifactTests().fixture()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            metadata = root / 'metadata.json'; metadata.write_text(json.dumps(meta), encoding='utf-8')
            zip_path = root / 'download.zip'; zip_path.write_bytes(archive)
            output = root / 'pages.tar'
            command = [sys.executable, '-I', str(Path(__file__).with_name('artifact.py')),
                       str(metadata), str(zip_path), str(output)]
            for key, value in kwargs.items():
                command += ['--' + key.replace('_', '-'), str(value)]
            result = subprocess.run(command, capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(json.loads(result.stdout)['publication_verified'])
            self.assertEqual(output.read_bytes(), tar)
            result = subprocess.run(command, capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(json.loads(result.stdout)['reason'], 'FileExistsError')
            self.assertEqual(output.read_bytes(), tar)

    def test_invalid_inputs_leave_output_absent(self):
        meta, archive, kwargs, _ = test_artifact.ArtifactTests().fixture()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            metadata = root / 'metadata.json'; metadata.write_text(json.dumps(meta), encoding='utf-8')
            zip_path = root / 'download.zip'; zip_path.write_bytes(archive + b'x')
            output = root / 'pages.tar'
            with self.assertRaises(ValueError):
                export(metadata, zip_path, output, **kwargs)
            self.assertFalse(output.exists())
            metadata.write_bytes(b'x' * 65537)
            with self.assertRaisesRegex(ValueError, 'input size limit'):
                export(metadata, zip_path, output, **kwargs)
            self.assertFalse(output.exists())
