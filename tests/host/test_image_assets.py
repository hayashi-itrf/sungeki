import base64
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient, connect, exchange, hello
from test_mcp_editing import call
from test_image_probe import png


class ImageAssetsTest(unittest.TestCase):
    def test_image_register_save_as_missing_relink_undo_and_reopen(self):
        with tempfile.TemporaryDirectory(prefix='vvm-assets-', dir='/tmp') as directory:
            # Resolve macOS /tmp -> /private/tmp so both OSes exercise portable saving.
            root = Path(directory).resolve()
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                def invoke(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError', False), result)
                    return result['structuredContent']
                source = root / '透過画像.png'
                source.write_bytes(png())
                probe = invoke('probe_image', path=str(source))['probe']
                initial = invoke('create_project', title='画像素材の保存と再指定の検証')['snapshot']
                aid, tid, cid = [str(uuid.uuid4()) for _ in range(3)]
                asset = {'id': aid, 'name': '透過素材', 'source': '検証生成', 'credit': 'テスト素材',
                         'locator': {'type': 'absolute', 'path': str(source)}, 'probe': {'type': 'image', 'info': probe}, 'content_hash': probe['sha256']}
                operation = str(uuid.uuid4())
                commands = [ {'type': 'register_asset', 'asset': asset},
                    {'type': 'add_track', 'track': {'id': tid, 'name': '画像', 'kind': 'visual', 'visible': True, 'muted': False}},
                    {'type': 'add_clip', 'clip': {'id': cid, 'track_id': tid, 'scene_id': None, 'start_us': 0, 'duration_us': 1000000,
                     'transform': {'x': 0, 'y': 0, 'width': 200, 'height': 300, 'opacity': 1}, 'content': {'type': 'image', 'asset_id': aid}}}]
                # One original is shared by an image and both mouth states of a sprite.
                sprite = json.loads(json.dumps(commands[-1]['clip']))
                sprite.update(id=str(uuid.uuid4()), start_us=2000000)
                sprite['content'] = {'type': 'sprite', 'character_id': None, 'pose': None,
                    'sprites': {'default_pose': 'normal', 'poses': [
                        {'name': 'normal', 'closed_asset_id': aid, 'open_asset_id': aid}]}}
                commands.append({'type': 'add_clip', 'clip': sprite})
                registered = invoke('apply_commands', operation_id=operation, commands=commands)['snapshot']
                rejected = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[{'type': 'remove_asset', 'asset_id': aid}])
                self.assertEqual(rejected['structuredContent']['code'], 'VALIDATION_FAILED')
                self.assertEqual(invoke('get_project')['snapshot'], registered)
                with connect(root) as connection:
                    exchange(connection, hello())
                    preview = exchange(connection, {'type': 'session', 'request': {'type': 'image_preview',
                        'session_id': initial['session_id'], 'asset_id': aid, 'expected_revision': registered['revision']}})['result']
                    self.assertEqual(preview['type'], 'image_preview')
                    self.assertEqual((preview['width'], preview['height']), (2, 3))
                    self.assertEqual(preview['sha256'], probe['sha256'])
                    self.assertTrue(base64.b64decode(preview['png_base64']).startswith(b'\x89PNG'))
                    source.write_bytes(b'changed')
                    rejected_preview = exchange(connection, {'type': 'session', 'request': {'type': 'image_preview',
                        'session_id': initial['session_id'], 'asset_id': aid, 'expected_revision': registered['revision']}})
                    self.assertEqual(rejected_preview['error']['code'], 'EXTERNAL_MODIFICATION')
                    source.write_bytes(png())
                metadata = {'type': 'set_asset_metadata', 'asset_id': aid, 'name': '出典確認済み素材',
                            'source': None, 'credit': '自作テスト素材\n転載不要'}
                dry_run = invoke('apply_commands', operation_id=str(uuid.uuid4()), commands=[metadata], dry_run=True)
                self.assertEqual(invoke('get_project')['snapshot']['project'], registered['project'])
                original = registered['project']
                changed_metadata = invoke('apply_commands', operation_id=str(uuid.uuid4()), commands=[metadata])['snapshot']
                expected = json.loads(json.dumps(original))
                expected['assets'][0].update({k: metadata[k] for k in ('name', 'source', 'credit')})
                self.assertEqual(changed_metadata['project'], expected)
                undone_metadata = invoke('undo', operation_id=str(uuid.uuid4()),
                    expected_operation_id=changed_metadata['undo_operation_id'])['snapshot']
                self.assertEqual(undone_metadata['project'], original)
                registered = invoke('redo', operation_id=str(uuid.uuid4()),
                    expected_operation_id=undone_metadata['redo_operation_id'])['snapshot']
                self.assertEqual(registered['project'], expected)
                first = root / '作品.vvm.json'
                second = root / '別の場所' / '別名.vvm.json'
                second.parent.mkdir()
                for path in [first, second]:
                    saved = invoke('save_project', path=str(path))['snapshot']
                    self.assertFalse(saved['dirty'])
                    stored_project = json.loads(json.dumps(registered['project']))
                    # Only the first document contains the original within its own folder.
                    if path == first:
                        stored_project['assets'][0]['locator'] = {
                            'type': 'project_relative', 'path': '透過画像.png'}
                    self.assertEqual(json.loads(path.read_text()), stored_project)
                    self.assertEqual(saved['project'], registered['project'])
                source.unlink()
                self.assertEqual(invoke('get_project')['snapshot']['project'], registered['project'])
                replacement = root / '再指定.png'
                replacement.write_bytes(png())
                changed = invoke('apply_commands', operation_id=str(uuid.uuid4()), commands=[{'type': 'relink_asset', 'asset_id': aid,
                    'locator': {'type': 'absolute', 'path': str(replacement)}, 'probe': {'type': 'image', 'info': invoke('probe_image', path=str(replacement))['probe']}}])['snapshot']
                self.assertEqual(changed['project']['clips'], registered['project']['clips'])
                expected_relink = json.loads(json.dumps(registered['project']))
                expected_relink['assets'][0]['locator']['path'] = str(replacement)
                self.assertEqual(changed['project'], expected_relink)
                undone = invoke('undo', operation_id=str(uuid.uuid4()), expected_operation_id=changed['undo_operation_id'])['snapshot']
                self.assertEqual(undone['project'], registered['project'])
                self.assertFalse(undone['dirty'])
                restored = invoke('redo', operation_id=str(uuid.uuid4()), expected_operation_id=undone['redo_operation_id'])['snapshot']
                self.assertEqual(restored['project'], changed['project'])
                invoke('save_project')
                client.close(); client = None
                host.close()
                host = HostProcess(root); host.ready()
                client = McpClient(root)
                reopened = invoke('open_project', path=str(second))['snapshot']
                self.assertEqual(reopened['project'], changed['project'])
                self.assertFalse(reopened['dirty'])
                self.assertEqual(reopened['session_id'], initial['session_id'])
            finally:
                if client: client.close()
                host.close()
