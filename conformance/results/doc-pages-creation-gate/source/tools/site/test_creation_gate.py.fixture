import base64
from dataclasses import replace
import unittest

from deployment.journal import require_creation_receipts
from journal import Event
from journal.model import Snapshot, encode
from test_receipt_journal import RAW


class CreationGateTests(unittest.TestCase):
    def snapshot(self, events, raw=RAW):
        evidence = encode(dict(version=1, owner='neknaj', repository='NEPL3',
                               response=base64.b64encode(raw).decode('ascii')))
        return Snapshot('a'*40, tuple(events), tuple(evidence for _ in events))

    def gate(self, state):
        require_creation_receipts(state, owner='neknaj', repository='NEPL3')

    def test_pairs_validate_original_response_for_both_intent_kinds(self):
        self.gate(Snapshot(None, (), ()))
        for kind, receipt in [('DeployIntent', 'DeployReceipt'), ('RecoveryIntent', 'RecoveryReceipt')]:
            event = Event(kind, 'tx', 1, 1, 'a'*40, 'b'*64)
            self.gate(self.snapshot([event, replace(event, kind=receipt)]))
            with self.assertRaises(ValueError):
                self.gate(self.snapshot([event, replace(event, kind=receipt)], raw=b'{}'))

    def test_later_events_cannot_hide_unresolved_or_forged_pair(self):
        intent = Event('DeployIntent', 'tx', 1, 1, 'a'*40, 'b'*64)
        receipt = replace(intent, kind='DeployReceipt')
        cases = [[intent], [receipt], [intent, replace(intent, kind='Healthy')],
                 [intent, replace(receipt, transaction='other')],
                 [intent, replace(receipt, payload_sha256='c'*64)],
                 [intent, replace(receipt, kind='RecoveryReceipt')],
                 [intent, receipt, replace(intent, transaction='next')],
                 [intent, receipt, replace(intent, kind='Healthy'), receipt]]
        for events in cases:
            with self.subTest(events=events), self.assertRaises(ValueError):
                self.gate(self.snapshot(events))
