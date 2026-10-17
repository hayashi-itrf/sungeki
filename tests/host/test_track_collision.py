"""Real host/MCP rejection, preview parity, retry and persistence."""
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class TrackCollisionTest(unittest.TestCase):
    def test_overlap_is_atomic_for_preview_and_commit_and_touching_persists(self):
        with tempfile.TemporaryDirectory(prefix='vvm-collision-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                def invoke(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']
                def edit(commands, **args):
                    return invoke('apply_commands', commands=commands,
                                  operation_id=str(uuid.uuid4()), **args)
                invoke('create_project', title='数値入力・AI編集のトラック重なり検証')
                track, other, a, b = [str(uuid.uuid4()) for _ in range(4)]
                tracks = [{'id': i, 'name': name, 'kind': 'visual', 'visible': True, 'muted': False}
                          for i, name in [(track, '図形'), (other, '別の図形')]]
                clips = [{'id': i, 'track_id': track, 'scene_id': None,
                          'start_us': start, 'duration_us': 1000000,
                          'transform': {'x': 0, 'y': 0, 'width': 100, 'height': 100, 'opacity': 1},
                          'content': {'type': 'shape', 'shape': 'rectangle', 'fill': '#123456'}}
                         for i, start in [(a, 0), (b, 2000000)]]
                original = edit([*[{'type': 'add_track', 'track': t} for t in tracks],
                                 *[{'type': 'add_clip', 'clip': c} for c in clips]])['snapshot']
                bad = [{'type': 'move_clip', 'clip_id': a, 'start_us': 1500000},
                       {'type': 'resize_clip', 'clip_id': a, 'duration_us': 2000001}]
                for command in bad:
                    for dry_run in [True, False]:
                        operation = str(uuid.uuid4())
                        args = dict(operation_id=operation, dry_run=dry_run, commands=[
                            {'type': 'rename_project', 'title': '失敗時に残さない'}, command])
                        for _ in range(2):
                            result = call(client, 'apply_commands', **args)
                            self.assertTrue(result.get('isError'), result)
                            self.assertIn('重な', result['structuredContent']['message'])
                            details = result['structuredContent']['details']
                            self.assertEqual(details['reason'], 'track_overlap')
                            self.assertEqual(details['track_id'], track)
                            self.assertEqual(details['track_name'], '図形')
                            self.assertEqual({details['first_clip_id'], details['second_clip_id']}, {a, b})
                            self.assertEqual(invoke('get_project')['snapshot'], original)
                commands = [{'type': 'move_clip', 'clip_id': a, 'start_us': 1000000}]
                edit(commands, dry_run=True)
                self.assertEqual(invoke('get_project')['snapshot'], original)
                changed = edit(commands)['snapshot']
                self.assertEqual(changed['project']['clips'][0]['start_us'], 1000000)
                undone = invoke('undo', operation_id=str(uuid.uuid4()),
                                expected_operation_id=changed['undo_operation_id'])['snapshot']
                self.assertEqual(undone['project'], original['project'])
                redone = invoke('redo', operation_id=str(uuid.uuid4()),
                                expected_operation_id=undone['redo_operation_id'])['snapshot']
                self.assertEqual(redone['project'], changed['project'])
                path = root / '数値入力・AI編集の重なり検証.vvm.json'
                invoke('save_project', path=str(path))
                self.assertEqual(json.loads(path.read_text()), changed['project'])
                self.assertEqual(invoke('open_project', path=str(path))['snapshot']['project'], changed['project'])
            finally:
                if client:
                    client.close()
                host.close()
