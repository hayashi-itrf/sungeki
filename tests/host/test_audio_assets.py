"""Metadata persistence via real MCP/host. Does not claim synthesis or playback."""
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


def uid():
    return str(uuid.uuid4())


class AudioAssetsTest(unittest.TestCase):
    def test_audio_metadata_save_checkpoint_undo_and_restart(self):
        with tempfile.TemporaryDirectory(prefix='vvm-audio-assets-', dir='/tmp') as directory:
            # Resolve macOS /tmp -> /private/tmp so both OSes exercise portable saving.
            root = Path(directory).resolve()
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)

                def invoke(tool, **args):
                    response = call(client, tool, **args)
                    self.assertFalse(response.get('isError'), response)
                    return response['structuredContent']

                initial = invoke('create_project', title='音声素材メタデータの保存・復旧検証')['snapshot']
                asset = {'id': uid(), 'name': '未接続の音声原本', 'source': None, 'credit': '検証用',
                         'locator': {'type': 'absolute', 'path': str(root / 'missing.wav')},
                         'content_hash': 'a' * 64, 'probe': {'type': 'audio', 'info': {
                             'sha256': 'a' * 64, 'duration_us': 10000, 'sample_rate': 24000,
                             'channels': 1, 'frames': 240, 'byte_length': 524}}}
                registered = invoke('apply_commands', operation_id=uid(),
                    commands=[{'type': 'register_asset', 'asset': asset}])['snapshot']
                checkpoint = uid()
                invoke('create_checkpoint', checkpoint_id=checkpoint, name='音声素材を保持', expected_revision=1)
                saved_path = root / '音声素材保存検証.vvm.json'
                invoke('save_project', path=str(saved_path))
                stored_asset = json.loads(json.dumps(asset))
                stored_asset['locator'] = {'type': 'project_relative', 'path': 'missing.wav'}
                self.assertEqual(json.loads(saved_path.read_text())['assets'], [stored_asset])
                self.assertEqual(invoke('get_project')['snapshot']['project'], registered['project'])
                reopened = invoke('open_project', path=str(saved_path))['snapshot']
                self.assertEqual(reopened['project'], registered['project'])
                removed = invoke('apply_commands', operation_id=uid(),
                    commands=[{'type': 'remove_asset', 'asset_id': asset['id']}])['snapshot']
                restored = invoke('restore_checkpoint', checkpoint_id=checkpoint,
                    operation_id=uid(), expected_revision=removed['revision'])['snapshot']
                self.assertEqual(restored['project'], registered['project'])
                undone = invoke('undo', operation_id=uid(),
                    expected_operation_id=restored['undo_operation_id'])['snapshot']
                self.assertEqual(undone['project']['assets'], [])
                invoke('redo', operation_id=uid(), expected_operation_id=undone['redo_operation_id'])
                client.close(); client = None
                host.close()
                host = HostProcess(root); host.ready()
                client = McpClient(root)
                recovered = invoke('attach_session', session_id=initial['session_id'])['snapshot']
                self.assertEqual(recovered['project'], registered['project'])
                entries = invoke('list_checkpoints')['checkpoints']
                self.assertIn(checkpoint, [entry['id'] for entry in entries])
            finally:
                if client:
                    client.close()
                host.close()
