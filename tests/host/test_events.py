import concurrent.futures
import tempfile
import unittest
from pathlib import Path
from support import HostProcess, connect, exchange, hello
from test_sessions import call, edit_request


def watch(c, cursor, wait=0):
    return exchange(c, {'type': 'watch', 'after': cursor, 'wait_ms': wait})


class EventsTest(unittest.TestCase):
    def test_commit_wakes_subscriber_replay_failure_and_save(self):
        with tempfile.TemporaryDirectory(prefix='vvm-events-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            try:
                info = host.ready()
                with connect(root) as c:
                    exchange(c, hello())
                    snap = call(c, {'type': 'create', 'title': '通知確認'})['snapshot']
                    cursor = snap['event_cursor']
                    self.assertEqual(cursor['host_instance_id'], info['host_instance_id'])
                    self.assertEqual(watch(c, cursor)['batch']['events'], [])

                    def waiting():
                        with connect(root) as subscriber:
                            exchange(subscriber, hello())
                            return watch(subscriber, cursor, 1500)['batch']

                    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
                        future = pool.submit(waiting)
                        request = edit_request(snap['session_id'], 0, [{'type': 'rename_project', 'title': '更新'}])
                        edited = call(c, request)['snapshot']
                        batch = future.result(timeout=3)
                    self.assertFalse(batch['resync_required'])
                    self.assertEqual(batch['events'][0]['revision'], 1)
                    self.assertEqual(batch['events'][0]['type'], 'edited')
                    self.assertEqual(batch['cursor'], edited['event_cursor'])
                    self.assertEqual(watch(c, cursor)['batch'], batch)
                    # Duplicate successful operation and failed stale edit publish nothing.
                    call(c, request)
                    failed = call(c, edit_request(snap['session_id'], 0, [{'type': 'rename_project', 'title': 'stale'}]))
                    self.assertEqual(failed['error']['code'], 'CONFLICT')
                    self.assertEqual(watch(c, batch['cursor'])['batch']['events'], [])
                    saved = call(c, {'type': 'save', 'session_id': snap['session_id'], 'path': str(root/'events.vvm.json'), 'expected_revision': 1})['snapshot']
                    event = watch(c, batch['cursor'])['batch']['events'][0]
                    self.assertEqual(event['type'], 'saved')
                    self.assertEqual(event['revision'], 1)
                    self.assertFalse(saved['dirty'])
            finally:
                host.close()

    def test_resync_after_retention_gap_restart_and_invalid_wait(self):
        with tempfile.TemporaryDirectory(prefix='vvm-event-gap-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            try:
                host.ready()
                with connect(root) as c:
                    exchange(c, hello())
                    initial = watch(c, None)['batch']
                    self.assertTrue(initial['resync_required'])
                    for i in range(257):
                        call(c, {'type': 'create', 'title': f'gap-{i}'})
                    gap = watch(c, initial['cursor'])['batch']
                    self.assertTrue(gap['resync_required'])
                    self.assertEqual(gap['events'], [])
                    self.assertEqual(watch(c, gap['cursor'], 20)['batch']['events'], [])
                    self.assertEqual(watch(c, gap['cursor'], 20001)['error']['code'], 'VALIDATION_FAILED')
                host.process.kill()
                host.process.wait(timeout=5)
                host.close()
                host = HostProcess(root)
                host.ready()
                with connect(root) as c:
                    exchange(c, hello())
                    reset = watch(c, gap['cursor'])['batch']
                    self.assertTrue(reset['resync_required'])
                    self.assertNotEqual(reset['cursor']['host_instance_id'], gap['cursor']['host_instance_id'])
            finally:
                host.close()
