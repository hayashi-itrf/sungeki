import json
import sqlite3
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, connect, exchange, hello, McpClient
from test_sessions import call, edit_request
from test_mcp_editing import call as tool


class SaveTest(unittest.TestCase):
    def test_save_dirty_undo_external_change_and_metadata_recovery(self):
        with tempfile.TemporaryDirectory(prefix='vvm-save-', dir='/tmp') as directory:
            root=Path(directory); host=HostProcess(root)
            try:
                host.ready()
                with connect(root) as c:
                    exchange(c,hello())
                    snap=call(c,{'type':'create','title':'保存テスト'})['snapshot']; sid=snap['session_id']
                    path=root/'作品.vvm.json'
                    saved=call(c,{'type':'save','session_id':sid,'path':str(path),'expected_revision':0})['snapshot']
                    self.assertFalse(saved['dirty']); self.assertEqual(saved['revision'],0)
                    self.assertEqual(json.loads(path.read_text())['title'],'保存テスト')
                    request=edit_request(sid,0,[{'type':'rename_project','title':'変更後'}])
                    after=call(c,request)['snapshot']; self.assertTrue(after['dirty'])
                    undo={'type':'edit','session_id':sid,'edit':{'operation_id':str(uuid.uuid4()),'expected_revision':1,'action':{'type':'undo','expected_operation_id':request['edit']['operation_id']}}}
                    restored=call(c,undo)['snapshot']; self.assertFalse(restored['dirty'])
                    other=call(c,{'type':'create','title':'他文書'})['snapshot']['session_id']
                    self.assertEqual(call(c,{'type':'save','session_id':other,'path':str(path),'expected_revision':0})['error']['code'],'CONFLICT')
                    path.write_text('external')
                    self.assertEqual(call(c,{'type':'save','session_id':sid,'path':None,'expected_revision':2})['error']['code'],'EXTERNAL_MODIFICATION')
                    self.assertEqual(path.read_text(),'external')
                host.close(); host=HostProcess(root); host.ready()
                with connect(root) as c:
                    exchange(c,hello()); recovered=call(c,{'type':'get','session_id':sid})['snapshot']
                    self.assertEqual(recovered['document_path'],str(path.resolve()))
            finally: host.close()

    def test_published_file_and_pending_binding_reconcile_after_restart(self):
        with tempfile.TemporaryDirectory(prefix='vvm-save-recover-',dir='/tmp') as directory:
            root=Path(directory); host=HostProcess(root)
            try:
                host.ready()
                with connect(root) as c:
                    exchange(c,hello()); sid=call(c,{'type':'create','title':'保存の中断復旧'})['snapshot']['session_id']
                    with sqlite3.connect(root/'data/recovery.sqlite') as db:
                        db.execute("CREATE TRIGGER binding_fault BEFORE INSERT ON documents BEGIN SELECT RAISE(ABORT,'injected'); END")
                    path=root/'recover.vvm.json'
                    self.assertEqual(call(c,{'type':'save','session_id':sid,'path':str(path),'expected_revision':0})['error']['code'],'IO_ERROR')
                    self.assertTrue(path.exists())
                host.process.kill(); host.process.wait(timeout=5); host.close()
                with sqlite3.connect(root/'data/recovery.sqlite') as db: db.execute('DROP TRIGGER binding_fault')
                host=HostProcess(root); host.ready()
                with connect(root) as c:
                    exchange(c,hello()); snap=call(c,{'type':'get','session_id':sid})['snapshot']
                    self.assertFalse(snap['dirty']); self.assertEqual(snap['document_path'],str(path.resolve()))
                    self.assertEqual(call(c,{'type':'save','session_id':sid,'path':None,'expected_revision':0})['type'],'snapshot')
            finally: host.close()

    def test_mcp_save(self):
        with tempfile.TemporaryDirectory(prefix='vvm-mcp-save-',dir='/tmp') as directory:
            root=Path(directory); host=HostProcess(root); client=None
            try:
                host.ready(); client=McpClient(root)
                tool(client,'create_project',title='新MCP保存確認')
                path=root/'mcp.vvm.json'
                result=tool(client,'save_project',path=str(path))
                self.assertFalse(result.get('isError',False)); self.assertFalse(result['structuredContent']['snapshot']['dirty'])
                self.assertEqual(json.loads(path.read_text())['title'],'新MCP保存確認')
            finally:
                if client: client.close()
                host.close()
