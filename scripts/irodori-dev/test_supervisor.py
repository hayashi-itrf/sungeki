"""Owned dummy HTTP child only; no real Irodori model or engine."""
import json
import os
from pathlib import Path
import select
import socket
import subprocess
import sys
import tempfile
import time
import unittest

SOURCE = Path(__file__).resolve().parent
CHILD = '''
import http.server, json, os, socket
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        data = json.dumps(dict(status='ready', busy=False) if self.path == '/v1/health' else dict(model=dict(checkpoint='fixture'), voices=[])).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def log_message(self, *args): pass
server = http.server.HTTPServer(('127.0.0.1', 0), Handler, bind_and_activate=False)
server.socket.close()
server.socket = socket.socket(fileno=os.dup(0))
server.serve_forever()
'''


class SupervisorTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory(prefix='irodori-owner-', dir='/tmp')
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.directory = self.root / 'control'
        self.directory.mkdir(mode=0o700)
        self.child = self.root / 'child.py'
        self.child.write_text(CHILD)
        self.children = []
        self.addCleanup(self.close)
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            self.port = listener.getsockname()[1]

    def close(self):
        for child in self.children:
            if child.poll() is None:
                child.terminate()
            child.wait(timeout=10)
            child.stdout.close()
            child.stderr.close()

    def start(self, model='fixture', slow=False):
        profile = dict(port=self.port, identity=dict(checkpoint=model), model=dict(checkpoint=model), voices=[],
                       binding=dict(work_root=str(self.root)))
        setup = ''
        if slow:
            host = self.root / 'target/release/sungeki-host'
            host.parent.mkdir(parents=True)
            host.write_text(f'#!{sys.executable}\nimport os, time\n'
                            f'open({str(self.root / "preflight.pid")!r}, "w").write(str(os.getpid()))\n'
                            'time.sleep(60)\n')
            host.chmod(0o700)
            setup = (f'import profile as p\np.ROOT=Path({str(self.root)!r})\n'
                     f'def inspect(cancel):\n    p.inspect_candidate(Path("unused"), cancel)\n    return {profile!r}\n'
                     's.load=inspect\n')
        script = (f'import sys; sys.path.insert(0, {str(SOURCE)!r}); import supervisor as s\n'
                  f'from pathlib import Path\ns.load=lambda cancel: {profile!r}\n'
                  f's.runtime=lambda: Path({str(self.root)!r})\n'
                  f's.command=lambda p: [{sys.executable!r}, "-B", {str(self.child)!r}]\n'
                  + setup +
                  f's.run(Path({str(self.directory)!r}))\n')
        process = subprocess.Popen([sys.executable, '-B', '-c', script],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.children.append(process)
        return process

    def request(self, command):
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(10)
            connection.connect(str(self.directory / 'control.sock'))
            connection.sendall((command + '\n').encode())
            return json.loads(connection.recv(8192))

    def started(self, process):
        self.assertTrue(select.select([process.stdout], [], [], 10)[0])
        line = process.stdout.readline()
        self.assertTrue(line, 'owner did not report startup')
        return json.loads(line)

    def test_start_wait_stop_status_and_exclusive_owner(self):
        owner = self.start()
        started = self.started(owner)
        deadline = time.monotonic() + 10
        while self.request('status')['state'] != 'HEALTHY':
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.05)
        second = self.start()
        self.assertNotEqual(second.wait(timeout=5), 0)
        self.assertEqual(self.request('status')['state'], 'HEALTHY')
        self.assertEqual(self.request('stop')['state'], 'STOPPED')
        self.assertEqual(self.request('status')['state'], 'STOPPED')
        with self.assertRaises(ProcessLookupError):
            os.kill(started['pid'], 0)

    def test_wrong_model_never_becomes_healthy_and_signal_cleans_up(self):
        owner = self.start(model='wrong')
        started = self.started(owner)
        time.sleep(.15)
        self.assertEqual(self.request('status')['state'], 'STARTING')
        owner.terminate()
        self.assertEqual(owner.wait(timeout=10), 0)
        with self.assertRaises(ProcessLookupError):
            os.kill(started['pid'], 0)
        self.assertFalse((self.directory / 'control.sock').exists())

    def test_existing_port_is_not_adopted_or_stopped(self):
        with socket.socket() as external:
            external.bind(('127.0.0.1', self.port))
            external.listen()
            owner = self.start()
            self.assertNotEqual(owner.wait(timeout=5), 0)
            with socket.create_connection(('127.0.0.1', self.port), timeout=1):
                accepted, _ = external.accept()
                accepted.close()
            self.assertFalse((self.directory / 'control.sock').exists())

    def test_stop_during_original_inspection_reaps_preflight_without_engine(self):
        owner = self.start(slow=True)
        marker = self.root / 'preflight.pid'
        deadline = time.monotonic() + 10
        while not marker.exists():
            self.assertIsNone(owner.poll())
            self.assertLess(time.monotonic(), deadline)
            time.sleep(.05)
        pid = int(marker.read_text())
        self.assertEqual(self.request('status')['state'], 'STARTING')
        self.assertEqual(self.request('stop')['state'], 'STOPPED')
        self.assertEqual(self.request('status')['state'], 'STOPPED')
        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)
        self.assertFalse((self.root / 'irodori-dev').exists())


if __name__ == '__main__':
    unittest.main()
