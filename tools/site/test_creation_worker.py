import json
import unittest

from deployment.create import validate
from deployment.create_worker import request_value
from tools.serialization.json import JsonValue


class CreationWorkerTests(unittest.TestCase):
    def value(self) -> dict[str, JsonValue]:
        return dict(owner='neknaj', repository='NEPL3', artifact_id=42, build_version='a' * 40,
                    token='private-token', oidc_token='private.oidc.token', timeout=2)

    def test_valid_request_and_secret_repr(self) -> None:
        request = request_value(json.dumps(self.value()).encode())
        self.assertEqual((request.owner, request.repository, request.artifact_id, request.timeout),
                         ('neknaj', 'NEPL3', 42, 2))
        credentials = validate(request.owner, request.repository, request.artifact_id,
                               request.build_version, request.token, request.oidc_token, request.timeout)
        self.assertEqual((credentials.token, credentials.oidc_token), ('private-token', 'private.oidc.token'))
        self.assertNotIn('private', repr(request))
        self.assertNotIn('private', repr(credentials))

    def test_field_types_and_unknown_keys_are_rejected(self) -> None:
        cases: tuple[tuple[str, JsonValue], ...] = (
            ('artifact_id', True), ('owner', []), ('token', None), ('oidc_token', 42),
            ('timeout', True), ('timeout', '2'), ('extra', None),
        )
        for field, value in cases:
            record = self.value()
            record[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                _ = request_value(json.dumps(record).encode())


if __name__ == '__main__':
    _ = unittest.main()
