import copy
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class ClipDurationTest(unittest.TestCase):
    def test_resize_scene_constraints_atomicity_undo_save_and_reopen(self):
        with tempfile.TemporaryDirectory(prefix='vvm-duration-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)

                def invoke(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError', False), result)
                    return result['structuredContent']

                def edit(commands, **args):
                    return invoke('apply_commands', operation_id=str(uuid.uuid4()), commands=commands, **args)

                invoke('create_project', title='表示時間の編集とシーン制約の検証')
                tid, cid, sid = [str(uuid.uuid4()) for _ in range(3)]
                clip = {'id': cid, 'track_id': tid, 'scene_id': sid, 'start_us': 1000000,
                        'duration_us': 1000000, 'transform': {'x': 10, 'y': 20, 'width': 100, 'height': 80, 'opacity': 1},
                        'content': {'type': 'text', 'text': '表示時間', 'font_size': 32, 'color': '#123456'}}
                original = edit([
                    {'type': 'add_track', 'track': {'id': tid, 'name': '映像', 'kind': 'visual', 'visible': True, 'muted': False}},
                    {'type': 'add_scene', 'scene': {'id': sid, 'name': '導入', 'start_us': 0, 'duration_us': 3000000}},
                    {'type': 'add_clip', 'clip': clip}])['snapshot']
                resize = {'type': 'resize_clip', 'clip_id': cid, 'duration_us': 2000000}
                edit([resize], dry_run=True)
                self.assertEqual(invoke('get_project')['snapshot'], original)
                changed = edit([resize])['snapshot']
                expected = copy.deepcopy(original['project'])
                expected['clips'][0]['duration_us'] = 2000000
                self.assertEqual(changed['project'], expected)
                for invalid in (0, 2000001, 604800000001):
                    result = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[
                        {'type': 'set_text', 'clip_id': cid, 'text': '失敗バッチ'},
                        {**resize, 'duration_us': invalid}])
                    self.assertTrue(result.get('isError'), result)
                    self.assertEqual(invoke('get_project')['snapshot'], changed)
                undone = invoke('undo', operation_id=str(uuid.uuid4()), expected_operation_id=changed['undo_operation_id'])['snapshot']
                self.assertEqual(undone['project'], original['project'])
                redone = invoke('redo', operation_id=str(uuid.uuid4()), expected_operation_id=undone['redo_operation_id'])['snapshot']
                self.assertEqual(redone['project'], changed['project'])
                extended = edit([{**resize, 'duration_us': 3000000},
                                 {'type': 'resize_scene', 'scene_id': sid, 'duration_us': 4000000}])['snapshot']
                self.assertEqual(extended['project']['clips'][0]['duration_us'], 3000000)
                self.assertEqual(extended['project']['clips'][0]['scene_id'], sid)
                copy_id = str(uuid.uuid4())
                duplicate = {'type': 'duplicate_clip', 'clip_id': cid, 'new_clip_id': copy_id, 'start_us': 0}
                edit([duplicate], dry_run=True)
                self.assertEqual(invoke('get_project')['snapshot'], extended)
                copied = edit([duplicate])['snapshot']
                expected_copy = copy.deepcopy(extended['project'])
                new_track = copied['project']['clips'][1]['track_id']
                self.assertNotEqual(new_track, tid)
                uuid.UUID(new_track)
                expected_copy['tracks'].append({**expected_copy['tracks'][0],
                    'id': new_track, 'name': '映像 (複製)'})
                expected_copy['clips'].append({**expected_copy['clips'][0],
                    'id': copy_id, 'track_id': new_track, 'start_us': 0})
                self.assertEqual(copied['project'], expected_copy)
                undone_copy = invoke('undo', operation_id=str(uuid.uuid4()), expected_operation_id=copied['undo_operation_id'])['snapshot']
                self.assertEqual(undone_copy['project'], extended['project'])
                extended = invoke('redo', operation_id=str(uuid.uuid4()), expected_operation_id=undone_copy['redo_operation_id'])['snapshot']
                self.assertEqual(extended['project'], expected_copy)
                path = root / '表示時間の編集.vvm.json'
                saved = invoke('save_project', path=str(path))['snapshot']
                self.assertEqual(json.loads(path.read_text()), extended['project'])
                client.close(); client = None
                host.close(); host = HostProcess(root); host.ready()
                client = McpClient(root)
                reopened = invoke('open_project', path=str(path))['snapshot']
                self.assertEqual(reopened['project'], saved['project'])
                self.assertFalse(reopened['dirty'])
            finally:
                if client: client.close()
                host.close()
