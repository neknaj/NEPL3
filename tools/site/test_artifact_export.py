import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from artifact import export
import test_artifact
from tools.serialization.json import decode, object_value


class ArtifactExportTests(unittest.TestCase):
    def test_command_writes_original_payload_and_never_overwrites(self) -> None:
        meta, archive, pins, tar = test_artifact.ArtifactTests().fixture()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            metadata = root / 'metadata.json'; _ = metadata.write_text(json.dumps(meta), encoding='utf-8')
            zip_path = root / 'download.zip'; _ = zip_path.write_bytes(archive)
            output = root / 'pages.tar'
            command = [sys.executable, '-I', str(Path(__file__).with_name('artifact.py')),
                       str(metadata), str(zip_path), str(output)]
            command.extend(pins.command_options())
            result = subprocess.run(command, capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(object_value(decode(result.stdout))['publication_verified'])
            self.assertEqual(output.read_bytes(), tar)
            result = subprocess.run(command, capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(object_value(decode(result.stdout))['reason'], 'FileExistsError')
            self.assertEqual(output.read_bytes(), tar)

    def test_invalid_inputs_leave_output_absent(self) -> None:
        meta, archive, pins, _ = test_artifact.ArtifactTests().fixture()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            metadata = root / 'metadata.json'; _ = metadata.write_text(json.dumps(meta), encoding='utf-8')
            zip_path = root / 'download.zip'; _ = zip_path.write_bytes(archive + b'x')
            output = root / 'pages.tar'
            with self.assertRaises(ValueError):
                _ = export(metadata, zip_path, output, owner='neknaj', repository='NEPL3',
                           artifact_id=7, run_id=8, repository_id=9, source_commit=pins.source_commit,
                           expected_tar=pins.expected_tar, expected_manifest=pins.expected_manifest)
            self.assertFalse(output.exists())
            _ = metadata.write_bytes(b'x' * 65537)
            with self.assertRaisesRegex(ValueError, 'input size limit'):
                _ = export(metadata, zip_path, output, owner='neknaj', repository='NEPL3',
                           artifact_id=7, run_id=8, repository_id=9, source_commit=pins.source_commit,
                           expected_tar=pins.expected_tar, expected_manifest=pins.expected_manifest)
            self.assertFalse(output.exists())
