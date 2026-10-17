import sqlite3
import tempfile
import unittest
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call
from test_checkpoints import uid


class EditPreviewTest(unittest.TestCase):
    def test_dry_run_pages_stale_apply_and_storage_unchanged(self):
        with tempfile.TemporaryDirectory(prefix='vvm-preview-', dir='/tmp') as directory:
            root = Path(directory); host = HostProcess(root); clients = []
            try:
                host.ready(); a, b = McpClient(root), McpClient(root); clients.extend([a, b])
                original = call(a, 'create_project', title='Edit preview fixture')['structuredContent']['snapshot']
                sid = original['session_id']; call(b, 'attach_session', session_id=sid)
                track = uid()
                commands = [{'type': 'rename_project', 'title': 'Reviewed'}, {'type': 'add_track', 'track': {
                    'id': track, 'kind': 'visual', 'name': 'Track', 'visible': True, 'muted': False}}]
                args = dict(dry_run=True, commands=commands, expected_revision=0, limit=1)
                with sqlite3.connect(root/'data/recovery.sqlite') as db:
                    before_db = db.execute('SELECT revision,version,snapshot FROM sessions WHERE id=?', [sid]).fetchone()
                    db.execute("CREATE TRIGGER preview_write_guard BEFORE UPDATE ON sessions BEGIN SELECT RAISE(ABORT,'preview must not write'); END")
                page = call(a, 'apply_commands', **args)['structuredContent']['preview']
                self.assertEqual(page['total_changes'], 2)
                self.assertEqual(page['source_revision'], 0)
                self.assertEqual(page['next_offset'], 1)
                self.assertFalse(page['no_op'])
                second = call(a, 'apply_commands', **args, offset=1)['structuredContent']['preview']
                self.assertIsNone(second['next_offset'])
                self.assertEqual(second['after_hash'], page['after_hash'])
                self.assertEqual(call(a, 'get_project')['structuredContent']['snapshot'], original)
                with sqlite3.connect(root/'data/recovery.sqlite') as db:
                    self.assertEqual(db.execute('SELECT revision,version,snapshot FROM sessions WHERE id=?', [sid]).fetchone(), before_db)
                    db.execute('DROP TRIGGER preview_write_guard')
                no_op = call(a, 'apply_commands', dry_run=True, commands=[])['structuredContent']['preview']
                self.assertTrue(no_op['no_op']); self.assertEqual(no_op['total_changes'], 0)
                self.assertEqual(no_op['before_hash'], no_op['after_hash'])
                invalid = call(a, 'apply_commands', dry_run=True, commands=commands + [{'type': 'delete_clip', 'clip_id': uid()}])
                self.assertEqual(invalid['structuredContent']['code'], 'VALIDATION_FAILED')
                for opts in [dict(limit=0), dict(limit=51), dict(offset=3)]:
                    bad = call(a, 'apply_commands', dry_run=True, commands=commands, **opts)
                    self.assertEqual(bad['structuredContent']['code'], 'VALIDATION_FAILED')
                changed = call(b, 'apply_commands', operation_id=uid(), commands=[{'type': 'rename_project', 'title': 'Concurrent'}])
                self.assertFalse(changed.get('isError'))
                stale_page = call(a, 'apply_commands', **args, offset=1)
                self.assertEqual(stale_page['structuredContent']['code'], 'CONFLICT')
                stale_apply = call(a, 'apply_commands', commands=commands, operation_id=uid(), expected_revision=0)
                self.assertEqual(stale_apply['structuredContent']['code'], 'CONFLICT')
                call(a, 'get_project')
                reviewed = call(a, 'apply_commands', dry_run=True, commands=commands)['structuredContent']['preview']
                applied = call(a, 'apply_commands', operation_id=uid(), commands=commands)['structuredContent']['snapshot']
                self.assertEqual(applied['project']['tracks'][0]['id'], track)
                self.assertEqual(applied['project']['title'], 'Reviewed')
                self.assertEqual(applied['revision'], reviewed['source_revision'] + 1)
                checkpoint = call(a, 'create_checkpoint', checkpoint_id=uid(), name='Applied preview', expected_revision=applied['revision'])['structuredContent']['checkpoint']
                self.assertEqual(checkpoint['content_hash'], reviewed['after_hash'])
            finally:
                for client in clients: client.close()
                host.close()
