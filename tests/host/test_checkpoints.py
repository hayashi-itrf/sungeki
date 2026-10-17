import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


def uid():
    return str(uuid.uuid4())


class CheckpointTest(unittest.TestCase):
    def test_mcp_retention_pagination_restore_undo_and_restart(self):
        with tempfile.TemporaryDirectory(prefix='vvm-checkpoint-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                initial = call(client, 'create_project', title='保存時点の保持テスト')['structuredContent']['snapshot']
                sid = initial['session_id']
                cid = uid()
                args = dict(checkpoint_id=cid, name='最初の状態', expected_revision=0)
                created = call(client, 'create_checkpoint', **args)['structuredContent']['checkpoint']
                after = call(client, 'get_project')['structuredContent']['snapshot']
                self.assertEqual(after['revision'], 0)
                self.assertEqual(after['dirty'], initial['dirty'])
                self.assertEqual(after['project'], initial['project'])
                self.assertIsNone(after['undo_operation_id'])
                other_id = uid()
                call(client, 'create_checkpoint', checkpoint_id=other_id, name='同じ内容の別保存時点', expected_revision=0)
                for i in range(105):
                    result = call(client, 'apply_commands', operation_id=uid(), commands=[{'type': 'rename_project', 'title': f'編集{i}'}])
                    self.assertFalse(result.get('isError'))
                # Exact create replay survives later edits; a changed request is rejected.
                self.assertEqual(call(client, 'create_checkpoint', **args)['structuredContent']['checkpoint'], created)
                self.assertTrue(call(client, 'create_checkpoint', **dict(args, name='別名'))['isError'])
                for i in range(1):
                    self.assertFalse(call(client, 'create_checkpoint', checkpoint_id=uid(), name=f'編集後{i}', expected_revision=105).get('isError'))
                page = call(client, 'list_checkpoints', limit=2)['structuredContent']
                self.assertEqual(len(page['checkpoints']), 2)
                following = call(client, 'list_checkpoints', after=page['next_after'], limit=2)['structuredContent']
                self.assertEqual(len(following['checkpoints']), 1)
                self.assertIn(cid, [c['id'] for c in page['checkpoints'] + following['checkpoints']])
                failed = call(client, 'restore_checkpoint', checkpoint_id=cid, operation_id=uid(), expected_revision=0)
                self.assertEqual(failed['structuredContent']['code'], 'CONFLICT')
                operation = uid()
                restored = call(client, 'restore_checkpoint', checkpoint_id=cid, operation_id=operation, expected_revision=105)['structuredContent']
                self.assertEqual(restored['snapshot']['project'], initial['project'])
                self.assertEqual(restored['snapshot']['revision'], 106)
                undone = call(client, 'undo', operation_id=uid(), expected_operation_id=operation)['structuredContent']['snapshot']
                self.assertEqual(undone['project']['title'], '編集104')
                self.assertEqual(undone['revision'], 107)
                client.close()
                client = None
                host.process.kill()
                host.process.wait(timeout=5)
                host.close()
                host = HostProcess(root)
                host.ready()
                client = McpClient(root)
                call(client, 'attach_session', session_id=sid)
                entries = call(client, 'list_checkpoints')['structuredContent']['checkpoints']
                self.assertEqual(len(entries), 3)
                self.assertIn(created, entries)
                retry = call(client, 'restore_checkpoint', checkpoint_id=cid, operation_id=operation, expected_revision=105)['structuredContent']
                self.assertEqual(retry['receipt'], restored['receipt'])
                self.assertEqual(retry['snapshot']['revision'], 107)
                reused = call(client, 'restore_checkpoint', checkpoint_id=other_id, operation_id=operation, expected_revision=105)
                self.assertEqual(reused['structuredContent']['code'], 'CONFLICT')
                # A receipt replay is not evidence that a fresh restore works after restart.
                before_restart_restore = call(client, 'get_project')['structuredContent']['snapshot']
                fresh_operation = uid()
                fresh = call(client, 'restore_checkpoint', checkpoint_id=cid,
                             operation_id=fresh_operation, expected_revision=107)
                self.assertFalse(fresh.get('isError'), fresh)
                fresh_snapshot = fresh['structuredContent']['snapshot']
                self.assertEqual(fresh_snapshot['project'], initial['project'])
                self.assertEqual(fresh_snapshot['revision'], 108)
                self.assertEqual(fresh_snapshot['undo_operation_id'], fresh_operation)
                fresh_undo = call(client, 'undo', operation_id=uid(),
                                  expected_operation_id=fresh_operation)
                self.assertFalse(fresh_undo.get('isError'), fresh_undo)
                undo_snapshot = fresh_undo['structuredContent']['snapshot']
                self.assertEqual(undo_snapshot['project'], before_restart_restore['project'])
                self.assertEqual(undo_snapshot['revision'], 109)
                self.assertEqual(call(client, 'list_checkpoints')['structuredContent']['checkpoints'], entries)
                call(client, 'create_project', title='別作品')
                wrong = call(client, 'restore_checkpoint', checkpoint_id=cid, operation_id=uid(), expected_revision=0)
                self.assertEqual(wrong['structuredContent']['code'], 'VALIDATION_FAILED')
            finally:
                if client:
                    client.close()
                host.close()

    def test_failed_creation_keeps_project_and_event_cursor(self):
        import sqlite3
        with tempfile.TemporaryDirectory(prefix='vvm-checkpoint-fault-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                before = call(client, 'create_project', title='保存時点障害')['structuredContent']['snapshot']
                args = dict(checkpoint_id=uid(), name='失敗後に再試行', expected_revision=0)
                with sqlite3.connect(root/'data/recovery.sqlite') as db:
                    db.execute("CREATE TRIGGER checkpoint_fault BEFORE INSERT ON checkpoints BEGIN SELECT RAISE(ABORT,'injected'); END")
                result = call(client, 'create_checkpoint', **args)
                self.assertEqual(result['structuredContent']['code'], 'IO_ERROR')
                self.assertEqual(call(client, 'get_project')['structuredContent']['snapshot'], before)
                self.assertEqual(call(client, 'list_checkpoints')['structuredContent']['checkpoints'], [])
                with sqlite3.connect(root/'data/recovery.sqlite') as db:
                    db.execute('DROP TRIGGER checkpoint_fault')
                created = call(client, 'create_checkpoint', **args)
                self.assertFalse(created.get('isError'))
            finally:
                if client:
                    client.close()
                host.close()
