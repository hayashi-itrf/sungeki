"""Real host/MCP contract tests; fixture metadata does not prove media decoding."""
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class AssetRelinkTest(unittest.TestCase):
    def test_media_relocation_is_undoable_and_persisted(self):
        fixtures = Path(__file__).parents[1] / 'fixtures'
        with tempfile.TemporaryDirectory(prefix='vvm-relink-', dir='/tmp') as directory:
            # Resolve macOS /tmp -> /private/tmp so both OSes exercise portable saving.
            root = Path(directory).resolve()
            host, client = HostProcess(root), None
            try:
                host.ready()
                client = McpClient(root)

                def invoke(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                def document_form(project):
                    stored = json.loads(json.dumps(project))
                    # Every fixture asset is placed directly inside this document folder.
                    for asset in stored['assets']:
                        asset['locator'] = {'type': 'project_relative',
                            'path': Path(asset['locator']['path']).relative_to(root).as_posix()}
                    return stored

                for filename in ['audio-project.json', 'video-clip-project.json',
                                 'psd-speech-project.json', 'sprite-project.json', 'vrm-clip-project.json']:
                    with self.subTest(fixture=filename):
                        project = json.loads((fixtures / filename).read_text())
                        project['id'] = str(uuid.uuid4())
                        project['title'] = f'素材再指定の保存検証・{filename}'
                        for asset in project['assets']:
                            asset['locator'] = {'type': 'absolute', 'path': str(root / asset['id'])}
                        path = root / filename.replace('.json', '.vvm.json')
                        path.write_text(json.dumps(project))
                        snapshot = invoke('open_project', path=str(path))['snapshot']
                        project = snapshot['project']
                        for asset in project['assets']:
                            # A replacement must not falsify synthesis provenance or mouth timing.
                            synthesized = any(clip['content']['type'] == 'speech'
                                and clip['content'].get('audio', {}).get('asset_id') == asset['id']
                                for clip in project['clips'])
                            if synthesized:
                                before_rejection = invoke('get_project')['snapshot']
                                altered = json.loads(json.dumps(asset['probe']))
                                altered['info']['sha256'] = 'f' * 64
                                self.assertNotEqual(altered, asset['probe'])
                                rejected = call(client, 'apply_commands',
                                    session_id=snapshot['session_id'],
                                    expected_revision=snapshot['revision'],
                                    operation_id=str(uuid.uuid4()), commands=[
                                        {'type': 'set_asset_metadata', 'asset_id': asset['id'],
                                         'name': 'Must roll back', 'source': None, 'credit': ''},
                                        {'type': 'relink_asset', 'asset_id': asset['id'],
                                         'locator': {'type': 'absolute', 'path': str(root / 'different.wav')},
                                         'probe': altered}])
                                self.assertTrue(rejected.get('isError'), rejected)
                                self.assertEqual(rejected['structuredContent']['code'], 'VALIDATION_FAILED')
                                self.assertEqual(invoke('get_project')['snapshot'], before_rejection)
                            operation = str(uuid.uuid4())
                            locator = {'type': 'absolute', 'path': str(root / ('relocated-' + asset['id']))}
                            changed = invoke('apply_commands', session_id=snapshot['session_id'],
                                expected_revision=snapshot['revision'], operation_id=operation, commands=[{
                                    'type': 'relink_asset', 'asset_id': asset['id'], 'locator': locator,
                                    'probe': asset['probe']}])['snapshot']
                            expected = json.loads(json.dumps(project))
                            next(a for a in expected['assets'] if a['id'] == asset['id'])['locator'] = locator
                            self.assertEqual(changed['project'], expected)
                            invoke('save_project', session_id=snapshot['session_id'])
                            self.assertEqual(json.loads(path.read_text()), document_form(expected))
                            self.assertEqual(invoke('get_project')['snapshot']['project'], expected)
                            snapshot = invoke('undo', session_id=snapshot['session_id'],
                                operation_id=str(uuid.uuid4()), expected_revision=changed['revision'],
                                expected_operation_id=operation)['snapshot']
                            self.assertEqual(snapshot['project'], project)
                        invoke('save_project', session_id=snapshot['session_id'])
                        self.assertEqual(json.loads(path.read_text()), document_form(project))
                        reopened = invoke('open_project', path=str(path))['snapshot']
                        self.assertEqual(reopened['project'], project)
            finally:
                if client:
                    client.close()
                host.close()
