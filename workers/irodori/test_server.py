import http.client
import json
import socket
import threading
import unittest
from server import MAX_REQUEST_BYTES, OwnedServer


class Bridge:
    def __init__(self):
        self.calls = []
        self.started = threading.Event()
        self.finish = threading.Event()
        self.wait = False
        self.fail = False
    def catalog(self):
        return {'model': {'checkpoint': 'fixture'}, 'voices': []}
    def synthesize_wav(self, payload):
        self.calls.append(payload)
        self.started.set()
        if self.wait and not self.finish.wait(5):
            raise RuntimeError('test wait expired')
        if self.fail:
            raise RuntimeError('/private/model/path should not be exposed')
        return b'fixture waveform; not a TTS output'


class ServerTests(unittest.TestCase):
    def setUp(self):
        self.listener = socket.socket()
        self.listener.bind(('127.0.0.1', 0))
        self.listener.listen()
        self.bridge = Bridge()
        self.server = OwnedServer(self.listener.fileno(), self.bridge)
        self.port = self.listener.getsockname()[1]
        self.thread = threading.Thread(target=self.server.serve_forever,
                                       kwargs={'poll_interval': .02})
        self.thread.start()
        self.addCleanup(self.cleanup)
    def cleanup(self):
        self.bridge.finish.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(3)
        self.assertFalse(self.thread.is_alive())
        self.listener.close()
    def request(self, path, body=None, headers=None):
        client = http.client.HTTPConnection('127.0.0.1', self.port, timeout=3)
        try:
            method = 'GET' if body is None else 'POST'
            client.request(method, path, body=body, headers=headers or {'Content-Type': 'application/json'})
            result = client.getresponse()
            return result.status, result.read(), result.getheader('Content-Type')
        finally:
            client.close()
    def test_catalog_health_and_waveform(self):
        self.assertEqual(self.request('/v1/catalog')[0], 200)
        self.assertFalse(json.loads(self.request('/v1/health')[1])['busy'])
        status, body, kind = self.request('/v1/synthesize', b'{"text":"fixture"}')
        self.assertEqual((status, kind), (200, 'audio/wav'))
        self.assertEqual(body, b'fixture waveform; not a TTS output')
        self.assertEqual(self.bridge.calls, [{'text': 'fixture'}])
        self.assertEqual(self.request('/synthesize', b'{}')[0], 404)
    def test_ambiguous_json_and_oversize_never_dispatch(self):
        for body in (b'{"text":1,"text":2}', b'{"seed":NaN}', b'\xff', b'{broken'):
            self.assertEqual(self.request('/v1/synthesize', body)[0], 422)
        self.assertEqual(self.request('/v1/synthesize', b'x', {
            'Content-Type': 'application/json', 'Content-Length': str(MAX_REQUEST_BYTES + 1)})[0], 413)
        self.assertEqual(self.request('/v1/synthesize', b'{}', {'Content-Type': 'text/plain'})[0], 415)
        self.assertEqual(self.bridge.calls, [])
    def test_busy_request_rejected_while_health_remains_responsive(self):
        self.bridge.wait = True
        results = []
        worker = threading.Thread(target=lambda: results.append(self.request('/v1/synthesize', b'{}')))
        worker.start()
        try:
            self.assertTrue(self.bridge.started.wait(2))
            self.assertTrue(json.loads(self.request('/v1/health')[1])['busy'])
            self.assertEqual(self.request('/v1/synthesize', b'{}')[0], 409)
        finally:
            self.bridge.finish.set()
            worker.join(3)
        self.assertEqual(len(self.bridge.calls), 1)
        self.assertEqual(results[0][0], 200)
    def test_runtime_failure_releases_lock_and_hides_path(self):
        self.bridge.fail = True
        status, body, _ = self.request('/v1/synthesize', b'{}')
        self.assertEqual(status, 500)
        self.assertNotIn(b'/private', body)
        self.bridge.fail = False
        self.assertEqual(self.request('/v1/synthesize', b'{}')[0], 200)
    def test_browser_and_wrong_host_rejected(self):
        for headers in ({'Origin': 'http://example.test'}, {'Host': 'example.test'}):
            self.assertEqual(self.request('/v1/synthesize', b'{}', headers)[0], 403)
        self.assertEqual(self.bridge.calls, [])
    def test_duplicate_lengths_and_chunked_body_rejected(self):
        for extra in ('Content-Length: 2\r\nContent-Length: 2',
                      'Transfer-Encoding: chunked\r\nContent-Length: 2'):
            with socket.create_connection(('127.0.0.1', self.port), timeout=2) as client:
                request = (f'POST /v1/synthesize HTTP/1.1\r\nHost: 127.0.0.1:{self.port}\r\n'
                           f'Content-Type: application/json\r\n{extra}\r\n\r\n{{}}')
                client.sendall(request.encode())
                self.assertIn(b' 400 ', client.recv(4096).split(b'\r\n')[0])
        self.assertEqual(self.bridge.calls, [])

    def test_client_disconnect_does_not_release_running_inference(self):
        self.bridge.wait = True
        client = socket.create_connection(('127.0.0.1', self.port), timeout=2)
        client.sendall((f'POST /v1/synthesize HTTP/1.1\r\nHost: 127.0.0.1:{self.port}\r\n'
                        'Content-Type: application/json\r\nContent-Length: 2\r\n\r\n{}').encode())
        try:
            self.assertTrue(self.bridge.started.wait(2))
        finally:
            client.close()
        self.assertTrue(json.loads(self.request('/v1/health')[1])['busy'])
        self.assertEqual(self.request('/v1/synthesize', b'{}')[0], 409)
        self.assertEqual(len(self.bridge.calls), 1)
        self.bridge.finish.set()

    def test_requires_listener_and_preserves_parent_descriptor(self):
        with socket.socket() as unbound:
            with self.assertRaises(ValueError):
                OwnedServer(unbound.fileno(), self.bridge)
        self.assertEqual(self.listener.getsockname(), ('127.0.0.1', self.port))


if __name__ == '__main__':
    unittest.main()
