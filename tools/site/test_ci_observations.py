import base64
import json
import subprocess
import unittest
from unittest.mock import patch

from deployment.observations import Query, Kind, read, _read
from deployment.transport import TransportError
from test_transport import server


class ObservationTests(unittest.TestCase):
    def test_fixed_routes_and_real_http(self):
        for kind, identity, attempt, suffix in [
            (Kind.MAIN, 0, 0, '/git/ref/heads/main'),
            (Kind.RUN, 7, 0, '/actions/runs/7'),
            (Kind.JOBS, 7, 2, '/actions/runs/7/attempts/2/jobs?per_page=100&page=1'),
            (Kind.ARTIFACT, 8, 0, '/actions/artifacts/8'),
        ]:
            with server(body=b'{"id":7}') as seen:
                result = _read(Query('neknaj', 'NEPL3', kind, identity, attempt), 'test-token')
            self.assertEqual(result, b'{"id":7}')
            self.assertEqual(seen[0][0], '/repos/neknaj/NEPL3' + suffix)
            self.assertEqual(seen[0][1]['Authorization'], 'Bearer test-token')

    def test_invalid_queries_never_open_socket(self):
        with patch('deployment.transport.HTTPSConnection') as connection:
            for query in [Query('../evil','NEPL3',Kind.RUN,1), Query('a','b','run',1),
                          Query('a','b',Kind.RUN,True), Query('a','b',Kind.JOBS,1,0),
                          Query('a','b',Kind.MAIN,1), Query('a','b',Kind.RUN,1,1)]:
                with self.assertRaises(ValueError): _read(query, 'token')
            connection.assert_not_called()

    def test_child_credentials_and_untrusted_output(self):
        query = Query('a', 'b', Kind.RUN, 1)
        with patch('deployment.observations.subprocess.run') as run:
            run.return_value = subprocess.CompletedProcess([], 0, base64.b64encode(b'{}'), b'')
            self.assertEqual(read(query, 'private-token', timeout=2), b'{}')
            self.assertNotIn('private-token', str(run.call_args.args))
            self.assertEqual(json.loads(run.call_args.kwargs['input'])['token'], 'private-token')
            self.assertEqual(run.call_args.kwargs['timeout'], 2)
            for raw in [b'[]', b'{"x":1,"x":2}', b' ' * 65537]:
                run.return_value.stdout = base64.b64encode(raw)
                with self.assertRaises(TransportError): read(query, 'private-token')
            run.side_effect = subprocess.TimeoutExpired('child', 2)
            with self.assertRaisesRegex(TransportError, 'deadline'): read(query, 'private-token', timeout=2)

    def test_http_failures_never_return_observations(self):
        query = Query('a', 'b', Kind.RUN, 1)
        for code, body in [(302,b'{}'), (403,b'secret'), (200,b'x'*65537)]:
            with server(code=code, body=body), self.assertRaises(TransportError):
                _read(query, 'private-token')


if __name__ == '__main__':
    unittest.main()
