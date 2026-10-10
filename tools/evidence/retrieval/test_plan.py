"""Synthetic status/identity transport plans; Rust judges formal acceptance."""
import json
from pathlib import Path
import tempfile
import unittest
from tools.evidence.retrieval.plan import requests
from tools.serialization.json import JsonValue


SNAPSHOT: dict[str, JsonValue] = {'design_revision': 'fixture-only', 'identity': {
    'profile': 'nepl3.repository-inputs/1', 'source_sha256': 'a' * 64, 'spec_sha256': 'b' * 64}}


def write(root: Path, name: str, value: JsonValue) -> None:
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    _ = path.write_text(json.dumps(value), encoding='utf-8')


def fixture(root: Path) -> None:
    write(root, 'implementation-status.json', {'tasks': [], 'acceptance': [
        {'id': 'A01', 'status': 'passed', 'evidence': ['conformance/results/A01.json']}]})
    write(root, 'conformance/results/A01.json', {'schema': 'nepl3.acceptance-evidence/1',
          'acceptance_id': 'A01', 'design_revision': SNAPSHOT['design_revision'],
          'identity': SNAPSHOT['identity'], 'result': 'passed', 'runs': [
              {'kind': 'review', 'log': 'dist/evidence/review.log', 'log_sha256': 'c' * 64}]})


class PlanTests(unittest.TestCase):
    def test_existing_owned_record_and_unexecuted_empty_plan(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name).resolve()
            fixture(root)
            result = requests(root, SNAPSHOT)
            self.assertEqual(len(result), 1)
            self.assertEqual(result[0].destination, 'dist/evidence/review.log')
            self.assertIsNone(result[0].artifact)
            write(root, 'implementation-status.json', {'tasks': [], 'acceptance': [
                {'id': 'A01', 'status': 'not-run', 'evidence': []}]})
            self.assertEqual(requests(root, SNAPSHOT), [])

    def test_stale_identity_and_duplicate_ownership_fail(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name).resolve()
            fixture(root)
            with self.assertRaises(ValueError):
                _ = requests(root, dict(SNAPSHOT, design_revision='different'))
            write(root, 'implementation-status.json', {'tasks': [
                {'id': 'T01', 'status': 'complete', 'evidence': ['conformance/results/A01.json']}], 'acceptance': [
                {'id': 'A01', 'status': 'passed', 'evidence': ['conformance/results/A01.json']}]})
            with self.assertRaises(ValueError):
                _ = requests(root, SNAPSHOT)

    def test_unexecuted_owner_cannot_trigger_retrieval(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name).resolve()
            fixture(root)
            for state in ('not-run', 'blocked', 'unknown'):
                write(root, 'implementation-status.json', {'tasks': [], 'acceptance': [
                    {'id': 'A01', 'status': state, 'evidence': ['conformance/results/A01.json']}]})
                with self.assertRaises(ValueError):
                    _ = requests(root, SNAPSHOT)

    def test_paths_and_duplicate_json_fail(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name).resolve()
            fixture(root)
            for path in ('../outside.json', 'conformance/results/../../outside.json', 'conformance/results/A01.log'):
                write(root, 'implementation-status.json', {'tasks': [], 'acceptance': [
                    {'id': 'A01', 'status': 'passed', 'evidence': [path]}]})
                with self.assertRaises(ValueError):
                    _ = requests(root, SNAPSHOT)
            fixture(root)
            _ = (root / 'conformance/results/A01.json').write_bytes(b'{"schema":"x","schema":"x"}')
            with self.assertRaises(ValueError):
                _ = requests(root, SNAPSHOT)


if __name__ == '__main__':
    _ = unittest.main()
