import json
import unittest

from deployment.worker import request_value
from tools.serialization.json import JsonValue


class StatusWorkerTests(unittest.TestCase):
    def value(self) -> dict[str, JsonValue]:
        return dict(receipt=dict(deployment_id='abc123',
                    status_endpoint='https://api.github.com/repos/neknaj/NEPL3/pages/deployments/abc123',
                    response_sha256='a' * 64), token='private-token', timeout=2.5)

    def test_validated_fields_and_secret_representation(self) -> None:
        request = request_value(json.dumps(self.value()).encode())
        self.assertEqual(request.receipt.deployment_id, 'abc123')
        self.assertEqual(request.token, 'private-token')
        self.assertEqual(request.timeout, 2.5)
        self.assertNotIn('private-token', repr(request))

    def test_external_field_types_are_rejected(self) -> None:
        cases: tuple[tuple[str, JsonValue], ...] = (
            ('receipt', []), ('token', None), ('token', 12), ('timeout', True),
            ('timeout', '2'), ('timeout', None),
        )
        for field, value in cases:
            record = self.value()
            record[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                _ = request_value(json.dumps(record).encode())


if __name__ == '__main__':
    _ = unittest.main()
