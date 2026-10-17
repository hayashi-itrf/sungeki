import copy
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class CanvasTest(unittest.TestCase):
    def test_canvas_atomic_undo_save_restart(self):
        with tempfile.TemporaryDirectory(prefix='vvm-canvas-', dir='/tmp') as directory:
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

                invoke('create_project', title='画面設定の編集と内容保持の検証')
                track, shape_track, text, shape = [str(uuid.uuid4()) for _ in range(4)]
                base = {'track_id': track, 'scene_id': None, 'start_us': 0, 'duration_us': 1000000,
                        'transform': {'x': 0, 'y': 0, 'width': 100, 'height': 100, 'opacity': 1}}
                original = edit([
                    {'type': 'add_track', 'track': {'id': track, 'name': '映像', 'kind': 'visual', 'visible': True, 'muted': False}},
                    {'type': 'add_track', 'track': {'id': shape_track, 'name': '図形', 'kind': 'visual', 'visible': True, 'muted': False}},
                    {'type': 'add_clip', 'clip': {**base, 'id': text, 'content': {'type': 'text', 'text': '本文を保持', 'font_size': 32, 'color': '#123456'}}},
                    {'type': 'add_clip', 'clip': {**base, 'track_id': shape_track, 'id': shape, 'content': {'type': 'shape', 'shape': 'rectangle', 'fill': '#ffffff'}}},
                ])['snapshot']
                canvas = {'width': 1080, 'height': 1920, 'fps': {'numerator': 30000, 'denominator': 1001}, 'background': '#f0e6d2'}
                commands = [{'type': 'set_canvas', 'canvas': canvas}]
                edit(commands, dry_run=True)
                self.assertEqual(invoke('get_project')['snapshot'], original)
                changed = edit(commands)['snapshot']
                expected = copy.deepcopy(original['project'])
                expected['canvas'] = canvas
                self.assertEqual(changed['project'], expected)
                invalids = [{'type': 'set_canvas', 'canvas': {**canvas, **values}} for values in (
                    {'width': 0}, {'height': 0}, {'background': 'red'},
                    {'fps': {'numerator': 0, 'denominator': 1001}},
                    {'fps': {'numerator': 30000, 'denominator': 0}},
                )]
                for invalid in invalids:
                    result = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[
                        {'type': 'set_text', 'clip_id': text, 'text': '失敗時は残さない'}, invalid])
                    self.assertTrue(result.get('isError'), result)
                    self.assertEqual(invoke('get_project')['snapshot'], changed)
                undone = invoke('undo', operation_id=str(uuid.uuid4()), expected_operation_id=changed['undo_operation_id'])['snapshot']
                self.assertEqual(undone['project'], original['project'])
                redone = invoke('redo', operation_id=str(uuid.uuid4()), expected_operation_id=undone['redo_operation_id'])['snapshot']
                self.assertEqual(redone['project'], expected)
                path = root / '画面設定の検証.vvm.json'
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
