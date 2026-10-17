import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


def uid():
    return str(uuid.uuid4())


class CheckpointComparisonTest(unittest.TestCase):
    def test_scene_clip_differences_and_membership_restore_after_restart(self):
        import json
        with tempfile.TemporaryDirectory(prefix='vvm-scene-checkpoint-', dir='/tmp') as directory:
            root = Path(directory); host = HostProcess(root); client = None
            def invoke(tool_name, **args):
                response = call(client, tool_name, **args)
                self.assertFalse(response.get('isError'), response)
                return response['structuredContent']
            try:
                host.ready(); client = McpClient(root)
                initial = invoke('create_project', title='シーンと字幕の保存時点比較')['snapshot']
                track, scene, clip, checkpoint = [uid() for _ in range(4)]
                before = invoke('apply_commands', operation_id=uid(), commands=[
                    dict(type='add_track', track=dict(id=track, name='字幕', kind='visual', visible=True, muted=False)),
                    dict(type='add_scene', scene=dict(id=scene, name='導入', start_us=0, duration_us=2000000)),
                    dict(type='add_clip', clip=dict(id=clip, track_id=track, scene_id=scene,
                        start_us=0, duration_us=1000000,
                        transform=dict(x=0, y=0, width=300, height=80, opacity=1),
                        content=dict(type='text', text='変更前', font_size=32, color='#000000'))),
                ])['snapshot']
                invoke('create_checkpoint', checkpoint_id=checkpoint, name='導入の保存時点', expected_revision=1)
                after = invoke('apply_commands', operation_id=uid(), commands=[
                    dict(type='rename_scene', scene_id=scene, name='紹介'),
                    dict(type='move_scene', scene_id=scene, start_us=3000000),
                    dict(type='set_text', clip_id=clip, text='変更後'),
                ])['snapshot']
                comparison = invoke('compare_checkpoints', checkpoint_id=checkpoint,
                                     target=dict(type='current', expected_revision=2))['comparison']
                actual = {(e['collection'], e['entity_id'], e['field']):
                          (json.loads(e['before']['json']), json.loads(e['after']['json']))
                          for e in comparison['entries']}
                self.assertEqual(actual, {
                    ('scenes', scene, '/name'): ('導入', '紹介'),
                    ('scenes', scene, '/start_us'): (0, 3000000),
                    ('clips', clip, '/start_us'): (0, 3000000),
                    ('clips', clip, '/content/text'): ('変更前', '変更後'),
                })
                self.assertEqual(comparison['total_changes'], 4)
                self.assertTrue(all(e['kind'] == 'changed' and not e['before']['truncated']
                                    and not e['after']['truncated'] for e in comparison['entries']))
                self.assertEqual(invoke('get_project')['snapshot'], after)
                client.close(); client = None; host.close()
                host = HostProcess(root); host.ready(); client = McpClient(root)
                invoke('attach_session', session_id=initial['session_id'])
                self.assertEqual(invoke('compare_checkpoints', checkpoint_id=checkpoint,
                    target=dict(type='current', expected_revision=2))['comparison'], comparison)
                restored = invoke('restore_checkpoint', checkpoint_id=checkpoint,
                                   operation_id=uid(), expected_revision=2)['snapshot']
                self.assertEqual(restored['project'], before['project'])
                self.assertEqual(restored['revision'], 3)
                undone = invoke('undo', operation_id=uid(),
                                expected_operation_id=restored['undo_operation_id'])['snapshot']
                self.assertEqual(undone['project'], after['project'])
                self.assertEqual(undone['revision'], 4)
            finally:
                if client: client.close()
                host.close()

    def test_pages_stale_revision_immutable_target_and_no_mutation(self):
        with tempfile.TemporaryDirectory(prefix='vvm-compare-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                call(client, 'create_project', title='Comparison fixture')
                base, target = uid(), uid()
                call(client, 'create_checkpoint', checkpoint_id=base, name='Before', expected_revision=0)
                track, clip = uid(), uid()
                result = call(client, 'apply_commands', operation_id=uid(), commands=[
                    {'type': 'rename_project', 'title': 'After'},
                    {'type': 'add_track', 'track': {'id': track, 'name': 'Text', 'kind': 'visual', 'visible': True, 'muted': False}},
                    {'type': 'add_clip', 'clip': {'id': clip, 'track_id': track, 'scene_id': None,
                        'start_us': 0, 'duration_us': 1000000,
                        'transform': {'x': 0, 'y': 0, 'width': 300, 'height': 80, 'opacity': 1},
                        'content': {'type': 'text', 'text': 'Caption', 'font_size': 32, 'color': '#000000'}}},
                ])
                self.assertFalse(result.get('isError'))
                call(client, 'create_checkpoint', checkpoint_id=target, name='After', expected_revision=1)
                before = call(client, 'get_project')['structuredContent']['snapshot']
                args = dict(checkpoint_id=base, target={'type': 'current', 'expected_revision': 1}, limit=1)
                entries, offset = [], 0
                while offset is not None:
                    page = call(client, 'compare_checkpoints', **args, offset=offset)['structuredContent']['comparison']
                    self.assertEqual(page['total_changes'], 3)
                    self.assertEqual(page['before_duration_us'], 0)
                    self.assertEqual(page['after_duration_us'], 1000000)
                    self.assertEqual(page['target_revision'], 1)
                    entries.extend(page['entries'])
                    offset = page['next_offset']
                self.assertEqual(len(entries), 3)
                self.assertEqual({e['entity_id'] for e in entries}, {None, track, clip})
                self.assertEqual(call(client, 'get_project')['structuredContent']['snapshot'], before)
                fixed_args = dict(checkpoint_id=base, target={'type': 'checkpoint', 'checkpoint_id': target})
                fixed = call(client, 'compare_checkpoints', **fixed_args)['structuredContent']['comparison']
                self.assertEqual(fixed['entries'], entries)
                self.assertIsNone(fixed['target_revision'])
                call(client, 'apply_commands', operation_id=uid(), commands=[{'type': 'rename_project', 'title': 'Newer'}])
                stale = call(client, 'compare_checkpoints', **args, offset=1)
                self.assertEqual(stale['structuredContent']['code'], 'CONFLICT')
                self.assertEqual(call(client, 'compare_checkpoints', **fixed_args)['structuredContent']['comparison'], fixed)
                for invalid in [dict(offset=4), dict(limit=0), dict(limit=51)]:
                    result = call(client, 'compare_checkpoints', **fixed_args, **invalid)
                    self.assertEqual(result['structuredContent']['code'], 'VALIDATION_FAILED')
                same = call(client, 'compare_checkpoints', checkpoint_id=base,
                            target={'type': 'checkpoint', 'checkpoint_id': base})['structuredContent']['comparison']
                self.assertEqual(same['entries'], [])
                self.assertEqual(same['before_hash'], same['after_hash'])
                call(client, 'create_project', title='Foreign')
                self.assertEqual(call(client, 'compare_checkpoints', **fixed_args)['structuredContent']['code'], 'VALIDATION_FAILED')
            finally:
                if client:
                    client.close()
                host.close()
