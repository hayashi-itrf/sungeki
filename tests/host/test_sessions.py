import tempfile
import unittest
import uuid
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
from support import HostProcess, connect, exchange, hello


def uid():
    return str(uuid.uuid4())


def call(connection, request):
    body = exchange(connection, {'type': 'session', 'request': request})
    if body['type'] == 'error':
        return body
    return body['result']


def edit_request(session_id, revision, commands, operation_id=None):
    return {'type': 'edit', 'session_id': session_id, 'edit': {
        'operation_id': operation_id or uid(), 'expected_revision': revision,
        'action': {'type': 'apply', 'commands': commands}}}


class SessionsTest(unittest.TestCase):
    def test_shared_edits_conflict_atomicity_and_crash_recovery(self):
        with tempfile.TemporaryDirectory(prefix='vvm-sessions-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            try:
                host.ready()
                with connect(root) as first, connect(root) as second:
                    exchange(first, hello())
                    exchange(second, hello())
                    created = call(first, {'type': 'create', 'title': '共通文書と復旧の検証'})['snapshot']
                    sid = created['session_id']
                    requests = [edit_request(sid, 0, [{'type': 'rename_project', 'title': title}])
                                for title in ['GUIから変更', 'MCPから変更']]
                    with ThreadPoolExecutor(2) as executor:
                        a = executor.submit(call, first, requests[0])
                        b = executor.submit(call, second, requests[1])
                        results = [a.result(), b.result()]
                    success = next(i for i, result in enumerate(results) if result['type'] == 'edited')
                    self.assertEqual(results[1-success]['error']['code'], 'CONFLICT')
                    receipt = results[success]['receipt']
                    current = call(second, {'type': 'get', 'session_id': sid})['snapshot']
                    self.assertEqual(current['revision'], 1)
                    rejected = call(first, edit_request(sid, 1, [
                        {'type': 'rename_project', 'title': '部分適用禁止'},
                        {'type': 'delete_clip', 'clip_id': uid()}]))
                    self.assertEqual(rejected['error']['code'], 'VALIDATION_FAILED')
                    self.assertEqual(call(second, {'type': 'get', 'session_id': sid})['snapshot'], current)
                    undo = {'type': 'edit', 'session_id': sid, 'edit': {
                        'operation_id': uid(), 'expected_revision': 1,
                        'action': {'type': 'undo', 'expected_operation_id': receipt['operation_id']}}}
                    restored = call(second, undo)['snapshot']
                    self.assertEqual(restored['project']['title'], created['project']['title'])
                    self.assertEqual(restored['revision'], 2)
                host.process.kill()
                host.process.wait(timeout=5)
                host.close()
                host = HostProcess(root)
                host.ready()
                with connect(root) as connection:
                    exchange(connection, hello())
                    recovered = call(connection, {'type': 'get', 'session_id': sid})['snapshot']
                    self.assertNotEqual(recovered['event_cursor']['host_instance_id'], restored['event_cursor']['host_instance_id'])
                    restored['event_cursor'] = recovered['event_cursor']
                    self.assertEqual(recovered, restored)
                    retry = call(connection, requests[success])
                    self.assertEqual(retry['receipt'], receipt)
                    self.assertEqual(retry['snapshot']['revision'], 2)
                    self.assertIn(sid, call(connection, {'type': 'list'})['session_ids'])
            finally:
                host.close()

    def test_database_failure_never_acknowledges_or_changes_project(self):
        import sqlite3
        with tempfile.TemporaryDirectory(prefix='vvm-db-fail-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            try:
                host.ready()
                with connect(root) as connection:
                    exchange(connection, hello())
                    oversized = call(connection, {'type': 'create', 'title': 'x' * (16 * 1024 * 1024 - 600)})
                    self.assertEqual(oversized['error']['code'], 'VALIDATION_FAILED')
                    self.assertEqual(call(connection, {'type': 'list'})['session_ids'], [])
                    before = call(connection, {'type': 'create', 'title': 'DB障害の検証'})['snapshot']
                    request = edit_request(before['session_id'], 0, [{'type': 'rename_project', 'title': '保存後'}])
                    with sqlite3.connect(root / 'data/recovery.sqlite') as db:
                        db.execute("CREATE TRIGGER fault AFTER UPDATE ON sessions BEGIN SELECT RAISE(ABORT, 'injected'); END")
                    self.assertEqual(call(connection, request)['error']['code'], 'IO_ERROR')
                    self.assertEqual(call(connection, {'type': 'get', 'session_id': before['session_id']})['snapshot'], before)
                    with sqlite3.connect(root / 'data/recovery.sqlite') as db:
                        db.execute('DROP TRIGGER fault')
                    self.assertEqual(call(connection, request)['receipt']['revision'], 1)
            finally:
                host.close()
