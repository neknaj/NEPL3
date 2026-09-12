from dataclasses import replace
from pathlib import Path
import subprocess
import tempfile
import unittest

from journal import Event, append, load
from deployment.journal import record_created, latest_created, record_status, status_history

RAW = b'{ "id":"123", "status_url":"https://api.github.com/repos/neknaj/NEPL3/pages/deployments/123/status" }'


class ReceiptJournalTests(unittest.TestCase):
    def test_real_git_roundtrip_and_exact_original_response(self):
        for kind in ("DeployIntent", "RecoveryIntent"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                repo = Path(directory) / "journal.git"
                subprocess.run(["git", "init", "--bare", "--quiet", str(repo)], check=True)
                intent = Event(kind, "tx1", 23, 1, "a" * 40, "b" * 64)
                head = append(repo, None, intent, b'{"pending":true}')
                next_head, receipt = record_created(repo, head, intent, RAW, owner="neknaj", repository="NEPL3")
                actual_head, event, reloaded, raw = latest_created(repo, owner="neknaj", repository="NEPL3")
                self.assertEqual(actual_head, next_head)
                self.assertEqual(receipt, reloaded); self.assertEqual(raw, RAW)
                self.assertEqual(event, replace(intent, kind=kind.replace("Intent", "Receipt")))
                # Repeating a receipt must not append another event, nor erase it.
                with self.assertRaises(ValueError):
                    record_created(repo, head, intent, RAW, owner="neknaj", repository="NEPL3")
                self.assertEqual(load(repo).head, next_head)
                with self.assertRaises(ValueError): latest_created(repo, owner="other", repository="NEPL3")

    def test_wrong_intent_identity_or_response_leaves_journal_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "journal.git"
            subprocess.run(["git", "init", "--bare", "--quiet", str(repo)], check=True)
            intent = Event("DeployIntent", "tx1", 23, 1, "a" * 40, "b" * 64)
            head = append(repo, None, intent, b'{}')
            for candidate, raw in [(replace(intent, transaction="other"), RAW),
                                   (replace(intent, payload_sha256="c" * 64), RAW),
                                   (intent, RAW.replace(b'/123/status', b'/456/status'))]:
                with self.subTest(candidate=candidate), self.assertRaises(ValueError):
                    record_created(repo, head, candidate, raw, owner="neknaj", repository="NEPL3")
                self.assertEqual(load(repo).head, head)

    def test_replay_does_not_trust_generic_storage_event_names(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "journal.git"
            subprocess.run(["git", "init", "--bare", "--quiet", str(repo)], check=True)
            intent = Event("DeployIntent", "tx1", 23, 1, "a" * 40, "b" * 64)
            head = append(repo, None, intent, b'{}')
            append(repo, head, replace(intent, kind="DeployReceipt"), b'{}')
            with self.assertRaises(ValueError): latest_created(repo, owner="neknaj", repository="NEPL3")

    def test_large_valid_api_response_is_not_truncated_to_fit_journal(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "journal.git"
            subprocess.run(["git", "init", "--bare", "--quiet", str(repo)], check=True)
            intent = Event("DeployIntent", "tx1", 23, 1, "a" * 40, "b" * 64)
            head = append(repo, None, intent, b'{}')
            # Valid API response within its 64 KiB input bound, but its base64
            # envelope exceeds the independently fixed journal evidence bound.
            raw = RAW[:-1] + b', "extra":"' + b'x' * 50000 + b'"}'
            with self.assertRaisesRegex(ValueError, "evidence limit"):
                record_created(repo, head, intent, raw, owner="neknaj", repository="NEPL3")
            self.assertEqual(load(repo).head, head)

    def test_status_history_reloads_original_bytes_and_rejects_late_observation(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "journal.git"
            subprocess.run(["git", "init", "--bare", "--quiet", str(repo)], check=True)
            intent = Event("DeployIntent", "tx1", 23, 1, "a" * 40, "b" * 64)
            head = append(repo, None, intent, b'{}')
            head, receipt = record_created(repo, head, intent, RAW, owner="neknaj", repository="NEPL3")
            for status in [b'{"status":"deployment_queued"}', b'{ "status":"succeed" }']:
                head, result = record_status(repo, head, status, owner="neknaj", repository="NEPL3", request_url=receipt.status_endpoint)
            history = status_history(repo, owner="neknaj", repository="NEPL3")
            self.assertEqual(history.creation_response, RAW)
            self.assertEqual(history.observations[-1].raw_response, b'{ "status":"succeed" }')
            self.assertEqual(len(history.observations), 2)
            with self.assertRaises(ValueError):
                record_status(repo, head, b'{"status":"succeed"}', owner="neknaj", repository="NEPL3", request_url=receipt.status_endpoint)
            self.assertEqual(load(repo).head, head)

    def test_status_request_mismatch_and_forged_storage_observation_rejected(self):
        from journal.model import encode
        import base64
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "journal.git"
            subprocess.run(["git", "init", "--bare", "--quiet", str(repo)], check=True)
            intent = Event("DeployIntent", "tx1", 23, 1, "a" * 40, "b" * 64)
            head = append(repo, None, intent, b'{}')
            head, receipt = record_created(repo, head, intent, RAW, owner="neknaj", repository="NEPL3")
            wrong = receipt.status_endpoint + "-other"
            with self.assertRaises(ValueError):
                record_status(repo, head, b'{"status":"succeed"}', owner="neknaj", repository="NEPL3", request_url=wrong)
            self.assertEqual(load(repo).head, head)
            proof = encode(dict(version=1, request_url=wrong, response=base64.b64encode(b'{"status":"succeed"}').decode()))
            append(repo, head, replace(intent, kind="Observation"), proof)
            with self.assertRaises(ValueError): status_history(repo, owner="neknaj", repository="NEPL3")
