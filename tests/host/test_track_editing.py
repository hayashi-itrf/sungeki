import copy
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class TrackEditingTest(unittest.TestCase):
    def test_properties_order_move_atomicity_undo_and_save(self):
        with tempfile.TemporaryDirectory(prefix='vvm-tracks-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready(); client = McpClient(root)
                def invoke(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError', False), result)
                    return result['structuredContent']
                def edit(commands, **args):
                    return invoke('apply_commands', operation_id=str(uuid.uuid4()), commands=commands, **args)
                invoke('create_project', title='トラック順序と所属変更の検証')
                a, b, audio, cid, sid, gid = [str(uuid.uuid4()) for _ in range(6)]
                tracks = [{'id': i, 'name': n, 'kind': kind, 'visible': True, 'muted': False}
                          for i, n, kind in [(a, '背面', 'visual'), (b, '前面', 'visual'), (audio, '音声', 'audio')]]
                clip = {'id': cid, 'track_id': a, 'scene_id': sid, 'start_us': 0, 'duration_us': 1000000,
                        'transform': {'x': 1, 'y': 2, 'width': 100, 'height': 80, 'opacity': 1},
                        'content': {'type': 'shape', 'shape': 'rectangle', 'fill': '#123456'}}
                original = edit([*[{'type': 'add_track', 'track': t} for t in tracks],
                    {'type': 'add_scene', 'scene': {'id': sid, 'name': '導入', 'start_us': 0, 'duration_us': 2000000}},
                    {'type': 'add_clip', 'clip': clip},
                    {'type': 'add_group', 'group': {'id': gid, 'name': '束', 'clip_ids': [cid]}}])['snapshot']
                properties = {'type': 'set_track_properties', 'track_id': b, 'name': '非表示の背景', 'visible': False, 'muted': True}
                commands = [properties, {'type': 'reorder_tracks', 'track_ids': [audio, b, a]},
                            {'type': 'set_clip_track', 'clip_id': cid, 'track_id': b}]
                edit(commands, dry_run=True)
                self.assertEqual(invoke('get_project')['snapshot'], original)
                changed = edit(commands)['snapshot']
                expected = copy.deepcopy(original['project'])
                expected['tracks'] = [expected['tracks'][2], expected['tracks'][1], expected['tracks'][0]]
                expected['tracks'][1].update(name=properties['name'], visible=False, muted=True)
                expected['clips'][0]['track_id'] = b
                self.assertEqual(changed['project'], expected)
                bad = [{'type': 'reorder_tracks', 'track_ids': ids} for ids in ([a, b], [a, a, audio], [a, b, str(uuid.uuid4())])]
                bad += [{'type': 'set_clip_track', 'clip_id': cid, 'track_id': i} for i in (audio, str(uuid.uuid4()))]
                bad += [{**properties, 'track_id': str(uuid.uuid4())}]
                for command in bad:
                    result = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[
                        {'type': 'rename_project', 'title': '拒否される編集'}, command])
                    self.assertTrue(result.get('isError'), result)
                    self.assertEqual(invoke('get_project')['snapshot'], changed)
                undone = invoke('undo', operation_id=str(uuid.uuid4()), expected_operation_id=changed['undo_operation_id'])['snapshot']
                self.assertEqual(undone['project'], original['project'])
                redone = invoke('redo', operation_id=str(uuid.uuid4()), expected_operation_id=undone['redo_operation_id'])['snapshot']
                self.assertEqual(redone['project'], expected)
                path = root / 'トラック編集.vvm.json'
                invoke('save_project', path=str(path))
                self.assertEqual(json.loads(path.read_text()), expected)
                client.close(); client = None
                host.close(); host = HostProcess(root); host.ready(); client = McpClient(root)
                reopened = invoke('open_project', path=str(path))['snapshot']
                self.assertEqual(reopened['project'], expected)
                self.assertFalse(reopened['dirty'])
            finally:
                if client: client.close()
                host.close()
