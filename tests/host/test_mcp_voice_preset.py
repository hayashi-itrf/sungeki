import copy
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import ROOT, HostProcess, McpClient
from test_mcp_editing import call


class VoicePresetTest(unittest.TestCase):
    def test_voice_snapshot_apply_delete_restart_replay_and_undo(self):
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

                initial = ok('create_project', title='声プリセット適用と復旧の確認')['snapshot']
                project = json.loads((ROOT / 'tests/fixtures/psd-speech-project.json').read_text())
                project['id'] = initial['project']['id']
                project['title'] = initial['project']['title']
                before = ok('apply_commands', operation_id=str(uuid.uuid4()), expected_revision=0,
                            commands=[dict(type='replace_project', checkpoint_id=None, project=project)])['snapshot']
                ok('save_project', path=str(root / '声プリセット適用と復旧の確認.vvm.json'))
                character = before['project']['characters'][0]
                voice = copy.deepcopy(character['voice'])
                voice['settings']['speed_scale'] = 1.5
                preset = dict(schema_version=1, name='声の調整', description='', tags=[],
                              content=dict(kind='voice', voice=voice, credit='VOICEVOX:検証'))
                key = str(uuid.uuid4())
                record = ok('save_preset', id=key, expected_revision=0, preset=preset)['record']
                command = dict(type='apply_voice_preset', preset_id=key, preset_revision=record['revision'],
                               preset=record['preset'], character_id=character['id'], credit=character['credit'] + '\nVOICEVOX:検証')
                args = dict(operation_id=str(uuid.uuid4()), expected_revision=1, commands=[command])
                applied = ok('apply_commands', **args)
                expected = copy.deepcopy(before['project'])
                expected['characters'][0]['voice'] = voice
                expected['characters'][0]['credit'] = command['credit']
                for clip in expected['clips']:
                    if clip['content']['type'] == 'speech':
                        clip['content']['audio'] = None
                self.assertEqual(applied['snapshot']['project'], expected)
                ok('delete_preset', id=key, expected_revision=1)
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
