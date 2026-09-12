from contextlib import contextmanager
import hashlib
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import subprocess
import threading
import unittest
from unittest.mock import patch

from deployment.download import destination, download, _download
from deployment.observations import Query, Kind
from deployment.transport import TransportError


@contextmanager
def server(body=b'zip bytes', location='https://storage123.blob.core.windows.net/a?sig=private',
           status=200, headers=(), length=None):
    seen=[]
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append((self.path, dict(self.headers)))
            if self.path.startswith('/repos/'):
                self.send_response(302)
                self.send_header('Location', location)
                self.send_header('Content-Length','0')
                self.end_headers()
            else:
                self.send_response(status)
                self.send_header('Content-Type','application/zip')
                self.send_header('Content-Length', str(len(body) if length is None else length))
                for k,v in headers: self.send_header(k,v)
                self.end_headers()
                self.wfile.write(body)
        def log_message(self,*args): pass
    instance=ThreadingHTTPServer(('127.0.0.1',0),Handler)
    thread=threading.Thread(target=instance.serve_forever,daemon=True); thread.start()
    def connect(host,*,timeout):
        if host not in ('api.github.com','storage123.blob.core.windows.net'): raise AssertionError(host)
        return HTTPConnection('127.0.0.1',instance.server_port,timeout=timeout)
    try:
        with patch('deployment.download.HTTPSConnection',connect): yield seen
    finally:
        instance.shutdown(); instance.server_close(); thread.join()


class DownloadTests(unittest.TestCase):
    query=Query('neknaj','NEPL3',Kind.ARTIFACT,8)
    data=b'zip bytes'
    def options(self): return dict(size=len(self.data),sha256=hashlib.sha256(self.data).hexdigest())

    def test_real_http_preserves_bytes_without_forwarding_auth(self):
        with server() as seen:
            self.assertEqual(_download(self.query,'private-token',**self.options()),self.data)
        self.assertEqual(len(seen),2)
        self.assertEqual(seen[0][0],'/repos/neknaj/NEPL3/actions/artifacts/8/zip')
        self.assertEqual(seen[0][1]['Authorization'],'Bearer private-token')
        self.assertEqual(seen[1][0],'/a?sig=private')
        self.assertNotIn('Authorization',seen[1][1])
        self.assertNotIn('Cookie',seen[1][1])
        self.assertNotIn('Referer',seen[1][1])

    def test_redirect_destinations_fail_before_second_request(self):
        for url in ['http://storage123.blob.core.windows.net/a', 'https://127.0.0.1/a',
                    'https://storage123.blob.core.windows.net.evil/a',
                    'https://user@storage123.blob.core.windows.net/a',
                    'https://storage123.blob.core.windows.net:443/a',
                    'https://storage123.blob.core.windows.net/a#frag']:
            with server(location=url) as seen, self.assertRaises(ValueError):
                _download(self.query,'token',**self.options())
            self.assertEqual(len(seen),1)
        for url in ['https://storage123.blob.core.windows.net/a\n', 'https://storage123.blob.core.windows.net/\\a']:
            with self.assertRaises(ValueError): destination(url)

    def test_storage_redirect_framing_and_corruption_fail(self):
        for kwargs in [dict(status=302), dict(body=b'bad bytes'), dict(length=10),
                       dict(headers=[('Content-Length','9')]),
                       dict(headers=[('Transfer-Encoding','chunked')]),
                       dict(headers=[('Content-Encoding','gzip')])]:
            with server(**kwargs), self.assertRaises(ValueError):
                _download(self.query,'token',**self.options())

    def test_public_parent_rechecks_and_sanitizes(self):
        with patch('deployment.download.subprocess.run') as run:
            run.return_value=subprocess.CompletedProcess([],0,self.data,b'')
            self.assertEqual(download(self.query,'private-token',**self.options()),self.data)
            self.assertNotIn('private-token',str(run.call_args.args))
            run.return_value.stdout=b'bad bytes'
            with self.assertRaises(TransportError): download(self.query,'private-token',**self.options())
            run.side_effect=subprocess.TimeoutExpired('child',1)
            with self.assertRaisesRegex(TransportError,'deadline'): download(self.query,'private-token',**self.options())

    def test_invalid_input_never_starts_child(self):
        with patch('deployment.download.subprocess.run') as run:
            for opts in [dict(size=True),dict(size=0),dict(size=268435457),dict(sha256='x'),
                         dict(timeout=True),dict(timeout=float('nan')),dict(timeout=61)]:
                with self.assertRaises(ValueError): download(self.query,'token',**dict(self.options(),**opts))
            with self.assertRaises(ValueError): download(Query('a','b',Kind.MAIN),'token',**self.options())
            run.assert_not_called()


if __name__=='__main__': unittest.main()
