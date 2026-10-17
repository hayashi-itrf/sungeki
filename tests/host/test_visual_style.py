import copy
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class VisualStyleTest(unittest.TestCase):
    def test_style_atomic_undo_save_restart(self):
        with tempfile.TemporaryDirectory(prefix='vvm-style-', dir='/tmp') as directory:
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

                invoke('create_project', title='文字色・文字サイズ・図形設定の検証')
                track, shape_track, text, shape = [str(uuid.uuid4()) for _ in range(4)]
                base = {'track_id': track, 'scene_id': None, 'start_us': 0, 'duration_us': 1000000,
                        'transform': {'x': 0, 'y': 0, 'width': 100, 'height': 100, 'opacity': 1}}
                original = edit([
                    {'type': 'add_track', 'track': {'id': track, 'name': '映像', 'kind': 'visual', 'visible': True, 'muted': False}},
                    {'type': 'add_track', 'track': {'id': shape_track, 'name': '図形', 'kind': 'visual', 'visible': True, 'muted': False}},
                    {'type': 'add_clip', 'clip': {**base, 'id': text, 'content': {'type': 'text', 'text': '本文を保持', 'font_size': 32, 'color': '#123456'}}},
                    {'type': 'add_clip', 'clip': {**base, 'track_id': shape_track, 'id': shape, 'content': {'type': 'shape', 'shape': 'rectangle', 'fill': '#ffffff'}}},
                ])['snapshot']
                commands = [
                    {'type': 'set_text_style', 'clip_id': text, 'font_size': 72.5, 'color': '#Ab12EF'},
                    {'type': 'set_shape', 'clip_id': shape, 'shape': 'ellipse', 'fill': '#2468ac'},
                ]
                edit(commands, dry_run=True)
                self.assertEqual(invoke('get_project')['snapshot'], original)
                changed = edit(commands)['snapshot']
                expected = copy.deepcopy(original['project'])
                expected['clips'][0]['content'].update(font_size=72.5, color='#Ab12EF')
                expected['clips'][1]['content'].update(shape='ellipse', fill='#2468ac')
                self.assertEqual(changed['project'], expected)
                invalids = [
                    {**commands[0], 'font_size': 0}, {**commands[0], 'font_size': -1},
                    {**commands[0], 'color': 'red'}, {**commands[1], 'fill': '#12345678'},
                    {**commands[0], 'clip_id': shape}, {**commands[1], 'clip_id': text},
                    {**commands[1], 'clip_id': str(uuid.uuid4())},
                ]
                for invalid in invalids:
                    result = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[
                        {'type': 'set_text', 'clip_id': text, 'text': '失敗時は残さない'}, invalid])
                    self.assertTrue(result.get('isError'), result)
                    self.assertEqual(invoke('get_project')['snapshot'], changed)
                undone = invoke('undo', operation_id=str(uuid.uuid4()), expected_operation_id=changed['undo_operation_id'])['snapshot']
                self.assertEqual(undone['project'], original['project'])
                redone = invoke('redo', operation_id=str(uuid.uuid4()), expected_operation_id=undone['redo_operation_id'])['snapshot']
                self.assertEqual(redone['project'], expected)
                path = root / '文字と図形の設定.vvm.json'
                invoke('save_project', path=str(path))
                self.assertEqual(json.loads(path.read_text()), expected)
                client.close(); client = None
                host.close(); host = HostProcess(root); host.ready()
                client = McpClient(root)
                reopened = invoke('open_project', path=str(path))['snapshot']
                self.assertEqual(reopened['project'], expected)
                self.assertFalse(reopened['dirty'])
            finally:
                if client: client.close()
                host.close()
