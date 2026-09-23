import json
import unittest

from deployment.artifact import receipt_value
from payload import Receipt
from tools.serialization.json import JsonValue


class ArtifactReceiptTests(unittest.TestCase):
    def test_receipt_fields_preserve_wire_values(self) -> None:
        expected = Receipt('a' * 64, 'b' * 64, 10240, 4)
        wire: dict[str, JsonValue] = dict(version=1, kind='pages-tar',
                    manifest_sha256='a' * 64, tar_sha256='b' * 64,
                    tar_bytes=10240, files=4, publication_verified=False)
        self.assertEqual(receipt_value(json.dumps(wire).encode()), expected)
        self.assertEqual(expected.representation(), wire)

    def test_invalid_field_is_rejected_with_other_fields_valid(self) -> None:
        cases: tuple[tuple[str, JsonValue], ...] = (
            ('version', True), ('version', 2), ('kind', 'other'), ('publication_verified', 0),
            ('publication_verified', True), ('tar_bytes', True), ('files', 4.0),
            ('manifest_sha256', []), ('tar_sha256', None),
        )
        for field, value in cases:
            wire = Receipt('a' * 64, 'b' * 64, 10240, 4).representation()
            wire[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                _ = receipt_value(json.dumps(wire).encode())


if __name__ == '__main__':
    _ = unittest.main()
