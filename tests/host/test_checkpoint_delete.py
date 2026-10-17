import sqlite3
import tempfile
import unittest
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call
from test_checkpoints import uid


class CheckpointDeleteTest(unittest.TestCase):
    def test_delete_retry_restart_no_resurrection_and_restore_receipt(self):
        with tempfile.TemporaryDirectory(prefix='vvm-delete-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                original = call(client, 'create_project', title='Delete checkpoint')['structuredContent']['snapshot']
                sid = original['session_id']
                cid = uid()
                create = dict(checkpoint_id=cid, name='Original', expected_revision=0)
                call(client, 'create_checkpoint', **create)
                call(client, 'apply_commands', operation_id=uid(), commands=[{'type': 'rename_project', 'title': 'Changed'}])
                restore = dict(checkpoint_id=cid, operation_id=uid(), expected_revision=1)
                restored = call(client, 'restore_checkpoint', **restore)['structuredContent']
                call(client, 'undo', operation_id=uid(), expected_operation_id=restore['operation_id'])
                before = call(client, 'get_project')['structuredContent']['snapshot']
                # A failed tombstone insert must roll back removal of the snapshot too.
                with sqlite3.connect(root/'data/recovery.sqlite') as db:
                    db.execute("CREATE TRIGGER delete_fault BEFORE INSERT ON deleted_checkpoints BEGIN SELECT RAISE(ABORT,'injected'); END")
                failed = call(client, 'delete_checkpoint', checkpoint_id=cid)
                self.assertEqual(failed['structuredContent']['code'], 'IO_ERROR')
                self.assertEqual(call(client, 'get_project')['structuredContent']['snapshot'], before)
                self.assertEqual(len(call(client, 'list_checkpoints')['structuredContent']['checkpoints']), 1)
                with sqlite3.connect(root/'data/recovery.sqlite') as db:
                    db.execute('DROP TRIGGER delete_fault')
                deleted = call(client, 'delete_checkpoint', checkpoint_id=cid)
                self.assertFalse(deleted.get('isError'))
                after = call(client, 'get_project')['structuredContent']['snapshot']
                self.assertEqual(after['event_cursor']['event_seq'], before['event_cursor']['event_seq'] + 1)
                self.assertEqual({k: v for k, v in after.items() if k != 'event_cursor'},
                                 {k: v for k, v in before.items() if k != 'event_cursor'})
                self.assertEqual(call(client, 'delete_checkpoint', checkpoint_id=cid), deleted)
                self.assertEqual(call(client, 'get_project')['structuredContent']['snapshot'], after)
                self.assertEqual(call(client, 'list_checkpoints')['structuredContent']['checkpoints'], [])
                # This exact original create request must never resurrect deleted content.
                self.assertEqual(call(client, 'create_checkpoint', **create)['structuredContent']['code'], 'CONFLICT')
                self.assertEqual(call(client, 'restore_checkpoint', **restore)['structuredContent']['receipt'], restored['receipt'])
                self.assertEqual(call(client, 'restore_checkpoint', **dict(restore, operation_id=uid()))['structuredContent']['code'], 'NOT_FOUND')
                self.assertEqual(call(client, 'restore_checkpoint', **dict(restore, checkpoint_id=uid()))['structuredContent']['code'], 'CONFLICT')
                client.close(); client = None
                host.process.kill(); host.process.wait(timeout=5); host.close()
                host = HostProcess(root); host.ready(); client = McpClient(root)
                call(client, 'attach_session', session_id=sid)
                self.assertEqual(call(client, 'delete_checkpoint', checkpoint_id=cid), deleted)
                self.assertEqual(call(client, 'create_checkpoint', **create)['structuredContent']['code'], 'CONFLICT')
                self.assertEqual(call(client, 'restore_checkpoint', **restore)['structuredContent']['receipt'], restored['receipt'])
                self.assertEqual(call(client, 'get_project')['structuredContent']['snapshot']['project']['title'], 'Changed')
                call(client, 'create_project', title='Foreign')
                self.assertEqual(call(client, 'delete_checkpoint', checkpoint_id=cid)['structuredContent']['code'], 'VALIDATION_FAILED')
                self.assertEqual(call(client, 'delete_checkpoint', checkpoint_id=uid())['structuredContent']['code'], 'NOT_FOUND')
            finally:
                if client:
                    client.close()
                host.close()
