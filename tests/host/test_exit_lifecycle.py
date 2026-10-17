"""Real IPC exit ownership, idle cleanup, and watch-disconnect regression tests."""
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from support import HostProcess, connect, exchange, hello, read_exact

class ExitLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='next-exit-')
        self.root = Path(self.temp.name)
        self.host = HostProcess(self.root, binary=os.environ.get("NEXT_EXIT_TEST_HOST"))
        self.host.ready()
        self.client = connect(self.root)
        exchange(self.client, hello(minor=147))

    def tearDown(self):
        self.client.close()
        self.host.close()
        self.temp.cleanup()

    def action(self, action):
        return exchange(self.client, {'type': 'lifecycle', 'action': action})['status']

    def test_idle_stop_exits_without_thirty_second_delay(self):
        self.assertEqual(self.action('inspect')['active_jobs'], 0)
        started = time.monotonic()
        self.assertTrue(self.action('cancel_and_stop')['stopping'])
        self.host.process.wait(timeout=3)
        self.assertLess(time.monotonic() - started, 3)
        self.assertFalse((self.root/'host.sock').exists())

    def test_other_process_is_protected_until_it_disconnects(self):
        script = """
import sys
from pathlib import Path
from support import connect, exchange, hello
s=connect(Path(sys.argv[1])); exchange(s,hello(minor=147))
print('ready',flush=True); input(); s.close()
"""
        child = subprocess.Popen([sys.executable, '-c', script, str(self.root)],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
                                 cwd=Path(__file__).resolve().parent)
        try:
            self.assertEqual(child.stdout.readline().strip(), 'ready')
            status = self.action('cancel_and_stop')
            self.assertGreater(status['other_clients'], 0)
            self.assertFalse(status['stopping'])
            self.assertIsNone(self.host.process.poll())
        finally:
            child.communicate('\n', timeout=3)
        for _ in range(50):
            if not self.action('inspect')['other_clients']: break
            time.sleep(.02)
        self.assertTrue(self.action('cancel_and_stop')['stopping'])
        self.host.process.wait(timeout=3)

    def test_cancel_and_stop_reaps_owned_worker_and_preserves_cancel_record(self):
        from unittest.mock import patch
        from support import McpClient
        from exit_fixture import environment, start
        from test_render_jobs import call, wait_reclaimed
        self.client.close(); self.host.close()
        binary = os.environ.get('NEXT_EXIT_TEST_HOST')
        with patch.dict(os.environ, environment(self.root)):
            self.host = HostProcess(self.root, binary=binary); self.host.ready()
            mcp = McpClient(self.root, **({'binary':Path(binary)} if binary else {}))
            try: job, marker = start(mcp, self.root)
            finally: mcp.close()
            self.client = connect(self.root); exchange(self.client, hello(minor=147))
            for _ in range(50):
                if not self.action('inspect')['other_clients']: break
                time.sleep(.02)
            self.assertGreater(self.action('inspect')['active_jobs'], 0)
            self.assertFalse(self.action('stop_if_idle')['stopping'])
            self.assertFalse(self.action('when_idle')['stopping'])
            os.kill(int(marker.read_text()), 0)  # Background choice preserved worker.
            self.assertTrue(self.action('cancel_and_stop')['stopping'])
            self.host.process.wait(timeout=5)
            with self.assertRaises(ProcessLookupError): os.kill(int(marker.read_text()), 0)
            wait_reclaimed(marker.parent)
            self.client.close(); self.host.close()
            self.host = HostProcess(self.root, binary=binary); self.host.ready()
            self.client = connect(self.root); exchange(self.client, hello(minor=147))
            result = exchange(self.client, {'type':'session','request':{'type':'render_job','request':{'type':'get','job_id':job,'wait_ms':0}}})
            self.assertEqual(result['result']['job']['phase'], 'cancelled')
            self.assertIsNone(result['result']['job']['output'])

    def test_connected_watch_still_receives_changes(self):
        import struct
        import select
        watch = connect(self.root)
        try:
            exchange(watch, hello(minor=147))
            first = exchange(watch, {'type':'watch','after':None,'wait_ms':0})
            body = {'type':'watch','after':first['batch']['cursor'],'wait_ms':20000}
            payload = json.dumps({'request_id':3,'body':body}).encode()
            watch.sendall(struct.pack('>I',len(payload))+payload)
            self.assertEqual(select.select([watch], [], [], .1)[0], [])
            exchange(self.client, {'type':'session','request':{'type':'create','title':'終了確認・通知の維持'}})
            size = struct.unpack('>I', read_exact(watch,4))[0]
            response = json.loads(read_exact(watch,size))
            self.assertEqual(response['request_id'],3)
            self.assertTrue(response['body']['batch']['events'])
        finally:
            watch.close()

    def test_background_exit_drops_disconnected_watch_promptly(self):
        watch = connect(self.root)
        exchange(watch, hello(minor=147))
        import struct
        # Get a cursor, then ask for a genuine long poll with no new edits.
        first=exchange(watch,{'type':'watch','after':None,'wait_ms':0})
        payload=json.dumps({'request_id':3,'body':{'type':'watch','after':first['batch']['cursor'],'wait_ms':20000}}).encode()
        watch.sendall(struct.pack('>I',len(payload))+payload)
        self.assertFalse(self.action('when_idle')['stopping'])
        self.assertIsNone(self.host.process.poll())
        watch.close(); self.client.close()
        self.host.process.wait(timeout=3)

if __name__ == '__main__': unittest.main()
