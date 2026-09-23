"""Journal envelope values are checked independently of Git storage."""
import unittest

from journal.model import Event, decode, encode, event_record
from tools.serialization.json import JsonValue


class JournalModelTests(unittest.TestCase):
    def test_envelope_bytes_and_owned_fields(self) -> None:
        event = Event('DeployIntent', 'tx1', 7, 2, 'a' * 40, 'b' * 64)
        # Canonical field order and wire spelling predate typed decoding.
        expected = ('{"attempt":2,"evidence_sha256":"' + 'c' * 64 + '","kind":"DeployIntent",'
                    + '"payload_sha256":"' + 'b' * 64 + '","run_id":7,"sequence":1,'
                    + '"source_commit":"' + 'a' * 40 + '","transaction":"tx1","version":1}\n').encode()
        self.assertEqual(encode(event.record(1, 'c' * 64)), expected)
        self.assertEqual(event_record(expected, 1, 'c' * 64), event)

    def test_each_invalid_field_is_rejected_with_other_fields_valid(self) -> None:
        event = Event('DeployIntent', 'tx1', 7, 2, 'a' * 40, 'b' * 64)
        cases: tuple[tuple[str, JsonValue], ...] = (
            ('kind', 'Unknown'), ('kind', 1), ('transaction', '../escape'),
            ('run_id', True), ('run_id', 0), ('attempt', 2**32), ('attempt', 1.0),
            ('source_commit', 'a' * 39), ('payload_sha256', []), ('sequence', True),
            ('sequence', 2), ('version', True), ('version', 2), ('evidence_sha256', 'd' * 64),
        )
        for field, value in cases:
            record = event.record(1, 'c' * 64)
            record[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                _ = event_record(encode(record), 1, 'c' * 64)
        record = event.record(1, 'c' * 64)
        record['extra'] = None
        with self.assertRaisesRegex(ValueError, 'fields'):
            _ = event_record(encode(record), 1, 'c' * 64)
        with self.assertRaisesRegex(ValueError, 'noncanonical'):
            _ = event_record(encode(event.record(1, 'c' * 64)) + b' ', 1, 'c' * 64)

    def test_json_boundary_rejects_duplicate_and_nonfinite_values(self) -> None:
        for raw in (b'{"a":1,"a":2}', b'{"a":NaN}', b'{"a":1e999}'):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                _ = decode(raw)


if __name__ == '__main__':
    _ = unittest.main()
