import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
CTL = ROOT / 'scripts/voicevox_ctl.sh'

class ControllerTest(unittest.TestCase):
    def test_exclusive_owner_stop_and_status_without_killing_external_listener(self):
        with tempfile.TemporaryDirectory(prefix='vvm-engine-test-') as temp:
            fake = Path(temp) / 'fake-engine'
            fake.write_text('''#!/usr/bin/env python3
import argparse
from http.server import HTTPServer, BaseHTTPRequestHandler
p=argparse.ArgumentParser();p.add_argument('--host');p.add_argument('--port',type=int);a,_=p.parse_known_args()
class H(BaseHTTPRequestHandler):
 def do_GET(self):
  body=b'"fixture"';self.send_response(200);self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
HTTPServer.allow_reuse_address=True
HTTPServer((a.host,a.port),H).serve_forever()
''')
            fake.chmod(0o700)
            with socket.socket() as probe:
                probe.bind(('127.0.0.1', 0))
                port = probe.getsockname()[1]
            env = dict(os.environ, SUNGEKI_VOICEVOX_ENGINE=str(fake), SUNGEKI_VOICEVOX_PORT=str(port))
            owner = subprocess.Popen([str(CTL), 'start'], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            try:
                started = json.loads(owner.stdout.readline())
                self.assertEqual(started['state'], 'STARTING')
                def command(name):
                    return subprocess.run([str(CTL), name], env=env, text=True, capture_output=True, timeout=10)
                self.assertEqual(json.loads(command('wait').stdout)['state'], 'HEALTHY')
                duplicate = command('start')
                self.assertNotEqual(duplicate.returncode, 0)
                self.assertIn('別のSungeki', duplicate.stderr)
                self.assertEqual(json.loads(command('stop').stdout)['state'], 'STOPPED')
                self.assertEqual(json.loads(command('status').stdout)['state'], 'STOPPED')
                with socket.socket() as external:
                    external.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                    external.bind(('127.0.0.1', port)); external.listen(1)
                    self.assertEqual(json.loads(command('stop').stdout)['state'], 'STOPPED')
                    with socket.create_connection(('127.0.0.1', port), timeout=1):
                        pass
            finally:
                owner.send_signal(signal.SIGTERM)
                owner.wait(timeout=10)
                output = owner.stderr.read()
                owner.stdout.close(); owner.stderr.close()
            self.assertEqual(owner.returncode, 0, output)
            self.assertNotEqual(command('status').returncode, 0)
            with socket.socket() as external:
                external.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                external.bind(('127.0.0.1', port)); external.listen(1)
                result = command('start')
                self.assertNotEqual(result.returncode, 0)
                with socket.create_connection(('127.0.0.1', port), timeout=1):
                    pass

if __name__ == '__main__':
    unittest.main()
