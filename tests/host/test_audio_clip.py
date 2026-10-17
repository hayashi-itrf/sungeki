"""Actual new-MCP audio editing; synthetic metadata, no render-success claim."""
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class AudioClipTest(unittest.TestCase):
    def test_edit_undo_save_restart_and_invalid_batch(self):
        fixture = json.loads((Path(__file__).parents[1] / 'fixtures/audio-project.json').read_text())
        with tempfile.TemporaryDirectory(prefix='vvm-audio-clip-', dir='/tmp') as temp:
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

                initial = ok('create_project', title='音声素材の保存検証')['snapshot']
                fixture['id'] = initial['project']['id']
                before = ok('apply_commands', operation_id=str(uuid.uuid4()), commands=[
                    dict(type='replace_project', checkpoint_id=None, project=fixture)])['snapshot']
                clip = before['project']['clips'][0]
                settings = dict(clip['content']['playback'], gain_db=-12.5,
                                ducking=dict(attenuation_db=-15, attack_us=250000, release_us=500000))
                command = dict(type='set_audio', clip_id=clip['id'],
                               asset_id=clip['content']['asset_id'], playback=settings)
                op = str(uuid.uuid4())
                edited = ok('apply_commands', operation_id=op, commands=[command])['snapshot']
                self.assertEqual(edited['project']['clips'][0]['content']['playback'], settings)
                undone = ok('undo', operation_id=str(uuid.uuid4()), expected_operation_id=op)['snapshot']
                self.assertEqual(undone['project'], before['project'])
                redone = ok('redo', operation_id=str(uuid.uuid4()), expected_operation_id=op)['snapshot']
                self.assertEqual(redone['project'], edited['project'])
                for bad in [dict(settings, looped=False), dict(settings, source_start_us=1000000),
                            dict(settings, fade_out_us=2000000), dict(settings, gain_db=25),
                            dict(settings, ducking=dict(settings['ducking'], attenuation_db=1)),
                            dict(settings, ducking=dict(settings['ducking'], attack_us=604800000001))]:
                    response = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[
                        dict(type='rename_project', title='must not change'), dict(command, playback=bad)])
                    self.assertTrue(response['isError'])
                    self.assertEqual(ok('get_project')['snapshot']['project'], edited['project'])
                saved = root / 'audio-clip-trim-loop-gain.vvm.json'
                ok('save_project', path=str(saved), expected_revision=redone['revision'])
                self.assertEqual(json.loads(saved.read_text()), edited['project'])
                client.close(); client = None; host.close()
                host = HostProcess(root); host.ready(); client = McpClient(root)
                reopened = ok('open_project', path=str(saved))['snapshot']
                self.assertEqual(reopened['project'], edited['project'])
            finally:
                if client:
                    client.close()
                host.close()
