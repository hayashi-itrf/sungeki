import copy
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import ROOT, HostProcess, McpClient
from test_mcp_editing import call


class ResizeLayoutTest(unittest.TestCase):
    def test_resize_preview_atomic_restart_replay_and_undo(self):
        with tempfile.TemporaryDirectory(prefix='vvm-voice-preset-', dir='/tmp') as directory:
            root = Path(directory)
            host, client = HostProcess(root), None
            try:
                host.ready()
                client = McpClient(root)

                def ok(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                initial = ok('create_project', title='画面と配置の一括変更と復旧の確認')['snapshot']
                project = json.loads((ROOT / 'tests/fixtures/psd-speech-project.json').read_text())
                project['id'] = initial['project']['id']
                project['title'] = initial['project']['title']
                before = ok('apply_commands', operation_id=str(uuid.uuid4()), expected_revision=0,
                            commands=[dict(type='replace_project', checkpoint_id=None, project=project)])['snapshot']
                ok('save_project', path=str(root / '画面と配置の一括変更と復旧の確認.vvm.json'))
                command = dict(type='resize_layout', width=160, height=320)
                args = dict(operation_id=str(uuid.uuid4()), expected_revision=1, commands=[command])
                ok('apply_commands', **args, dry_run=True)
                self.assertEqual(ok('get_project')['snapshot']['project'], before['project'])
                applied = ok('apply_commands', **args)
                expected = copy.deepcopy(before['project'])
                expected['canvas'].update(width=160, height=320)
                for character in expected['characters']:
                    character['subtitle_style']['font_size'] *= 0.5
                for clip in expected['clips']:
                    if clip['content']['type'] == 'audio':
                        continue
                    t = clip['transform']
                    t['x'] *= 0.5
                    t['y'] = t['y'] * 0.5 + 115
                    t['width'] *= 0.5
                    t['height'] *= 0.5
                self.assertEqual(applied['snapshot']['project'], expected)
                result = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[
                    dict(type='rename_project', title='失敗時に残してはいけない'),
                    dict(type='resize_layout', width=0, height=320)])
                self.assertTrue(result.get('isError'), result)
                self.assertEqual(ok('get_project')['snapshot'], applied['snapshot'])
                client.close()
                client = None
                host.close()
                host = HostProcess(root)
                host.ready()
                client = McpClient(root)
                ok('attach_session', session_id=before['session_id'])
                replay = ok('apply_commands', **args)
                self.assertEqual(replay['receipt'], applied['receipt'])
                self.assertEqual(replay['snapshot']['project'], expected)
                undone = ok('undo', operation_id=str(uuid.uuid4()), expected_revision=2,
                            expected_operation_id=args['operation_id'])['snapshot']
                self.assertEqual(undone['project'], before['project'])
                self.assertFalse(undone['dirty'])
                self.assertEqual(ok('apply_commands', **args)['snapshot']['project'], before['project'])
            finally:
                if client:
                    client.close()
                host.close()
