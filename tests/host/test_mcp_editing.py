import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient


def call(client, tool_name, **args):
    result = client.call('tools/call', {'name': tool_name, 'arguments': args})['result']
    return result


class McpEditingTest(unittest.TestCase):
    def test_create_attach_edit_retry_conflict_and_undo(self):
        with tempfile.TemporaryDirectory(prefix='vvm-mcp-edit-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            clients = []
            try:
                host.ready()
                a, b = McpClient(root), McpClient(root)
                clients.extend([a, b])
                snap = call(a, 'create_project', title='新MCP共通編集テスト')['structuredContent']['snapshot']
                sid = snap['session_id']
                call(b, 'attach_session', session_id=sid)
                operation = str(uuid.uuid4())
                args = {'operation_id': operation, 'commands': [{'type': 'rename_project', 'title': 'AIから編集'}]}
                first = call(a, 'apply_commands', **args)['structuredContent']
                self.assertEqual(first['snapshot']['revision'], 1)
                retry = call(a, 'apply_commands', **args)['structuredContent']
                self.assertEqual(retry['receipt'], first['receipt'])
                conflict = call(b, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[{'type': 'rename_project', 'title': '古い更新'}])
                self.assertTrue(conflict['isError'])
                self.assertEqual(conflict['structuredContent']['code'], 'CONFLICT')
                self.assertIn('別の編集で作品が更新', conflict['structuredContent']['message'])
                self.assertNotIn('History(', conflict['structuredContent']['message'])
                refreshed = call(b, 'get_project')['structuredContent']['snapshot']
                self.assertEqual(refreshed['project']['title'], 'AIから編集')
                undone = call(b, 'undo', operation_id=str(uuid.uuid4()), expected_operation_id=operation)['structuredContent']['snapshot']
                self.assertEqual(undone['project']['title'], snap['project']['title'])
                self.assertEqual(undone['revision'], 2)
                self.assertEqual(call(a, 'get_project')['structuredContent']['snapshot'], undone)
                self.assertIn(sid, call(a, 'list_sessions')['structuredContent']['session_ids'])
                track = str(uuid.uuid4())
                clips = []
                for content in [{'type': 'text', 'text': '字幕', 'font_size': 32, 'color': '#123456'},
                                {'type': 'shape', 'shape': 'rectangle', 'fill': '#aabbcc'}]:
                    clips.append({'id': str(uuid.uuid4()), 'track_id': track, 'scene_id': None,
                                  'start_us': len(clips) * 1000000, 'duration_us': 1000000,
                                  'transform': {'x': 10, 'y': 20, 'width': 300, 'height': 80, 'opacity': 1},
                                  'content': content})
                commands = [{'type': 'add_track', 'track': {'id': track, 'name': '映像', 'kind': 'visual', 'visible': True, 'muted': False}}]
                commands += [{'type': 'add_clip', 'clip': clip} for clip in clips]
                added = call(a, 'apply_commands', operation_id=str(uuid.uuid4()), commands=commands)['structuredContent']['snapshot']
                self.assertEqual(added['project']['clips'], clips)
                self.assertEqual(call(b, 'get_project')['structuredContent']['snapshot'], added)

            finally:
                for client in clients:
                    client.close()
                host.close()
