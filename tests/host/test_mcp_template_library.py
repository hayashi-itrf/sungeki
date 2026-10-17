import copy
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import ROOT, HostProcess, McpClient
from test_mcp_editing import call


class TemplateLibraryTest(unittest.TestCase):
    def test_saved_recipe_roundtrip_and_restart_preserve_all_settings_without_edit(self):
        with tempfile.TemporaryDirectory(prefix='vvm-template-', dir='/tmp') as directory:
            root = Path(directory)
            host, client = HostProcess(root), None
            try:
                host.ready()
                client = McpClient(root)
                def ok(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']
                initial = ok('create_project', title='制作テンプレート保存検証')['snapshot']
                recipe = json.loads((ROOT / 'tests/fixtures/recipes/build-dialogue.json').read_text())
                preset = dict(schema_version=1, name='掛け合いテンプレート', description='保存確認', tags=['会話'], content=dict(kind='template', recipe=recipe))
                key = str(uuid.uuid4())
                first = ok('save_preset', id=key, expected_revision=0, preset=preset)['record']
                self.assertEqual(first['preset'], preset)
                fetched = ok('get_preset', id=key)['record']
                episode = copy.deepcopy(fetched['preset']['content']['recipe'])
                episode['title'] = '次回の動画'
                episode['scenes'][0]['lines'][0]['subtitle_text'] = '新しい本文'
                self.assertEqual(ok('get_preset', id=key)['record'], first)
                snapshot = ok('get_project')['snapshot']
                for field in ['project', 'revision', 'dirty', 'undo_operation_id']:
                    self.assertEqual(snapshot[field], initial[field])
                client.close()
                client = None
                host.close()
                host = HostProcess(root)
                host.ready()
                client = McpClient(root)
                self.assertEqual(ok('get_preset', id=key)['record'], first)
                self.assertEqual(ok('list_presets', after_id=None, limit=20)['items'][0]['kind'], 'template')
            finally:
                if client:
                    client.close()
                host.close()
