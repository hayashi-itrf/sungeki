import concurrent.futures
import json
import os
import sqlite3
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient, connect, exchange, hello
from test_sessions import call, edit_request
from test_mcp_editing import call as tool


class OpenTest(unittest.TestCase):
    def test_mcp_open_preserves_edits_history_and_restart(self):
        with tempfile.TemporaryDirectory(prefix='vvm-open-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            clients = []
            try:
                host.ready()
                a, b = McpClient(root), McpClient(root)
                clients.extend([a, b])
                created = tool(a, 'create_project', title='再編集の確認')['structuredContent']['snapshot']
                path = root / '作品.vvm.json'
                tool(a, 'save_project', path=str(path))
                operation = str(uuid.uuid4())
                edited = tool(a, 'apply_commands', operation_id=operation, commands=[{'type': 'rename_project', 'title': '未保存の変更'}])['structuredContent']['snapshot']
                opened = tool(b, 'open_project', path=str(path))['structuredContent']['snapshot']
                self.assertEqual(opened, edited)
                self.assertEqual(tool(b, 'get_project')['structuredContent']['snapshot'], edited)
                alias = root / '別名.vvm.json'
                os.link(path, alias)
                self.assertEqual(tool(b, 'open_project', path=str(alias))['structuredContent']['snapshot'], edited)
                for client in clients:
                    client.close()
                clients.clear()
                host.process.kill()
                host.process.wait(timeout=5)
                host.close()
                host = HostProcess(root)
                host.ready()
                b = McpClient(root)
                clients.append(b)
                recovered = tool(b, 'open_project', path=str(path))['structuredContent']['snapshot']
                self.assertNotEqual(recovered['event_cursor']['host_instance_id'], edited['event_cursor']['host_instance_id'])
                edited['event_cursor'] = recovered['event_cursor']
                self.assertEqual(recovered, edited)
                undone = tool(b, 'undo', operation_id=str(uuid.uuid4()), expected_operation_id=operation)['structuredContent']['snapshot']
                self.assertFalse(undone['dirty'])
                self.assertEqual(undone['project'], created['project'])
                data = json.loads(path.read_text())
                data['title'] = '外部の編集'
                path.write_text(json.dumps(data))
                failure = tool(b, 'open_project', path=str(path))
                self.assertTrue(failure['isError'])
                self.assertEqual(failure['structuredContent']['code'], 'EXTERNAL_MODIFICATION')
                self.assertEqual(tool(b, 'get_project')['structuredContent']['snapshot'], undone)
            finally:
                for client in clients:
                    client.close()
                host.close()

    def test_new_document_atomic_registration_and_concurrent_open(self):
        with tempfile.TemporaryDirectory(prefix='vvm-open-new-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            try:
                host.ready()
                with connect(root) as c:
                    exchange(c, hello())
                    source = call(c, {'type': 'create', 'title': '読込元'})['snapshot']
                    path = root / 'loaded.vvm.json'
                    path.write_text(json.dumps(source['project']))
                    database = root / 'data/recovery.sqlite'
                    with sqlite3.connect(database) as db:
                        db.execute("CREATE TRIGGER open_fault BEFORE INSERT ON documents BEGIN SELECT RAISE(ABORT,'injected'); END")
                    failure = call(c, {'type': 'open', 'path': str(path)})
                    self.assertEqual(failure['error']['code'], 'IO_ERROR')
                    self.assertEqual(call(c, {'type': 'list'})['session_ids'], [source['session_id']])
                    with sqlite3.connect(database) as db:
                        db.execute('DROP TRIGGER open_fault')

                def open_once(_):
                    with connect(root) as c:
                        exchange(c, hello())
                        return call(c, {'type': 'open', 'path': str(path)})['snapshot']

                with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                    results = list(pool.map(open_once, range(2)))
                self.assertEqual(results[0], results[1])
                self.assertNotEqual(results[0]['session_id'], source['session_id'])
                self.assertEqual(results[0]['project'], source['project'])
                self.assertFalse(results[0]['dirty'])
                with connect(root) as c:
                    exchange(c, hello())
                    before = call(c, {'type': 'list'})
                    path.write_text('{invalid')
                    self.assertEqual(call(c, {'type': 'open', 'path': str(path)})['error']['code'], 'VALIDATION_FAILED')
                    self.assertEqual(call(c, {'type': 'list'}), before)
                    path.write_text(json.dumps(dict(source['project'], title='x' * 66000)))
                    self.assertEqual(call(c, {'type': 'open', 'path': str(path)})['error']['code'], 'EXTERNAL_MODIFICATION')
                    large = root / 'large.vvm.json'
                    large.write_text(json.dumps(dict(source['project'], title='x' * (16 * 1024 * 1024))))
                    self.assertEqual(call(c, {'type': 'open', 'path': str(large)})['error']['code'], 'VALIDATION_FAILED')
                    self.assertEqual(call(c, {'type': 'list'}), before)
            finally:
                host.close()
