"""Master settings use common editing, never rewrite per-clip gain or originals."""
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class AudioMixTest(unittest.TestCase):
    def test_preview_edit_undo_persistence_clear_and_atomic_rejection(self):
        with tempfile.TemporaryDirectory(prefix='vvm-master-', dir='/tmp') as temp:
            root = Path(temp)
            host = HostProcess(root)
            client = None
            try:
                self.assertGreaterEqual(host.ready()['protocol_minor'], 44)
                client = McpClient(root)

                def ok(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                initial = ok('create_project', title='Master settings fixture')['snapshot']
                fixture = json.loads((Path(__file__).parents[1]/'fixtures/audio-project.json').read_text())
                fixture['id'] = initial['project']['id']
                before = ok('apply_commands', operation_id=str(uuid.uuid4()), commands=[
                    dict(type='replace_project', checkpoint_id=None, project=fixture)])['snapshot']
                command = dict(type='set_audio_mix', audio_mix=dict(normalization=dict(target_lufs=-18, true_peak_db=-1)))
                preview = ok('apply_commands', dry_run=True, commands=[command])['preview']
                self.assertFalse(preview['no_op'])
                self.assertEqual(preview['total_changes'], 1)
                self.assertEqual(ok('get_project')['snapshot'], before)
                op = str(uuid.uuid4())
                edited = ok('apply_commands', operation_id=op, commands=[command])['snapshot']
                expected = json.loads(json.dumps(before['project']))
                expected['audio_mix'] = command['audio_mix']
                self.assertEqual(edited['project'], expected)
                self.assertEqual(ok('undo', operation_id=str(uuid.uuid4()), expected_operation_id=op)['snapshot']['project'], before['project'])
                redone = ok('redo', operation_id=str(uuid.uuid4()), expected_operation_id=op)['snapshot']
                self.assertEqual(redone['project'], expected)
                for lufs, peak in [(-71, -1), (-18, 1), (-18, -21)]:
                    rejected = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[
                        dict(type='rename_project', title='must not leak'),
                        dict(type='set_audio_mix', audio_mix=dict(normalization=dict(target_lufs=lufs, true_peak_db=peak)))])
                    self.assertTrue(rejected['isError'])
                    self.assertEqual(ok('get_project')['snapshot'], redone)
                saved = root/'master-settings.vvm.json'
                ok('save_project', path=str(saved), expected_revision=redone['revision'])
                self.assertEqual(json.loads(saved.read_text()), expected)
                client.close(); client = None; host.close()
                host = HostProcess(root); host.ready(); client = McpClient(root)
                reopened = ok('open_project', path=str(saved))['snapshot']
                self.assertEqual(reopened['project'], expected)
                cleared = ok('apply_commands', operation_id=str(uuid.uuid4()), commands=[
                    dict(type='set_audio_mix', audio_mix=dict(normalization=None))])['snapshot']
                self.assertEqual(cleared['project'], before['project'])
            finally:
                if client:
                    client.close()
                host.close()
