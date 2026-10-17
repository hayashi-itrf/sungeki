"""Exercise nested preset placement through the real MCP and persistent host."""
import json
from pathlib import Path
import tempfile
import unittest
import uuid
from support import HostProcess, McpClient, ROOT
from test_mcp_editing import call


def uid():
    return str(uuid.uuid4())


class PresetBatchCollisionTest(unittest.TestCase):
    def test_final_placement_preview_retry_undo_and_disk_reload(self):
        with tempfile.TemporaryDirectory(prefix='vvm-preset-batch-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready(seconds=30)
                client = McpClient(root, response_timeout=30)

                def invoke(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                invoke('create_project', title='プリセットと後続移動の一括編集検証')
                track, occupied, standing = uid(), uid(), uid()
                transform = dict(x=0, y=0, width=100, height=100, opacity=1)
                original = invoke('apply_commands', operation_id=uid(), commands=[
                    dict(type='add_track', track=dict(id=track, name='映像', kind='visual',
                         visible=True, muted=False)),
                    dict(type='add_clip', clip=dict(id=occupied, track_id=track, scene_id=None,
                         start_us=0, duration_us=1000000, transform=transform,
                         content=dict(type='shape', shape='rectangle', fill='#123456'))),
                ])['snapshot']
                recipe = json.loads((ROOT/'tests/fixtures/recipes/build-dialogue.json').read_text())
                role = recipe['roles'][0]
                preset = dict(schema_version=1, name='検証用立ち絵', description='', tags=[],
                    content=dict(kind='character', character=role['character'],
                                 assets=recipe['assets'], standing=role['standing']))
                command = dict(type='apply_character_preset', preset_id=uid(), preset_revision=1,
                    preset=preset, ids=dict(character_id=uid(),
                        asset_ids={a['id']: uid() for a in recipe['assets']}),
                    placement=dict(new_track_name=None, clip_id=standing, track_id=track,
                        scene_id=None, start_us=0, duration_us=1000000, transform=transform))
                for dry_run in [True, False]:
                    result = call(client, 'apply_commands', operation_id=uid(),
                                  dry_run=dry_run, commands=[command])
                    self.assertTrue(result.get('isError'), result)
                    self.assertEqual(result['structuredContent']['code'], 'VALIDATION_FAILED')
                    self.assertEqual(invoke('get_project')['snapshot'], original)
                commands = [command, dict(type='move_clip', clip_id=standing, start_us=1000000)]
                invoke('apply_commands', operation_id=uid(), dry_run=True, commands=commands)
                self.assertEqual(invoke('get_project')['snapshot'], original)
                operation = uid()
                changed = invoke('apply_commands', operation_id=operation, commands=commands)
                retry = invoke('apply_commands', operation_id=operation, commands=commands)
                self.assertEqual(retry, changed)
                snapshot = changed['snapshot']
                self.assertEqual(len(snapshot['project']['clips']), 2)
                self.assertEqual(next(c for c in snapshot['project']['clips']
                                      if c['id'] == standing)['start_us'], 1000000)
                undone = invoke('undo', operation_id=uid(),
                                expected_operation_id=snapshot['undo_operation_id'])['snapshot']
                self.assertEqual(undone['project'], original['project'])
                redone = invoke('redo', operation_id=uid(),
                                expected_operation_id=undone['redo_operation_id'])['snapshot']
                self.assertEqual(redone['project'], snapshot['project'])
                path = root/'プリセット一括移動.vvm.json'
                invoke('save_project', path=str(path))
                self.assertEqual(json.loads(path.read_text()), snapshot['project'])
                client.close()
                client = None
                host.close()
                # A fresh recovery DB forces opening the saved file, not reattaching.
                host = HostProcess(root, data_dir=root/'reopened')
                host.ready(seconds=30)
                client = McpClient(root, data_dir=root/'reopened', response_timeout=30)
                reopened = invoke('open_project', path=str(path))['snapshot']
                self.assertEqual(reopened['project'], snapshot['project'])
            finally:
                if client:
                    client.close()
                host.close()
