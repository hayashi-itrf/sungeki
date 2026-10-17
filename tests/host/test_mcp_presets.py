import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import ROOT, HostProcess, McpClient
from test_mcp_editing import call


class PresetTest(unittest.TestCase):
    def test_changed_original_rejected_before_any_project_edit(self):
        with tempfile.TemporaryDirectory(prefix='vvm-preset-original-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)

                def ok(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                initial = ok('create_project', title='プリセット原本変更の拒否検証')['snapshot']
                image = root / 'standing.png'
                original = (ROOT / 'tests/fixtures/build-source.png').read_bytes()
                image.write_bytes(original)
                probe = ok('probe_image', path=str(image))['probe']
                recipe = json.loads((ROOT / 'tests/fixtures/recipes/build-dialogue.json').read_text())
                asset_id = str(uuid.uuid4())
                asset = dict(id=asset_id, locator=dict(type='absolute', path=str(image)),
                             name='fixture', source=None, probe=dict(type='image', info=probe),
                             credit='test fixture', content_hash=probe['sha256'])
                preset = dict(schema_version=1, name='原本検査', description='', tags=[],
                              content=dict(kind='character', character=recipe['roles'][0]['character'], assets=[asset], standing=None))
                key = str(uuid.uuid4())
                ok('save_preset', id=key, expected_revision=0, preset=preset)
                args = dict(session_id=initial['session_id'], expected_revision=0,
                            operation_id=str(uuid.uuid4()), preset_id=key, preset_revision=1,
                            ids=dict(character_id=str(uuid.uuid4()), asset_ids={asset_id: str(uuid.uuid4())}), placement=None)
                image.write_bytes(original + b'changed')
                error = call(client, 'instantiate_preset', **args)
                self.assertEqual(error['structuredContent']['code'], 'CONFLICT')
                unchanged = ok('get_project')['snapshot']
                self.assertEqual(unchanged['project'], initial['project'])
                self.assertEqual(unchanged['revision'], 0)
                image.write_bytes(original)
                applied = ok('instantiate_preset', **args)['snapshot']
                self.assertEqual(applied['project']['assets'][0]['id'], args['ids']['asset_ids'][asset_id])
                self.assertEqual(image.read_bytes(), original)
            finally:
                if client:
                    client.close()
                host.close()

    def test_instantiation_replay_after_library_delete_restart_and_undo(self):
        with tempfile.TemporaryDirectory(prefix='vvm-instantiate-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            clients = []
            try:
                host.ready()
                client = McpClient(root)
                clients.append(client)

                def ok(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                initial = ok('create_project', title='人物プリセット適用と再送の検証')['snapshot']
                recipe = json.loads((ROOT / 'tests/fixtures/recipes/build-dialogue.json').read_text())
                preset = dict(schema_version=1, name='字幕付き人物', description='', tags=[],
                              content=dict(kind='character', character=recipe['roles'][0]['character'], assets=[], standing=None))
                key = str(uuid.uuid4())
                ok('save_preset', id=key, expected_revision=0, preset=preset)
                args = dict(session_id=initial['session_id'], expected_revision=0,
                            operation_id=str(uuid.uuid4()), preset_id=key, preset_revision=1,
                            ids=dict(character_id=str(uuid.uuid4()), asset_ids={}), placement=None)
                applied = ok('instantiate_preset', **args)
                self.assertEqual(len(applied['snapshot']['project']['characters']), 1)
                self.assertEqual(applied['snapshot']['project']['characters'][0]['id'], args['ids']['character_id'])
                ok('delete_preset', id=key, expected_revision=1)
                self.assertEqual(ok('instantiate_preset', **args)['receipt'], applied['receipt'])
                changed = dict(args, ids=dict(character_id=str(uuid.uuid4()), asset_ids={}))
                result = call(client, 'instantiate_preset', **changed)
                self.assertEqual(result['structuredContent']['code'], 'CONFLICT')
                client.close()
                clients.clear()
                host.close()
                host = HostProcess(root)
                host.ready()
                client = McpClient(root)
                clients.append(client)
                ok('attach_session', session_id=initial['session_id'])
                self.assertEqual(ok('instantiate_preset', **args)['receipt'], applied['receipt'])
                undone = ok('undo', operation_id=str(uuid.uuid4()), expected_revision=1,
                            expected_operation_id=args['operation_id'])['snapshot']
                self.assertEqual(undone['project'], initial['project'])
                replay = ok('instantiate_preset', **args)
                self.assertEqual(replay['receipt'], applied['receipt'])
                self.assertEqual(replay['snapshot']['project'], initial['project'])
            finally:
                for client in clients:
                    client.close()
                host.close()

    def test_library_survives_restart_without_changing_project(self):
        with tempfile.TemporaryDirectory(prefix='vvm-presets-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            clients = []
            try:
                host.ready()
                a, b = McpClient(root), McpClient(root)
                clients.extend([a, b])

                def ok(client, tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                tools = {t['name'] for t in a.call('tools/list')['result']['tools']}
                self.assertTrue({'list_presets', 'get_preset', 'save_preset', 'delete_preset'} <= tools)
                initial = ok(a, 'create_project', title='プリセット独立保存の検証')['snapshot']
                saved = ok(a, 'save_project', path=str(root / 'プリセット独立保存の検証.vvm.json'))['snapshot']
                recipe = json.loads((ROOT / 'tests/fixtures/recipes/build-dialogue.json').read_text())
                preset = dict(schema_version=1, name='会話の声', description='', tags=[],
                              content=dict(kind='voice', voice=recipe['roles'][0]['character']['voice'], credit='VOICEVOX:ずんだもん'))
                key = str(uuid.uuid4())
                first = ok(a, 'save_preset', id=key, expected_revision=0, preset=preset)['record']
                self.assertEqual(ok(b, 'get_preset', id=key)['record'], first)
                self.assertEqual(ok(a, 'save_preset', id=key, expected_revision=0, preset=preset)['record'], first)
                preset['name'] = '更新後の声'
                second = ok(b, 'save_preset', id=key, expected_revision=1, preset=preset)['record']
                conflict = call(a, 'delete_preset', id=key, expected_revision=1)
                self.assertEqual(conflict['structuredContent']['code'], 'CONFLICT')
                snapshot = ok(a, 'get_project')['snapshot']
                for field in ['project', 'revision', 'dirty', 'undo_operation_id', 'redo_operation_id']:
                    self.assertEqual(snapshot[field], saved[field])
                for client in clients:
                    client.close()
                clients.clear()
                host.close()
                host = HostProcess(root)
                host.ready()
                a = McpClient(root)
                clients.append(a)
                self.assertEqual(ok(a, 'get_preset', id=key)['record'], second)
                deleted = ok(a, 'delete_preset', id=key, expected_revision=2)['record']
                self.assertIsNone(deleted['preset'])
                self.assertEqual(ok(a, 'list_presets', after_id=None, limit=20)['items'], [])
                stale = call(a, 'save_preset', id=key, expected_revision=0, preset=preset)
                self.assertEqual(stale['structuredContent']['code'], 'CONFLICT')
            finally:
                for client in clients:
                    client.close()
                host.close()


if __name__ == '__main__':
    unittest.main()
